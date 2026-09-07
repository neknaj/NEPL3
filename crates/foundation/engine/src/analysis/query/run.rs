use super::*;
use nepl3_core::facts::{FactSet, Occurrence};
pub(super) fn sort_sources(
    sources: &mut [SourceSnapshot],
    b: &mut Budget,
) -> Result<(), QueryError> {
    for i in 1..sources.len() {
        let mut j = i;
        while j > 0 {
            let a = sources[j - 1].identity();
            let c = sources[j].identity();
            b.charge(
                Resource::Work,
                (a.source.0.len() + c.source.0.len()) as u64 + 42,
            )?;
            let order = a
                .source
                .0
                .cmp(&c.source.0)
                .then_with(|| a.revision.cmp(&c.revision))
                .then_with(|| a.digest.cmp(&c.digest));
            if !order.is_gt() {
                break;
            }
            sources.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(())
}

pub(super) fn run(
    binding: &BoundBindingReply,
    request: &QueryRequest,
    out: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<QueryOutcome, QueryError> {
    run_selected(binding, request, None, out, b, admission)
}
pub(super) fn run_selected(
    binding: &BoundBindingReply,
    request: &QueryRequest,
    occurrence_id: Option<OccurrenceId>,
    out: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<QueryOutcome, QueryError> {
    let analysis = binding.for_source(&request.key, &request.source, b)?;
    let sources = binding.reply().sources();
    let source = find_source(sources, &request.source, b)?;
    admission.admit_existing(source, b)?;
    source.check_range(request.offset, request.offset)?;
    add_source(out, source, b, admission)?;
    let facts = analysis.facts();
    let mut selected: Option<&Occurrence> = None;
    for occurrence in &facts.occurrences {
        b.charge(Resource::Nodes, 1)?;
        if let Some(id) = occurrence_id {
            b.charge(Resource::Work, 1)?;
            if occurrence.id == id {
                selected = Some(occurrence);
                break;
            }
            continue;
        }
        b.charge(
            Resource::Work,
            (occurrence.span.snapshot_ref().source.0.len() + request.source.source_id.0.len())
                as u64
                + 42,
        )?;
        if occurrence.span.snapshot_ref() == source.identity()
            && occurrence.span.start() <= request.offset
            && request.offset < occurrence.span.end()
            && selected.is_none_or(|prior| {
                occurrence.span.end() - occurrence.span.start()
                    < prior.span.end() - prior.span.start()
            })
        {
            selected = Some(occurrence);
        }
    }
    if occurrence_id.is_some() && selected.is_none() {
        return Err(QueryError::Facts);
    }
    b.charge(Resource::Work, analysis.result().open_inputs.len() as u64)?;
    let selection = selected
        .map(|v| -> Result<QuerySelection, QueryError> {
            Ok(QuerySelection {
                occurrence: v.id,
                span: copy_span(&v.span, b)?,
                resolution: v.resolution.clone_with_budget(b)?,
                open_input: analysis.result().open_inputs.contains(&v.id),
            })
        })
        .transpose()?;
    let ids = match selected.map(|v| &v.resolution) {
        Some(ReferenceResolution::Resolved(id)) => core::slice::from_ref(id),
        Some(ReferenceResolution::Ambiguous(ids)) => ids.as_slice(),
        _ => &[],
    };
    match request.kind {
        QueryKind::Definition => {
            let mut targets = Vec::new();
            for id in ids {
                b.charge(Resource::Work, facts.entities.len() as u64 + 1)?;
                let entity = facts
                    .entities
                    .iter()
                    .find(|v| v.id == *id)
                    .ok_or(QueryError::Facts)?;
                let location = if let Some(range) = &entity.definition {
                    let target = span_source(sources, range, b)?;
                    add_source(out, target, b, admission)?;
                    Some(DefinitionLocation {
                        uri: text(target.uri(), b)?,
                        range: copy_span(range, b)?,
                        selection: entity
                            .selection
                            .as_ref()
                            .map(|v| copy_span(v, b))
                            .transpose()?,
                    })
                } else {
                    None
                };
                push(
                    &mut targets,
                    DefinitionTarget {
                        entity: *id,
                        location,
                    },
                    b,
                )?;
            }
            Ok(QueryOutcome::Definition { selection, targets })
        }
        QueryKind::References(options) => {
            let mut groups = Vec::new();
            for id in ids {
                let locations = references(*id, facts, sources, options, out, b, admission)?;
                push(
                    &mut groups,
                    EntityReferences {
                        entity: *id,
                        locations,
                    },
                    b,
                )?;
            }
            Ok(QueryOutcome::References { selection, groups })
        }
    }
}
fn references(
    id: EntityId,
    facts: &FactSet,
    sources: &[SourceSnapshot],
    options: ReferenceOptions,
    out: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Vec<ReferenceLocation>, QueryError> {
    let mut locations = Vec::new();
    for occurrence in &facts.occurrences {
        b.charge(Resource::Nodes, 1)?;
        b.charge(Resource::Work, 1)?;
        let allowed = match occurrence.role {
            OccurrenceRole::Reference => true,
            OccurrenceRole::Definition => options.include_definitions,
            OccurrenceRole::Import => options.include_imports,
            OccurrenceRole::Export => options.include_exports,
        };
        if !allowed {
            continue;
        }
        let ambiguous = match &occurrence.resolution {
            ReferenceResolution::Resolved(target) if *target == id => false,
            ReferenceResolution::Ambiguous(ids) if options.include_ambiguous => {
                b.charge(Resource::Work, ids.len() as u64)?;
                if !ids.contains(&id) {
                    continue;
                }
                true
            }
            _ => continue,
        };
        let source = span_source(sources, &occurrence.span, b)?;
        add_source(out, source, b, admission)?;
        push(
            &mut locations,
            ReferenceLocation {
                occurrence: occurrence.id,
                role: occurrence.role,
                uri: text(source.uri(), b)?,
                span: copy_span(&occurrence.span, b)?,
                ambiguous,
            },
            b,
        )?;
    }
    Ok(locations)
}
fn find_source<'a>(
    sources: &'a [SourceSnapshot],
    wanted: &SourceRef,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, QueryError> {
    for s in sources {
        b.charge(
            Resource::Work,
            (s.identity().source.0.len() + wanted.source_id.0.len()) as u64 + 42,
        )?;
        if s.identity().source == wanted.source_id
            && s.identity().revision == wanted.revision
            && s.identity().digest == wanted.digest
        {
            return Ok(s);
        }
    }
    Err(QueryError::Source(SourceError::MissingSnapshot))
}
fn span_source<'a>(
    sources: &'a [SourceSnapshot],
    span: &Span,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, QueryError> {
    for source in sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + span.snapshot_ref().source.0.len()) as u64 + 42,
        )?;
        if source.identity() == span.snapshot_ref() {
            return Ok(source);
        }
    }
    Err(QueryError::Source(SourceError::MissingSnapshot))
}
fn add_source(
    out: &mut Vec<SourceSnapshot>,
    source: &SourceSnapshot,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), QueryError> {
    admission.admit_existing(source, b)?;
    for previous in out.iter() {
        b.charge(
            Resource::Work,
            (previous.identity().source.0.len() + source.identity().source.0.len()) as u64 + 42,
        )?;
        if previous.identity() == source.identity() {
            return Ok(());
        }
    }
    let copy = source.clone_with_budget(b)?;
    push(out, copy, b)
}
fn copy_span(span: &Span, b: &mut Budget) -> Result<Span, QueryError> {
    let len = span.snapshot_ref().source.0.len() as u64;
    b.charge(Resource::Work, len + 42)?;
    b.charge(Resource::AllocationUnits, len)?;
    Ok(span.clone())
}
fn text(value: &str, b: &mut Budget) -> Result<String, QueryError> {
    b.charge(Resource::Work, value.len() as u64)?;
    b.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(String::from(value))
}
fn push<T>(out: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), QueryError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    out.push(value);
    Ok(())
}
