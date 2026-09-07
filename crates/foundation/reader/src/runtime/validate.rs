//! Boundary checking shared by native and externally supplied provider results.
use super::*;
use nepl3_core::{origin::SourceMap, schema::TypeDescriptor};
impl ProviderReply {
    /// Check the constant-size outcome/report tag invariant before consuming a
    /// pending slot. Payload, schema, source and usage validation still occurs
    /// at the provider boundary. Terminal event overflow requires a stopped result.
    pub fn validate_outcome(&self) -> Result<(), ReaderError> {
        let overflow = match self {
            Self::Read(reply) => match reply.as_ref() {
                ReadReply::Stopped { .. } => return Ok(()),
                ReadReply::Matched { report, .. }
                | ReadReply::NoMatch { report, .. }
                | ReadReply::NeedMore { report, .. }
                | ReadReply::Failed { report, .. }
                | ReadReply::Await { report, .. } => report.trace_overflow.is_some(),
            },
            Self::Transform(reply) => {
                !matches!(reply.outcome, TransformOutcome::Stopped { .. })
                    && reply.report.trace_overflow.is_some()
            }
        };
        if overflow {
            Err(ReaderError::ProviderContract)
        } else {
            Ok(())
        }
    }
}
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
    let selected = sources
        .get_ref(request.snapshot.identity())
        .ok_or(SourceError::MissingSnapshot)?;
    if !selected.eq_with_budget(request.snapshot, budget)? {
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
        if let Some(existing) = sources.get_revision_with_budget(
            &source.identity().source,
            source.identity().revision,
            budget,
        )? && !existing.eq_with_budget(source, budget)?
        {
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
mod provider;
pub(super) use provider::apply_provider;
pub(crate) use provider::{ProviderBoundary, ProviderReplyRef, check_provider};

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
/// Validate one diagnostic against the explicitly declared request sources and accepted delta.
/// Does not clone a report, admit new sources, or charge the diagnostic count.
pub(crate) fn accepted_diagnostic(
    diagnostic: &Diagnostic,
    sources: &SourceStore,
    added: &[SourceSnapshot],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    diagnostic
        .validate(sources, added, registry, budget)
        .map_err(report_error)
}
fn report_error(error: nepl3_core::diagnostic::validation::ReportValidationError) -> ReaderError {
    use nepl3_core::diagnostic::validation::ReportValidationError;
    match error {
        ReportValidationError::Stopped(r) => r.into(),
        ReportValidationError::Source(e) => e.into(),
        ReportValidationError::Schema(e) => e.into(),
        ReportValidationError::Metadata | ReportValidationError::Usage => {
            ReaderError::ProviderContract
        }
    }
}
pub(crate) fn accepted_report(
    accepted: &Report,
    sources: &SourceStore,
    added: &[SourceSnapshot],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    budget.poll()?;
    if !usage_at_least(budget.usage(), accepted.usage) {
        return Err(ReaderError::ProviderContract);
    }
    accepted
        .validate(sources, added, registry, budget)
        .map_err(report_error)
}
pub(crate) fn report(
    report: &Report,
    saved: Usage,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    budget.poll()?;
    if !usage_at_least(report.usage, saved)
        || !usage_at_least(budget.usage(), report.usage)
        || report.usage.diagnostics.saturating_sub(saved.diagnostics)
            < report.diagnostics.len() as u64
        || report.usage.events.saturating_sub(saved.events) < report.events.len() as u64
    {
        return Err(ReaderError::ProviderContract);
    }
    report
        .validate(sources, &[], registry, budget)
        .map_err(report_error)
}
#[allow(clippy::too_many_arguments)]
fn artifacts(
    machine: &ProviderBoundary<'_>,
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
    budget.poll()?;
    if maps.is_empty() && view.elements.is_empty() && view.roots.is_empty() && facts.is_empty() {
        // There is no new mapping or source-position use to validate. The
        // private checkpoint contains already accepted mappings; neither a
        // provider reply nor an echoed continuation can mutate that collector.
        // Source declarations, value/state, report and outcome checks remain
        // in check_provider. Nonempty artifacts still validate the full union,
        // including cycles introduced across old and new mappings.
        return Ok(());
    }
    let mut source_maps = Vec::new();
    for mapping in machine.current.source_maps.iter().chain(maps) {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<nepl3_core::origin::Mapping>() as u64,
        )?;
        source_maps.push(copy(mapping, budget)?);
    }
    let mapped = SourceMap::validate_mappings(&source_maps, sources, budget)?;
    view.validate_with_maps(sources, machine.registry, &mapped, budget)?;
    let consumed = machine.snapshot.span_with_budget(start, end, budget)?;
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
    if report.trace_overflow.is_some() && machine.current.trace_overflow.is_some() {
        return Err(ReaderError::ProviderContract);
    }
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
        machine.current.trace_overflow = Some(overflow);
    }
    Ok(())
}

fn combined(
    machine: &ProviderBoundary<'_>,
    added: &[SourceSnapshot],
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, ReaderError> {
    let mut combined = SourceStore::default();
    for source in core::iter::once(machine.snapshot)
        .chain(machine.declared.iter())
        .chain(&machine.current.sources)
    {
        combined.insert_ref_with_budget(source, budget)?;
    }
    for (index, source) in added.iter().enumerate() {
        for prior in &added[..index] {
            budget.charge(
                Resource::Work,
                (prior.identity().source.0.len() as u64)
                    .saturating_add(source.identity().source.0.len() as u64)
                    .saturating_add(34),
            )?;
        }
        if added[..index]
            .iter()
            .any(|s| s.identity() == source.identity())
        {
            return Err(ReaderError::ProviderContract);
        }
        if let Some(prior) = sources.get_revision_with_budget(
            &source.identity().source,
            source.identity().revision,
            budget,
        )? && !prior.eq_with_budget(source, budget)?
        {
            return Err(SourceError::IdentityConflict.into());
        }
        admission.admit_existing(source, budget)?;
        combined.insert_ref_with_budget(source, budget)?;
    }
    Ok(combined)
}
