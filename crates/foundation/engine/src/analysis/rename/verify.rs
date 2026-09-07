use super::*;
use nepl3_core::facts::{Entity, NamespaceRef, ScopeId};

pub(super) fn verify(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    p: &PreparedBindingRequest<'_, '_>,
    reply: &BoundBindingReply,
) -> Result<(), RenameError> {
    d.budget.poll()?;
    if p.key != reply.key()
        || p.key.profile_digest != d.original.key.profile_digest
        || p.key.execution_digest != d.original.key.execution_digest
        || p.key.request_digest != d.original.key.request_digest
        || p.options != d.original.options
        || p.limits != d.original.limits
    {
        return Err(RenameError::RequestMismatch);
    }
    let old = d
        .binding
        .for_source(&d.request.key, &d.request.source, d.budget)?
        .facts();
    let crate::binding::BindingOutcome::Complete(analysis) = &reply.reply().outcome else {
        return Err(BindingAccessError::Incomplete.into());
    };
    let new = analysis.facts();
    roots(d, old, new)?;
    super::shape::check(d, p, old, new)?;
    facts(d, old, new)?;
    let crate::binding::BindingOutcome::Complete(prior) = &d.binding.reply().outcome else {
        return Err(BindingAccessError::Incomplete.into());
    };
    for (a, c) in old.occurrences.iter().zip(&new.occurrences) {
        d.budget.charge(
            Resource::Work,
            (prior.result().open_inputs.len() + analysis.result().open_inputs.len()) as u64,
        )?;
        if prior.result().open_inputs.contains(&a.id)
            != analysis.result().open_inputs.contains(&c.id)
        {
            return Err(RenameError::ResolutionChanged);
        }
    }
    Ok(())
}
fn roots(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    old: &FactSet,
    new: &FactSet,
) -> Result<(), RenameError> {
    // Being the target of one map never exempts an entire snapshot. Every byte
    // and URI, including unmapped tails, belongs to the speculative transaction.
    for (a, c) in [(old, new), (new, old)] {
        for source in &a.sources {
            let mut known = false;
            for other in &c.sources {
                d.budget.charge(
                    Resource::Work,
                    (source.identity().source.0.len() + other.identity().source.0.len()) as u64
                        + 42,
                )?;
                if source.identity().source == other.identity().source {
                    known = true;
                }
            }
            if !known {
                return Err(RenameError::RequestMismatch);
            }
        }
    }
    for source in &new.sources {
        let mut wanted = None;
        for candidate in d.sources.snapshots() {
            d.budget.charge(
                Resource::Work,
                (source.identity().source.0.len() + candidate.identity().source.0.len()) as u64
                    + 42,
            )?;
            if source.identity().source == candidate.identity().source
                && wanted.is_none_or(|v: &SourceSnapshot| {
                    v.identity().revision < candidate.identity().revision
                })
            {
                wanted = Some(candidate);
            }
        }
        let wanted = wanted.ok_or(RenameError::RequestMismatch)?;
        d.budget.charge(
            Resource::Work,
            (source.text().len() + wanted.text().len() + source.uri().len() + wanted.uri().len())
                as u64
                + 42,
        )?;
        if source != wanted {
            return Err(RenameError::RequestMismatch);
        }
    }
    Ok(())
}

fn scope(
    old: &FactSet,
    new: &FactSet,
    a: ScopeId,
    c: ScopeId,
    b: &mut Budget,
) -> Result<bool, RenameError> {
    b.charge(Resource::Work, (old.scopes.len() + new.scopes.len()) as u64)?;
    Ok(old.scopes.iter().position(|v| v.id == a) == new.scopes.iter().position(|v| v.id == c))
}
fn namespace(
    old: &FactSet,
    new: &FactSet,
    a: NamespaceRef,
    c: NamespaceRef,
    b: &mut Budget,
) -> Result<bool, RenameError> {
    let a = old
        .namespaces
        .get(a.0 as usize)
        .ok_or(RenameError::ResolutionChanged)?;
    let c = new
        .namespaces
        .get(c.0 as usize)
        .ok_or(RenameError::ResolutionChanged)?;
    b.charge(
        Resource::Work,
        (a.schema.package.len() + c.schema.package.len() + a.name.len() + c.name.len()) as u64 + 42,
    )?;
    Ok(a.schema == c.schema
        && a.name == c.name
        && a.policy == c.policy
        && scope(old, new, a.root, c.root, b)?)
}
fn entity_location(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    old: &FactSet,
    new: &FactSet,
    a: &Entity,
    c: &Entity,
) -> Result<bool, RenameError> {
    if !scope(old, new, a.scope, c.scope, d.budget)?
        || !namespace(old, new, a.namespace, c.namespace, d.budget)?
    {
        return Ok(false);
    }
    for (a, c) in [(&a.selection, &c.selection), (&a.definition, &c.definition)] {
        match (a, c) {
            (Some(a), Some(c)) if spans(d, old, new, a, c)? => {}
            (None, None) => {}
            _ => return Ok(false),
        }
    }
    Ok(true)
}
fn facts(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    old: &FactSet,
    new: &FactSet,
) -> Result<(), RenameError> {
    if old.scopes.len() != new.scopes.len()
        || old.entities.len() != new.entities.len()
        || old.occurrences.len() != new.occurrences.len()
        || old.namespaces.len() != new.namespaces.len()
    {
        return Err(RenameError::ResolutionChanged);
    }
    for (a, c) in old.scopes.iter().zip(&new.scopes) {
        match (a.parent, c.parent) {
            (Some(a), Some(c)) if scope(old, new, a, c, d.budget)? => {}
            (None, None) => {}
            _ => return Err(RenameError::ResolutionChanged),
        }
    }
    let mut entities = Vec::new();
    for a in &old.entities {
        let wanted = if a.id == d.entity {
            &d.request.new_name
        } else {
            &a.name
        };
        let mut found = None;
        for c in &new.entities {
            d.budget
                .charge(Resource::Work, (wanted.len() + c.name.len()) as u64 + 1)?;
            if c.name != *wanted || !entity_location(d, old, new, a, c)? {
                continue;
            }
            if found.is_some() {
                return Err(RenameError::Collision);
            }
            found = Some(c.id);
        }
        let id = found.ok_or(RenameError::ResolutionChanged)?;
        d.budget.charge(Resource::Work, entities.len() as u64)?;
        if entities.iter().any(|(_, v)| *v == id) {
            return Err(RenameError::Collision);
        }
        push(&mut entities, (a.id, id), d.budget)?;
    }
    let target = entities
        .iter()
        .find(|(id, _)| *id == d.entity)
        .map(|(_, id)| *id)
        .ok_or(RenameError::ResolutionChanged)?;
    let target = new
        .entities
        .iter()
        .find(|v| v.id == target)
        .ok_or(RenameError::ResolutionChanged)?;
    for other in &new.entities {
        d.budget.charge(
            Resource::Work,
            (target.name.len() + other.name.len()) as u64 + 1,
        )?;
        if target.id != other.id
            && target.scope == other.scope
            && target.namespace == other.namespace
            && target.name == other.name
        {
            return Err(RenameError::Collision);
        }
    }
    // Correspondence uses source geometry, lexical placement, role and issuance
    // order, not equality of opaque EntityId/OccurrenceId numbers.
    for (a, c) in old.occurrences.iter().zip(&new.occurrences) {
        if a.role != c.role
            || !scope(old, new, a.scope, c.scope, d.budget)?
            || !namespace(old, new, a.namespace, c.namespace, d.budget)?
            || !spans(d, old, new, &a.span, &c.span)?
        {
            return Err(RenameError::ResolutionChanged);
        }
        let renamed = matches!(a.resolution,ReferenceResolution::Resolved(id) if id==d.entity);
        let wanted = if renamed {
            &d.request.new_name
        } else {
            &a.name
        };
        d.budget
            .charge(Resource::Work, (wanted.len() + c.name.len()) as u64)?;
        if *wanted != c.name {
            return Err(RenameError::ResolutionChanged);
        }
        if !resolution(&a.resolution, &c.resolution, &entities, d.budget)? {
            return Err(RenameError::ResolutionChanged);
        }
    }
    Ok(())
}
fn resolution(
    a: &ReferenceResolution,
    c: &ReferenceResolution,
    ids: &[(EntityId, EntityId)],
    b: &mut Budget,
) -> Result<bool, RenameError> {
    let mapped = |id: EntityId, b: &mut Budget| -> Result<Option<EntityId>, RenameError> {
        b.charge(Resource::Work, ids.len() as u64)?;
        Ok(ids.iter().find(|(a, _)| *a == id).map(|(_, c)| *c))
    };
    Ok(match (a, c) {
        (ReferenceResolution::Resolved(a), ReferenceResolution::Resolved(c)) => {
            mapped(*a, b)? == Some(*c)
        }
        (ReferenceResolution::Unresolved(a), ReferenceResolution::Unresolved(c)) => {
            b.charge(Resource::Work, (a.len() + c.len()) as u64)?;
            a == c
        }
        (ReferenceResolution::Ambiguous(a), ReferenceResolution::Ambiguous(c)) => {
            if a.len() != c.len() {
                return Ok(false);
            }
            for (a, c) in a.iter().zip(c) {
                if mapped(*a, b)? != Some(*c) {
                    return Ok(false);
                }
            }
            true
        }
        (ReferenceResolution::Deferred(a), ReferenceResolution::Deferred(c)) => {
            use nepl3_core::value::TypedValue;
            if a.len() != c.len() {
                return Ok(false);
            }
            for (a, c) in a.iter().zip(c) {
                let (schema_a, schema_c, name_a, name_c, variant_a, variant_c, fields_a, fields_c) =
                    match (a, c) {
                        (TypedValue::Record(a), TypedValue::Record(c)) => (
                            &a.schema, &c.schema, &a.kind, &c.kind, "", "", &a.fields, &c.fields,
                        ),
                        (TypedValue::Variant(a), TypedValue::Variant(c)) => (
                            &a.schema,
                            &c.schema,
                            &a.type_name,
                            &c.type_name,
                            a.variant.as_str(),
                            c.variant.as_str(),
                            &a.fields,
                            &c.fields,
                        ),
                        _ => return Ok(false),
                    };
                b.charge(
                    Resource::Work,
                    (schema_a.package.len()
                        + schema_c.package.len()
                        + name_a.len()
                        + name_c.len()
                        + variant_a.len()
                        + variant_c.len()) as u64
                        + 42,
                )?;
                if schema_a != schema_c
                    || name_a != name_c
                    || variant_a != variant_c
                    || fields_a.len() != fields_c.len()
                {
                    return Ok(false);
                }
                for (a, c) in fields_a.iter().zip(fields_c) {
                    if !a.equal_with_budget(c, b)? {
                        return Ok(false);
                    }
                }
            }
            true
        }
        _ => false,
    })
}
pub(super) fn spans(
    d: &mut RenameDraft<'_, '_, '_, '_>,
    old: &FactSet,
    new: &FactSet,
    a: &Span,
    c: &Span,
) -> Result<bool, RenameError> {
    if same_source(a.snapshot_ref(), c.snapshot_ref(), d.budget)?
        && a.start() == c.start()
        && a.end() == c.end()
    {
        return Ok(true);
    }
    // Broad covers can include mapped and unmapped bytes. The draft already
    // derived the exact edits for each declared snapshot, so compare their own
    // byte geometry before attempting a cross-source correspondence.
    for candidate in d.sources.snapshots() {
        d.budget.charge(
            Resource::Work,
            (candidate.identity().source.0.len() + a.snapshot_ref().source.0.len()) as u64 + 42,
        )?;
        if candidate.identity().source == a.snapshot_ref().source
            && same_source(candidate.identity(), c.snapshot_ref(), d.budget)?
        {
            return Ok(
                shift(a, a.start(), &d.candidate_edits, d.budget)? == c.start()
                    && shift(a, a.end(), &d.candidate_edits, d.budget)? == c.end(),
            );
        }
    }
    let a = root(a, old, d.budget)?;
    let c = root(c, new, d.budget)?;
    let Some(snapshot) = d.sources.latest(&a.snapshot_ref().source) else {
        return Ok(false);
    };
    if !same_source(snapshot.identity(), c.snapshot_ref(), d.budget)? {
        return Ok(false);
    }
    Ok(shift(&a, a.start(), &d.edits, d.budget)? == c.start()
        && shift(&a, a.end(), &d.edits, d.budget)? == c.end())
}
fn shift(span: &Span, point: u64, edits: &[TextEdit], b: &mut Budget) -> Result<u64, RenameError> {
    let mut result = i128::from(point);
    for edit in edits {
        if !same_source(span.snapshot_ref(), edit.span.snapshot_ref(), b)? {
            continue;
        }
        if edit.span.start() < point && point < edit.span.end() {
            return Err(RenameError::ShapeChanged);
        }
        if edit.span.end() <= point {
            result +=
                edit.replacement.len() as i128 - i128::from(edit.span.end() - edit.span.start());
        }
    }
    u64::try_from(result).map_err(|_| RenameError::ShapeChanged)
}
fn root(span: &Span, facts: &FactSet, b: &mut Budget) -> Result<Span, RenameError> {
    let mut span = copy_span(span, b)?;
    let mut seen = Vec::new();
    loop {
        b.observe_depth(seen.len() as u64 + 1)?;
        for old in &seen {
            let old: &Span = old;
            if same_source(old.snapshot_ref(), span.snapshot_ref(), b)?
                && old.start() == span.start()
                && old.end() == span.end()
            {
                return Err(OriginError::Cycle.into());
            }
        }
        push(&mut seen, copy_span(&span, b)?, b)?;
        let mut projected =
            super::mapping::project(&span, &facts.source_maps, &facts.sources, true, b)?;
        if projected.len() > 1 {
            return Err(RenameError::RenameNotInvertible);
        }
        let Some(next) = projected.pop() else {
            return Ok(span);
        };
        span = next;
    }
}
