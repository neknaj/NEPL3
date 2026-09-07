use super::*;
use crate::{selection::BundleContext, tree::TreeError};
use nepl3_core::{
    syntax::{SyntaxBundle, canonical::BundleMappings},
    view::Trivia,
};
use nepl3_reader::model::ReaderFact;

pub(crate) fn tree_error(e: TreeError) -> RegionError {
    match e {
        TreeError::Stopped(v) => v.into(),
        _ => RegionError::Owner,
    }
}
pub(crate) fn mappings<'a>(
    bundle: &'a SyntaxBundle,
    b: &mut Budget,
) -> Result<BundleMappings<'a>, RegionError> {
    BundleMappings::new(bundle, b).map_err(|e| match e {
        nepl3_core::syntax::canonical::CanonicalError::Stopped(v) => v.into(),
        _ => RegionError::Owner,
    })
}
pub(crate) fn context<'a>(
    input: &'a PreparedBindingRequest<'_, '_>,
    bundle: &SyntaxBundle,
    b: &mut Budget,
) -> Result<&'a BundleContext, RegionError> {
    for context in &input.tree.tree().contexts {
        b.charge(Resource::Work, 1)?;
        let found = crate::tree::path(
            &input.tree.tree().bundle,
            &context.path,
            input.profile.registry(),
            b,
        )
        .map_err(tree_error)?;
        if core::ptr::eq(found, bundle) {
            return Ok(context);
        }
    }
    Err(RegionError::Owner)
}
pub(crate) fn source<'a>(
    map: &'a BundleMappings<'_>,
    span: &Span,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, RegionError> {
    for owner in map.entries() {
        for source in &owner.bundle().sources {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() + span.snapshot_ref().source.0.len()) as u64 + 40,
            )?;
            if source.identity() == span.snapshot_ref() {
                source.check_range(span.start(), span.end())?;
                return Ok(source);
            }
        }
    }
    Err(SourceError::MissingSnapshot.into())
}
fn origin_error(e: nepl3_core::origin::OriginError) -> RegionError {
    match e {
        nepl3_core::origin::OriginError::Stopped(v) => v.into(),
        nepl3_core::origin::OriginError::Source(v) => v.into(),
        _ => RegionError::Mapping,
    }
}
fn returned(
    head: Option<&Span>,
    trivia: &[Trivia],
    at: &Span,
    maps: &nepl3_core::origin::ValidatedSourceMap<'_>,
    b: &mut Budget,
) -> Result<(), RegionError> {
    if let Some(head) = head
        && maps.contains(head, at, b).map_err(origin_error)?
    {
        return Ok(());
    }
    for item in trivia {
        if maps.contains(&item.span, at, b).map_err(origin_error)? {
            return Ok(());
        }
    }
    Err(RegionError::Sidecar)
}
/// Validate the explicit reader return sidecar against its syntax owner. This
/// proves geometry and selected entry consistency, not execution provenance.
pub(crate) fn validate(
    input: &PreparedBindingRequest<'_, '_>,
    facts: Option<&[ReaderFactBatch]>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), RegionError> {
    let map = mappings(&input.tree.tree().bundle, b)?;
    let Some(facts) = facts else {
        return Ok(());
    };
    let mut closures = Vec::new();
    for owner in map.entries() {
        let mut store = nepl3_core::source::SourceStore::default();
        for source in &owner.bundle().sources {
            admission.admit_existing(source, b)?;
            store.insert(source.clone_with_budget(b)?)?;
        }
        let mapped = nepl3_core::origin::SourceMap::validate_mappings(
            &owner.bundle().source_maps,
            &store,
            b,
        )
        .map_err(origin_error)?;
        push(&mut closures, (store, mapped), b)?;
    }
    for (index, batch) in facts.iter().enumerate() {
        b.charge(Resource::Nodes, 1)?;
        let owner = crate::tree::path(
            &input.tree.tree().bundle,
            &batch.path,
            input.profile.registry(),
            b,
        )
        .map_err(tree_error)?;
        let mut owner_index = None;
        for (i, entry) in map.entries().iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(entry.bundle(), owner) {
                owner_index = Some(i);
                break;
            }
        }
        let (store, mapped) = closures
            .get(owner_index.ok_or(RegionError::Owner)?)
            .ok_or(RegionError::Owner)?;
        let check_span = |at: &Span, b: &mut Budget| -> Result<(), RegionError> {
            b.charge(Resource::Work, at.snapshot_ref().source.0.len() as u64 + 40)?;
            store
                .get_ref(at.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .check_range(at.start(), at.end())?;
            Ok(())
        };
        let context = context(input, owner, b)?;
        input
            .profile
            .validate_entry(&batch.entry, b)
            .map_err(|e| match e {
                crate::profile::ProfileError::Stopped(v) => v.into(),
                _ => RegionError::Owner,
            })?;
        let head = if let Some(node) = batch.node {
            b.charge(Resource::Work, context.nodes.len() as u64)?;
            let selection = context
                .nodes
                .iter()
                .find(|v| v.node == node)
                .ok_or(RegionError::Owner)?;
            b.charge(
                Resource::Work,
                (batch.entry.alias.len()
                    + batch.entry.category.len()
                    + batch.entry.mode.len()
                    + batch.entry.package.schema.package.len()) as u64
                    * 2
                    + 72,
            )?;
            if selection.entry != batch.entry {
                return Err(RegionError::Owner);
            }
            let node = owner
                .nodes
                .get(usize::try_from(node.0).map_err(|_| RegionError::Owner)?)
                .ok_or(RegionError::Owner)?;
            let token = owner
                .tokens
                .get(
                    usize::try_from(node.token.ok_or(RegionError::Owner)?.0)
                        .map_err(|_| RegionError::Owner)?,
                )
                .ok_or(RegionError::Owner)?;
            if token.leading_trivia.len() != batch.trivia.len() {
                return Err(RegionError::Sidecar);
            }
            for (a, d) in token.leading_trivia.iter().zip(&batch.trivia) {
                b.charge(
                    Resource::Work,
                    (a.span.snapshot_ref().source.0.len() + d.span.snapshot_ref().source.0.len())
                        as u64
                        + 44,
                )?;
                if a != d {
                    return Err(RegionError::Sidecar);
                }
            }
            Some(&token.head)
        } else {
            None
        };
        // One successful tokenizer dispatch owns one node's batch. Empty
        // trailing batches have no node and are kept in declared order.
        for prior in &facts[..index] {
            b.charge(
                Resource::Work,
                (prior.path.len() + batch.path.len()) as u64 + 1,
            )?;
            if batch.node.is_some() && prior.node == batch.node {
                let prior_owner = crate::tree::path(
                    &input.tree.tree().bundle,
                    &prior.path,
                    input.profile.registry(),
                    b,
                )
                .map_err(tree_error)?;
                if core::ptr::eq(owner, prior_owner) {
                    return Err(RegionError::Sidecar);
                }
            }
        }
        for trivia in &batch.trivia {
            check_span(&trivia.span, b)?;
            if let Some(head) = head {
                b.charge(
                    Resource::Work,
                    (head.snapshot_ref().source.0.len() + trivia.span.snapshot_ref().source.0.len())
                        as u64
                        + 42,
                )?;
                if head.snapshot_ref() != trivia.span.snapshot_ref()
                    || trivia.span.end() > head.start()
                {
                    return Err(RegionError::Sidecar);
                }
            }
        }
        for fact in &batch.facts {
            b.charge(Resource::Nodes, 1)?;
            let at = match fact {
                ReaderFact::Capture { name, span } => {
                    b.charge(Resource::Work, name.len() as u64 + 1)?;
                    if name.is_empty() {
                        return Err(RegionError::Sidecar);
                    }
                    span
                }
                ReaderFact::Presentation { class, span } => {
                    b.charge(
                        Resource::Work,
                        (class.name.len() + class.schema.package.len()) as u64 + 1,
                    )?;
                    if class.name.is_empty()
                        || input.profile.registry().descriptor(&class.schema).is_none()
                    {
                        return Err(RegionError::Sidecar);
                    }
                    span
                }
                ReaderFact::Relation {
                    schema,
                    kind,
                    from,
                    to,
                } => {
                    b.charge(
                        Resource::Work,
                        (schema.package.len() + kind.len()) as u64 + 1,
                    )?;
                    if kind.is_empty() || input.profile.registry().descriptor(schema).is_none() {
                        return Err(RegionError::Sidecar);
                    }
                    check_span(to, b)?;
                    from
                }
            };
            check_span(at, b)?;
            if matches!(fact, ReaderFact::Capture { .. }) {
                returned(head, &batch.trivia, at, mapped, b)?;
            }
        }
    }
    Ok(())
}
