//! Boundary checking shared by native and externally supplied provider results.
use super::*;
use nepl3_core::{origin::SourceMap, schema::TypeDescriptor, value::TypedValue};
pub(crate) fn request(
    request: &ReadRequest<'_>,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    state_type: &TypeDescriptor,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), ReaderError> {
    budget.poll()?;
    request.snapshot.check_range(request.start, request.limit)?;
    if sources.get_ref(request.snapshot.identity()) != Some(request.snapshot) {
        return Err(SourceError::MissingSnapshot.into());
    }
    admission.admit_existing(request.snapshot, budget)?;
    registry.validate(state_type, request.state, budget)?;
    let context = request.context;
    if registry.selected("nepl3.foundation", 1) != Some(context.foundation_schema())
        || registry.descriptor(&context.schema).is_none()
    {
        return Err(ReaderError::Context);
    }
    for source in context.sources() {
        budget.charge(Resource::Work, sources.snapshots().len() as u64)?;
        if sources.snapshots().iter().any(|existing| {
            existing.identity().source == source.identity().source
                && existing.identity().revision == source.identity().revision
                && existing != *source
        }) {
            return Err(SourceError::IdentityConflict.into());
        }
        admission.admit_existing(source, budget)?;
    }
    // A digest proof can be reused with a new operation; selected schema identities must still resolve.
    for binding in &context.environment.value.bindings {
        if registry.descriptor(&binding.namespace.schema).is_none() {
            return Err(SchemaError::UnknownSchema.into());
        }
        registry.validate_typed(&binding.value, budget)?;
    }

    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn provider(
    machine: &mut Machine<'_, '_>,
    frame: &ReaderFrame,
    call: &ProviderCall,
    reply: ProviderReply,
    saved: Usage,
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Outcome, ReaderError> {
    let (operation, kind) = match call {
        ProviderCall::Read { operation, .. } => (operation, ProviderKind::Read),
        ProviderCall::Transform { operation, .. } => (operation, ProviderKind::Transform),
        ProviderCall::Dependent { operation, .. } => (operation, ProviderKind::Dependent),
    };
    let signature = machine.checked.plan().provider(operation, kind)?;
    let original_sources = sources;
    let base_sources = combined(machine, &[], sources, budget, admission)?;
    let sources = &base_sources;
    match (call, reply) {
        (ProviderCall::Transform { request, .. }, ProviderReply::Transform(reply)) => {
            let combined = combined(machine, &reply.sources, original_sources, budget, admission)?;
            let sources = &combined;
            report(&reply.report, saved, machine.registry, sources, budget)?;
            machine
                .registry
                .validate(&signature.value_output, &reply.value, budget)?;
            artifacts(
                machine,
                &reply.sources,
                &reply.source_maps,
                &reply.view,
                &reply.facts,
                request.span.start(),
                request.span.end(),
                sources,
                budget,
                admission,
            )?;
            let mut view = reply.view;
            machine
                .current
                .view
                .elements
                .truncate(frame.checkpoint.view.elements.len());
            machine
                .current
                .view
                .roots
                .truncate(frame.checkpoint.view.roots.len());
            append_view(&mut machine.current.view, &mut view, budget)?;
            append_artifacts(
                machine,
                reply.facts,
                reply.sources,
                reply.source_maps,
                reply.report,
                budget,
            )?;
            Ok(Outcome::Matched(reply.value))
        }
        (ProviderCall::Read { request, .. }, ProviderReply::Read(reply))
        | (
            ProviderCall::Dependent {
                request: DependentRequest { request, .. },
                ..
            },
            ProviderReply::Read(reply),
        ) => match *reply {
            ReadReply::Await { .. } => Err(ReaderError::ProviderContract),
            ReadReply::Matched {
                value,
                end,
                new_state,
                mut view,
                facts,
                sources: added,
                source_maps,
                report: returned,
            } => {
                if end < request.start || end > request.limit {
                    return Err(ReaderError::ProviderContract);
                }
                machine.request.snapshot.check_range(request.start, end)?;
                let combined = combined(machine, &added, original_sources, budget, admission)?;
                let sources = &combined;
                report(&returned, saved, machine.registry, sources, budget)?;
                machine
                    .registry
                    .validate(&signature.value_output, &value, budget)?;
                machine
                    .registry
                    .validate(&signature.state_type, &new_state, budget)?;
                artifacts(
                    machine,
                    &added,
                    &source_maps,
                    &view,
                    &facts,
                    request.start,
                    end,
                    sources,
                    budget,
                    admission,
                )?;
                append_view(&mut machine.current.view, &mut view, budget)?;
                machine.current.cursor = end;
                machine.current.state = new_state;
                append_artifacts(machine, facts, added, source_maps, returned, budget)?;
                Ok(Outcome::Matched(value))
            }
            ReadReply::NoMatch {
                expected,
                furthest,
                report: returned,
                sources: added,
                source_maps,
            } => {
                if furthest < request.start
                    || !added.is_empty()
                    || !source_maps.is_empty()
                    || furthest > request.limit
                    || !returned.diagnostics.is_empty()
                    || !returned.events.is_empty()
                    || returned.trace_overflow.is_some()
                {
                    return Err(ReaderError::ProviderContract);
                }
                machine.request.snapshot.check_range(furthest, furthest)?;
                report(&returned, saved, machine.registry, sources, budget)?;
                expectations(&expected, machine.registry, budget)?;
                machine.current = copy(&frame.checkpoint, budget)?;
                Ok(Outcome::NoMatch { expected, furthest })
            }
            ReadReply::NeedMore {
                expected,
                report: returned,
                sources: added,
                source_maps,
            } => {
                if request.final_input
                    || !added.is_empty()
                    || !source_maps.is_empty()
                    || !returned.diagnostics.is_empty()
                    || !returned.events.is_empty()
                    || returned.trace_overflow.is_some()
                {
                    return Err(ReaderError::ProviderContract);
                }
                report(&returned, saved, machine.registry, sources, budget)?;
                expectations(&expected, machine.registry, budget)?;
                machine.current = copy(&frame.checkpoint, budget)?;
                Ok(Outcome::NeedMore(expected))
            }
            ReadReply::Failed {
                diagnostic,
                recovery,
                report: returned,
                sources: added,
                source_maps,
            } => {
                let combined = combined(machine, &added, original_sources, budget, admission)?;
                let sources = &combined;
                artifacts(
                    machine,
                    &added,
                    &source_maps,
                    &ViewBundle {
                        elements: Vec::new(),
                        roots: Vec::new(),
                    },
                    &[],
                    request.start,
                    request.limit,
                    sources,
                    budget,
                    admission,
                )?;
                report(&returned, saved, machine.registry, sources, budget)?;
                if returned
                    .diagnostics
                    .iter()
                    .filter(|d| *d == &diagnostic)
                    .count()
                    != 1
                {
                    return Err(ReaderError::ProviderContract);
                }
                if let Some(span) = &recovery {
                    validate_span(span, sources)?;
                }
                append_artifacts(machine, Vec::new(), added, source_maps, returned, budget)?;
                slot::<Diagnostic>(budget)?;
                Ok(Outcome::Failed {
                    diagnostic: Box::new(diagnostic),
                    recovery,
                })
            }
            ReadReply::Stopped {
                reason,
                report: returned,
                sources: added,
                source_maps,
            } => {
                let combined = combined(machine, &added, original_sources, budget, admission)?;
                let sources = &combined;
                artifacts(
                    machine,
                    &added,
                    &source_maps,
                    &ViewBundle {
                        elements: Vec::new(),
                        roots: Vec::new(),
                    },
                    &[],
                    request.start,
                    request.limit,
                    sources,
                    budget,
                    admission,
                )?;
                report(&returned, saved, machine.registry, sources, budget)?;
                append_artifacts(machine, Vec::new(), added, source_maps, returned, budget)?;
                Ok(Outcome::Stopped(reason))
            }
        },
        _ => Err(ReaderError::ProviderContract),
    }
}
fn expectations(
    expected: &[Expectation],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    for expectation in expected {
        budget.charge(Resource::Work, 1)?;
        match expectation {
            Expectation::ScalarClass(crate::plan::CharClass::Range { lo, hi }) if lo > hi => {
                return Err(ReaderError::ProviderContract);
            }
            Expectation::Provider {
                operation,
                arguments,
            } => {
                let desc = registry
                    .descriptor(&operation.schema)
                    .ok_or(SchemaError::UnknownSchema)?;
                if !desc.operations.iter().any(|op| op.name == operation.name) {
                    return Err(ReaderError::ProviderContract);
                }
                registry.validate_typed(arguments, budget)?;
            }
            _ => {}
        }
    }
    Ok(())
}
fn validate_span(
    span: &nepl3_core::source::Span,
    sources: &SourceStore,
) -> Result<(), ReaderError> {
    sources
        .get_ref(span.snapshot_ref())
        .ok_or(SourceError::MissingSnapshot)?
        .slice(span)?;
    Ok(())
}
fn typed(
    value: &TypedValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    registry.validate_typed(value, budget)?;
    Ok(())
}
fn report(
    report: &Report,
    saved: Usage,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    if !usage_at_least(report.usage, saved)
        || !usage_at_least(budget.usage(), report.usage)
        || report.usage.diagnostics.saturating_sub(saved.diagnostics)
            < report.diagnostics.len() as u64
        || report.usage.events.saturating_sub(saved.events) < report.events.len() as u64
    {
        return Err(ReaderError::ProviderContract);
    }
    for diagnostic in &report.diagnostics {
        budget.charge(Resource::Work, 1)?;
        if diagnostic.code.is_empty()
            || diagnostic.stage.is_empty()
            || registry.descriptor(&diagnostic.schema).is_none()
        {
            return Err(ReaderError::ProviderContract);
        }
        typed(&diagnostic.arguments, registry, budget)?;
        if let Some(span) = &diagnostic.primary {
            validate_span(span, sources)?;
        }
        for related in &diagnostic.related {
            typed(&related.arguments, registry, budget)?;
            if let Some(span) = &related.span {
                validate_span(span, sources)?;
            }
        }
        for fix in &diagnostic.fixes {
            for edit in &fix.edits {
                validate_span(&edit.span, sources)?;
            }
        }
    }
    for event in &report.events {
        budget.charge(Resource::Work, 1)?;
        if event.kind.is_empty() || registry.descriptor(&event.schema).is_none() {
            return Err(ReaderError::ProviderContract);
        }
        typed(&event.payload, registry, budget)?;
        if let Some(span) = &event.span {
            validate_span(span, sources)?;
        }
    }
    if report
        .trace_overflow
        .as_ref()
        .is_some_and(|o| o.dropped == 0)
    {
        return Err(ReaderError::ProviderContract);
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn artifacts(
    machine: &Machine<'_, '_>,
    added: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    view: &ViewBundle,
    facts: &[ReaderFact],
    start: u64,
    end: u64,
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), ReaderError> {
    for source in added {
        admission.admit_existing(source, budget)?;
    }
    let mut source_map = SourceMap::default();
    for mapping in machine.current.source_maps.iter().chain(maps) {
        source_map.insert(copy(mapping, budget)?, sources, budget)?;
    }
    let mapped = source_map.validated();
    view.validate_with_maps(sources, machine.registry, &mapped, budget)?;
    let consumed = machine
        .request
        .snapshot
        .span_with_budget(start, end, budget)?;
    for element in &view.elements {
        if !mapped.contains(&consumed, &element.span, budget)? {
            return Err(ReaderError::ProviderContract);
        }
    }
    for fact in facts {
        budget.charge(Resource::Work, 1)?;
        match fact {
            ReaderFact::Capture { name, span } => {
                if name.is_empty() {
                    return Err(ReaderError::ProviderContract);
                }
                validate_span(span, sources)?;
                if !mapped.contains(&consumed, span, budget)? {
                    return Err(ReaderError::ProviderContract);
                }
            }
            ReaderFact::Presentation { class, span } => {
                if class.name.is_empty() || machine.registry.descriptor(&class.schema).is_none() {
                    return Err(ReaderError::ProviderContract);
                }
                validate_span(span, sources)?;
            }
            ReaderFact::Relation {
                schema,
                kind,
                from,
                to,
            } => {
                if kind.is_empty() || machine.registry.descriptor(schema).is_none() {
                    return Err(ReaderError::ProviderContract);
                }
                validate_span(from, sources)?;
                validate_span(to, sources)?;
            }
        }
    }
    Ok(())
}
fn append_view(
    target: &mut ViewBundle,
    incoming: &mut ViewBundle,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    reindex(incoming, target.elements.len() as u64, true)?;
    budget.charge(
        Resource::AllocationUnits,
        (incoming.elements.len() as u64).saturating_mul(core::mem::size_of::<ViewElement>() as u64)
            + (incoming.roots.len() as u64).saturating_mul(core::mem::size_of::<ViewRef>() as u64),
    )?;
    target.elements.append(&mut incoming.elements);
    target.roots.append(&mut incoming.roots);
    Ok(())
}
fn append_artifacts(
    machine: &mut Machine<'_, '_>,
    mut facts: Vec<ReaderFact>,
    mut sources: Vec<SourceSnapshot>,
    mut maps: Vec<nepl3_core::origin::Mapping>,
    mut report: Report,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    let sizes = [
        (facts.len(), core::mem::size_of::<ReaderFact>()),
        (sources.len(), core::mem::size_of::<SourceSnapshot>()),
        (
            maps.len(),
            core::mem::size_of::<nepl3_core::origin::Mapping>(),
        ),
        (report.diagnostics.len(), core::mem::size_of::<Diagnostic>()),
        (
            report.events.len(),
            core::mem::size_of::<nepl3_core::diagnostic::Event>(),
        ),
    ];
    for (count, size) in sizes {
        budget.charge(
            Resource::AllocationUnits,
            (count as u64).saturating_mul(size as u64),
        )?;
    }
    machine.current.facts.append(&mut facts);
    machine.current.sources.append(&mut sources);
    machine.current.source_maps.append(&mut maps);
    machine.current.diagnostics.append(&mut report.diagnostics);
    machine.current.events.append(&mut report.events);
    if let Some(overflow) = report.trace_overflow {
        if machine.current.trace_overflow.is_some() {
            return Err(ReaderError::ProviderContract);
        }
        machine.current.trace_overflow = Some(overflow);
    }
    Ok(())
}

fn combined(
    machine: &Machine<'_, '_>,
    added: &[SourceSnapshot],
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, ReaderError> {
    let mut combined = SourceStore::default();
    for source in core::iter::once(machine.request.snapshot)
        .chain(machine.request.context.sources().iter().copied())
        .chain(&machine.current.sources)
    {
        combined.insert(copy(source, budget)?)?;
    }
    for (index, source) in added.iter().enumerate() {
        if added[..index]
            .iter()
            .any(|s| s.identity() == source.identity())
        {
            return Err(ReaderError::ProviderContract);
        }
        budget.charge(Resource::Work, sources.snapshots().len() as u64)?;
        if sources.snapshots().iter().any(|existing| {
            existing.identity().source == source.identity().source
                && existing.identity().revision == source.identity().revision
                && existing != source
        }) {
            return Err(SourceError::IdentityConflict.into());
        }
        admission.admit_existing(source, budget)?;
        combined.insert(copy(source, budget)?)?;
    }
    Ok(combined)
}
