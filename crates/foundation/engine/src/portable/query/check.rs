use super::*;
use nepl3_core::source::{SourceError, SourceSnapshot};
pub(super) fn validate<E>(
    reply: &QueryReply,
    request: &QueryRequest,
    registry: &SchemaRegistry,
    store: &SourceStore,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::Work, 128)?;
    if reply.key != request.key {
        return Err(PortableError::RequestMismatch);
    }
    reply
        .report
        .validate(store, &[], registry, b)
        .map_err(crate::facts::FactsError::from)?;
    // This query implementation emits no new diagnostics/events. Analysis
    // diagnostics remain in the binding reply, not re-emitted by every lookup.
    if !reply.report.diagnostics.is_empty()
        || !reply.report.events.is_empty()
        || reply.report.trace_overflow.is_some()
    {
        return Err(PortableError::Shape);
    }
    match &reply.outcome {
        QueryOutcome::Invalid(e) => {
            if e.stop_reason().is_some() || !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        QueryOutcome::Stopped(_) => {
            if !reply.sources.is_empty() {
                return Err(PortableError::Shape);
            }
            return Ok(());
        }
        _ => {}
    }
    let mut source = None;
    for v in store.snapshots() {
        b.charge(
            Resource::Work,
            (v.identity().source.0.len() + request.source.source_id.0.len()) as u64 + 42,
        )?;
        if v.identity().source == request.source.source_id
            && v.identity().revision == request.source.revision
            && v.identity().digest == request.source.digest
        {
            source = Some(v);
        }
    }
    let source = source.ok_or(SourceError::MissingSnapshot)?;
    source.check_range(request.offset, request.offset)?;
    let (selection, ids) = match &reply.outcome {
        QueryOutcome::Definition { selection, targets } => {
            if !matches!(request.kind, QueryKind::Definition) {
                return Err(PortableError::RequestMismatch);
            }
            for target in targets {
                if let Some(l) = &target.location {
                    location(&l.range, &l.uri, store, b)?;
                    if let Some(s) = &l.selection {
                        span(s, store, b)?;
                        b.charge(
                            Resource::Work,
                            (s.snapshot_ref().source.0.len()
                                + l.range.snapshot_ref().source.0.len())
                                as u64
                                + 42,
                        )?;
                        if s.snapshot_ref() != l.range.snapshot_ref()
                            || s.start() < l.range.start()
                            || s.end() > l.range.end()
                        {
                            return Err(PortableError::Shape);
                        }
                    }
                }
            }
            (selection, targets.iter().map(|v| v.entity).collect_ids(b)?)
        }
        QueryOutcome::References { selection, groups } => {
            let QueryKind::References(options) = request.kind else {
                return Err(PortableError::RequestMismatch);
            };
            for group in groups {
                for (i, l) in group.locations.iter().enumerate() {
                    b.charge(Resource::Work, i as u64 + 1)?;
                    let allowed = match l.role {
                        OccurrenceRole::Reference => true,
                        OccurrenceRole::Definition => options.include_definitions,
                        OccurrenceRole::Import => options.include_imports,
                        OccurrenceRole::Export => options.include_exports,
                    };
                    if !allowed
                        || (l.ambiguous && !options.include_ambiguous)
                        || group.locations[..i]
                            .iter()
                            .any(|v| v.occurrence == l.occurrence)
                    {
                        return Err(PortableError::Shape);
                    }
                    location(&l.span, &l.uri, store, b)?;
                }
            }
            (selection, groups.iter().map(|v| v.entity).collect_ids(b)?)
        }
        _ => return Err(PortableError::Shape),
    };
    let expected = if let Some(selection) = selection {
        if selection.open_input
            && !matches!(selection.resolution, ReferenceResolution::Unresolved(_))
        {
            return Err(PortableError::Shape);
        }
        span(&selection.span, store, b)?;
        b.charge(
            Resource::Work,
            (selection.span.snapshot_ref().source.0.len() + source.identity().source.0.len())
                as u64
                + 42,
        )?;
        if selection.span.snapshot_ref() != source.identity()
            || selection.span.start() > request.offset
            || request.offset >= selection.span.end()
        {
            return Err(PortableError::Shape);
        }
        match &selection.resolution {
            ReferenceResolution::Resolved(id) => core::slice::from_ref(id),
            ReferenceResolution::Ambiguous(ids) => {
                if ids.len() < 2 {
                    return Err(PortableError::Shape);
                }
                ids.as_slice()
            }
            ReferenceResolution::Deferred(v) => {
                if v.is_empty() {
                    return Err(PortableError::Shape);
                }
                &[]
            }
            ReferenceResolution::Unresolved(_) => &[],
        }
    } else {
        &[]
    };
    b.charge(Resource::Work, (ids.len() + expected.len()) as u64 + 1)?;
    if ids != expected {
        return Err(PortableError::Shape);
    }
    for (i, id) in ids.iter().enumerate() {
        b.charge(Resource::Work, i as u64 + 1)?;
        if ids[..i].contains(id) {
            return Err(PortableError::Shape);
        }
    }
    Ok(())
}
trait CollectIds: Iterator<Item = EntityId> + Sized {
    fn collect_ids<E>(self, b: &mut Budget) -> Result<Vec<EntityId>, PortableError<E>> {
        let mut out = Vec::new();
        for id in self {
            push(&mut out, id, b)?;
        }
        Ok(out)
    }
}
impl<I: Iterator<Item = EntityId>> CollectIds for I {}
fn span<'a, E>(
    v: &Span,
    store: &'a SourceStore,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, PortableError<E>> {
    for source in store.snapshots() {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + v.snapshot_ref().source.0.len()) as u64 + 42,
        )?;
        if source.identity() == v.snapshot_ref() {
            source.check_range(v.start(), v.end())?;
            return Ok(source);
        }
    }
    Err(SourceError::MissingSnapshot.into())
}
fn location<E>(
    range: &Span,
    uri: &str,
    store: &SourceStore,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    let source = span(range, store, b)?;
    b.charge(Resource::Work, (source.uri().len() + uri.len()) as u64)?;
    if source.uri() != uri {
        return Err(SourceError::IdentityConflict.into());
    }
    Ok(())
}
