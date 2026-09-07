use super::*;
pub(super) fn prepare<'a, 'tree, 'profile, 'budget>(
    original: &'a PreparedBindingRequest<'tree, 'profile>,
    binding: &'a BoundBindingReply,
    request: &'a RenameRequest,
    budget: &'budget mut Budget,
) -> Result<RenameDraft<'a, 'tree, 'profile, 'budget>, RenameError> {
    budget.poll()?;
    request.validate_identity(budget)?;
    if original.key != request.key || binding.key() != request.key {
        return Err(RenameError::RequestMismatch);
    }
    if budget.limits() != original.limits {
        return Err(BindingAccessError::LimitsMismatch.into());
    }
    budget.charge(Resource::Work, request.new_name.len() as u64)?;
    if request.new_name.is_empty() {
        return Err(RenameError::InvalidName);
    }
    let analysis = binding.for_source(&request.key, &request.source, budget)?;
    let facts = analysis.facts();
    let mut sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    for source in binding.reply().sources() {
        admission.admit_existing(source, budget)?;
        for prior in sources.snapshots() {
            budget.charge(
                Resource::Work,
                (prior.identity().source.0.len() + source.identity().source.0.len()) as u64 + 42,
            )?;
        }
        sources.insert(source.clone_with_budget(budget)?)?;
    }
    let source = sources
        .resolve(&request.source)
        .ok_or(SourceError::MissingSnapshot)?;
    source.check_range(request.offset, request.offset)?;
    let mut selected: Option<&nepl3_core::facts::Occurrence> = None;
    for occurrence in &facts.occurrences {
        budget.charge(Resource::Nodes, 1)?;
        if same_source(occurrence.span.snapshot_ref(), source.identity(), budget)?
            && occurrence.span.start() <= request.offset
            && request.offset < occurrence.span.end()
            && selected.is_none_or(|v| {
                occurrence.span.end() - occurrence.span.start() < v.span.end() - v.span.start()
            })
        {
            selected = Some(occurrence);
        }
    }
    let entity = match &selected.ok_or(RenameError::NoOccurrence)?.resolution {
        ReferenceResolution::Resolved(id) => *id,
        ReferenceResolution::Ambiguous(_) => return Err(RenameError::Ambiguous),
        ReferenceResolution::Unresolved(_) => return Err(RenameError::Unresolved),
        ReferenceResolution::Deferred(_) => return Err(RenameError::Deferred),
    };
    budget.charge(Resource::Work, facts.entities.len() as u64)?;
    let target = facts
        .entities
        .iter()
        .find(|v| v.id == entity)
        .ok_or(RenameError::RequestMismatch)?;
    if target.selection.is_none() {
        return Err(RenameError::NoLocation);
    }
    let namespace = facts
        .namespaces
        .get(target.namespace.0 as usize)
        .ok_or(RenameError::RequestMismatch)?;
    for registration in &original.profile.profile().languages {
        let package = original
            .profile
            .language(&registration.alias, budget)
            .map_err(|error| match error {
                crate::profile::ProfileError::Stopped(reason) => RenameError::Stopped(reason),
                _ => RenameError::RequestMismatch,
            })?;
        budget.charge(
            Resource::Work,
            (namespace.schema.package.len() + package.schema.package.len()) as u64 + 42,
        )?;
        if namespace.schema != package.schema {
            continue;
        }
        for form in &package.forms {
            budget.charge(
                Resource::Work,
                (form.spelling.len() + request.new_name.len()) as u64,
            )?;
            if form.spelling == request.new_name {
                return Err(RenameError::InvalidName);
            }
        }
    }
    let mut edits = Vec::new();
    for occurrence in &facts.occurrences {
        budget.charge(Resource::Work, 1)?;
        match &occurrence.resolution {
            ReferenceResolution::Ambiguous(ids) => {
                budget.charge(Resource::Work, ids.len() as u64)?;
                if ids.contains(&entity) {
                    return Err(RenameError::Ambiguous);
                }
            }
            ReferenceResolution::Resolved(id) if *id == entity => {
                if let Some(id) = occurrence.origin {
                    budget.charge(Resource::Work, 1)?;
                    if matches!(
                        facts.origins.get(id.0 as usize),
                        Some(Origin::Composite(_) | Origin::Synthetic { .. }) | None
                    ) {
                        return Err(RenameError::RenameNotInvertible);
                    }
                }
                add_edit(
                    &occurrence.span,
                    &occurrence.name,
                    facts,
                    request,
                    &sources,
                    &mut edits,
                    budget,
                )?;
            }
            _ => {}
        }
    }
    // A declared entity name must be editable even when a custom provider does
    // not also issue a Definition occurrence.
    add_edit(
        target.selection.as_ref().ok_or(RenameError::NoLocation)?,
        &target.name,
        facts,
        request,
        &sources,
        &mut edits,
        budget,
    )?;
    for i in 1..edits.len() {
        let mut j = i;
        while j > 0 {
            let a = &edits[j - 1].span;
            let c = &edits[j].span;
            budget.charge(
                Resource::Work,
                (a.snapshot_ref().source.0.len() + c.snapshot_ref().source.0.len()) as u64 + 42,
            )?;
            if (&a.snapshot_ref().source.0, a.start(), a.end())
                <= (&c.snapshot_ref().source.0, c.start(), c.end())
            {
                break;
            }
            edits.swap(j - 1, j);
            j -= 1;
        }
    }
    // Exact derived snapshots are part of the candidate, too. Apply explicit
    // internal edits while preserving every unmapped byte; only writable root
    // edits are eventually advertised to the editor.
    let candidates = derived_edits(&edits, facts, &sources, budget)?;
    sources.apply(&candidates, budget, &mut admission)?;
    Ok(RenameDraft {
        original,
        binding,
        request,
        entity,
        edits,
        candidate_edits: candidates,
        sources,
        admission,
        budget,
    })
}
fn derived_edits(
    edits: &[TextEdit],
    facts: &FactSet,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<Vec<TextEdit>, RenameError> {
    let mut out = Vec::new();
    let mut depths = Vec::new();
    for edit in edits {
        push(
            &mut out,
            TextEdit {
                span: copy_span(&edit.span, b)?,
                expected_digest: edit.expected_digest,
                replacement: text(&edit.replacement, b)?,
            },
            b,
        )?;
        push(&mut depths, 1u64, b)?;
    }
    let mut index = 0;
    while index < out.len() {
        b.observe_depth(depths[index])?;
        let projected = super::mapping::project(
            &out[index].span,
            &facts.source_maps,
            &facts.sources,
            false,
            b,
        )?;
        for span in projected {
            let source = sources
                .get_ref(span.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?;
            let mut known = false;
            for prior in &out {
                if same_source(prior.span.snapshot_ref(), span.snapshot_ref(), b)?
                    && prior.span.start() == span.start()
                    && prior.span.end() == span.end()
                {
                    known = true;
                }
            }
            if known {
                continue;
            }
            let old = source.slice(&span)?;
            b.charge(Resource::Work, old.len() as u64)?;
            let value = TextEdit {
                span,
                expected_digest: Digest::of(old.as_bytes()),
                replacement: text(&out[index].replacement, b)?,
            };
            push(&mut out, value, b)?;
            let depth = depths[index].checked_add(1).ok_or(StopReason::DepthLimit)?;
            push(&mut depths, depth, b)?;
        }
        index += 1;
    }
    Ok(out)
}
fn add_edit(
    span: &Span,
    name: &str,
    facts: &FactSet,
    request: &RenameRequest,
    sources: &SourceStore,
    edits: &mut Vec<TextEdit>,
    b: &mut Budget,
) -> Result<(), RenameError> {
    let span = inverse(span, facts, &request.writable, b)?;
    let old = sources
        .get_ref(span.snapshot_ref())
        .ok_or(SourceError::MissingSnapshot)?
        .slice(&span)?;
    b.charge(Resource::Work, (old.len() + name.len()) as u64)?;
    // Replacement of an encoded spelling needs a declared inverse encoder.
    // Equal decoded names do not authorize dropping quotes or escape syntax.
    if old != name {
        return Err(RenameError::RenameNotInvertible);
    }
    for prior in edits.iter() {
        if same_source(prior.span.snapshot_ref(), span.snapshot_ref(), b)?
            && prior.span.start() == span.start()
            && prior.span.end() == span.end()
        {
            return Ok(());
        }
    }
    b.charge(Resource::Work, old.len() as u64)?;
    push(
        edits,
        TextEdit {
            expected_digest: Digest::of(old.as_bytes()),
            span,
            replacement: text(&request.new_name, b)?,
        },
        b,
    )
}
fn inverse(
    span: &Span,
    facts: &FactSet,
    writable: &[SourceRef],
    b: &mut Budget,
) -> Result<Span, RenameError> {
    let mut current = copy_span(span, b)?;
    let mut visited = Vec::new();
    loop {
        b.observe_depth(visited.len() as u64 + 1)?;
        b.charge(Resource::Nodes, 1)?;
        for seen in &visited {
            let seen: &Span = seen;
            if same_source(seen.snapshot_ref(), current.snapshot_ref(), b)?
                && seen.start() == current.start()
                && seen.end() == current.end()
            {
                return Err(OriginError::Cycle.into());
            }
        }
        push(&mut visited, copy_span(&current, b)?, b)?;
        let mut projected =
            super::mapping::project(&current, &facts.source_maps, &facts.sources, true, b)?;
        if projected.len() > 1 {
            return Err(RenameError::RenameNotInvertible);
        }
        if let Some(next) = projected.pop() {
            current = next;
            continue;
        }
        for allowed in writable {
            b.charge(
                Resource::Work,
                (allowed.source_id.0.len() + current.snapshot_ref().source.0.len()) as u64 + 42,
            )?;
            if allowed.source_id == current.snapshot_ref().source
                && allowed.revision == current.snapshot_ref().revision
                && allowed.digest == current.snapshot_ref().digest
            {
                return Ok(current);
            }
        }
        return Err(RenameError::NotWritable);
    }
}
