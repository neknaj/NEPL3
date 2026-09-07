//! Borrowed report validation. Resource accounting of a remote operation remains
//! an operation-boundary obligation; a report's claimed Usage is never absorbed.
use super::{Diagnostic, Event, Report};
use crate::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SourceError, SourceSnapshot, SourceStore, Span},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportValidationError {
    Stopped(StopReason),
    Source(SourceError),
    Schema(SchemaError),
    Metadata,
    Usage,
}
impl From<StopReason> for ReportValidationError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SourceError> for ReportValidationError {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(r) => Self::Stopped(r),
            v => Self::Source(v),
        }
    }
}
impl From<SchemaError> for ReportValidationError {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(r) => Self::Stopped(r),
            v => Self::Schema(v),
        }
    }
}

/// Host-selected resolver for exact, already authorized source ranges. A window
/// resolver must reject spans outside its explicit windows and must not treat a
/// claimed full-snapshot digest as proof of bytes it was not given.
pub trait DiagnosticSourceResolver {
    fn slice<'a>(
        &'a self,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<&'a str, ReportValidationError>;
}
fn check_span(
    sources: &impl DiagnosticSourceResolver,
    span: &Span,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    sources.slice(span, b)?;
    Ok(())
}
struct Sources<'a> {
    store: &'a SourceStore,
    added: &'a [SourceSnapshot],
}
impl<'a> Sources<'a> {
    fn new(
        store: &'a SourceStore,
        added: &'a [SourceSnapshot],
        b: &mut Budget,
    ) -> Result<Self, ReportValidationError> {
        b.charge(Resource::Work, 1)?;
        for (i, source) in added.iter().enumerate() {
            for prior in store.snapshots().iter().chain(&added[..i]) {
                b.charge(
                    Resource::Work,
                    (source.identity().source.0.len() as u64)
                        .saturating_add(prior.identity().source.0.len() as u64)
                        .saturating_add(34),
                )?;
                if prior.identity().source == source.identity().source
                    && prior.identity().revision == source.identity().revision
                {
                    b.charge(
                        Resource::Work,
                        (prior.uri().len() as u64)
                            .saturating_add(source.uri().len() as u64)
                            .saturating_add(1),
                    )?;
                    if prior.identity() != source.identity() || prior.uri() != source.uri() {
                        return Err(SourceError::IdentityConflict.into());
                    }
                }
            }
        }
        Ok(Self { store, added })
    }
}
impl DiagnosticSourceResolver for Sources<'_> {
    fn slice<'a>(&'a self, span: &Span, b: &mut Budget) -> Result<&'a str, ReportValidationError> {
        for source in self.store.snapshots().iter().chain(self.added) {
            b.charge(
                Resource::Work,
                (span.snapshot_ref().source.0.len() as u64)
                    .saturating_add(source.identity().source.0.len() as u64)
                    .saturating_add(34),
            )?;
            if source.identity() == span.snapshot_ref() {
                return Ok(source.slice(span)?);
            }
        }
        Err(SourceError::MissingSnapshot.into())
    }
}
fn diagnostic(
    value: &Diagnostic,
    sources: &impl DiagnosticSourceResolver,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    validate_diagnostic_metadata(
        &value.schema,
        &value.code,
        &value.stage,
        &value.arguments,
        registry,
        b,
    )?;
    if let Some(span) = &value.primary {
        check_span(sources, span, b)?;
    }
    for related in &value.related {
        b.charge(Resource::Work, 1)?;
        registry.validate_typed(&related.arguments, b)?;
        if let Some(span) = &related.span {
            check_span(sources, span, b)?;
        }
    }
    for fix in &value.fixes {
        b.charge(Resource::Work, 1)?;
        for (index, edit) in fix.edits.iter().enumerate() {
            let expected = sources.slice(&edit.span, b)?;
            b.charge(Resource::Work, expected.len() as u64)?;
            if Digest::of(expected.as_bytes()) != edit.expected_digest {
                return Err(SourceError::ExpectedDigest.into());
            }
            // Preserve the advertised edit order without an allocation for sorting.
            // The transaction applies a per-snapshot sorted copy; duplicate starts
            // (including insertion anchors) and intersecting ranges cannot coexist.
            for prior in &fix.edits[..index] {
                b.charge(
                    Resource::Work,
                    (prior.span.snapshot_ref().source.0.len() as u64)
                        .saturating_add(edit.span.snapshot_ref().source.0.len() as u64)
                        .saturating_add(34),
                )?;
                if prior.span.snapshot_ref().source == edit.span.snapshot_ref().source
                    && prior.span.snapshot_ref() != edit.span.snapshot_ref()
                {
                    return Err(SourceError::SnapshotMismatch.into());
                }
                if prior.span.snapshot_ref() == edit.span.snapshot_ref()
                    && (prior.span.start() == edit.span.start()
                        || (prior.span.start() < edit.span.end()
                            && edit.span.start() < prior.span.end()))
                {
                    return Err(SourceError::OverlappingEdits.into());
                }
            }
        }
    }
    Ok(())
}
fn event(
    value: &Event,
    sources: &impl DiagnosticSourceResolver,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    validate_event_metadata(&value.schema, &value.kind, &value.payload, registry, b)?;
    if let Some(span) = &value.span {
        check_span(sources, span, b)?;
    }
    Ok(())
}
impl Diagnostic {
    /// Same typed/Fix validation with a host-selected range resolver. This does
    /// not confer source access, authenticate Usage, or absorb remote charges.
    pub fn validate_with_sources(
        &self,
        sources: &impl DiagnosticSourceResolver,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<(), ReportValidationError> {
        diagnostic(self, sources, registry, b)
    }
    /// Validate against explicit declarations. This does not clone, admit sources,
    /// charge Diagnostics, or prove a domain-specific diagnostic's truth.
    pub fn validate(
        &self,
        declared: &SourceStore,
        added: &[SourceSnapshot],
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<(), ReportValidationError> {
        diagnostic(self, &Sources::new(declared, added, b)?, registry, b)
    }
}
impl Report {
    /// Checks typed data, declared-source geometry and internal count consistency.
    /// The caller separately binds Usage to its authenticated operation history.
    pub fn validate(
        &self,
        declared: &SourceStore,
        added: &[SourceSnapshot],
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<(), ReportValidationError> {
        let sources = Sources::new(declared, added, b)?;
        self.validate_with_sources(&sources, registry, b)
    }
    /// Uses the same Report and Fix rules for a host-authorized range resolver.
    /// Empty reports still poll the shared operation budget.
    pub fn validate_with_sources(
        &self,
        sources: &impl DiagnosticSourceResolver,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<(), ReportValidationError> {
        b.charge(Resource::Work, 1)?;
        validate_report_usage(
            self.usage,
            self.diagnostics.len(),
            self.events.len(),
            self.trace_overflow.as_ref(),
        )?;
        for value in &self.diagnostics {
            diagnostic(value, sources, registry, b)?;
        }
        for value in &self.events {
            event(value, sources, registry, b)?;
        }
        Ok(())
    }
}

/// Position-independent diagnostic rules, shared by full-snapshot diagnostics
/// and protocols whose separately checked positions are only projected claims.
pub fn validate_diagnostic_metadata(
    schema: &crate::value::SchemaRef,
    code: &str,
    stage: &str,
    arguments: &crate::value::TypedValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    b.charge(Resource::Work, schema.package.len() as u64 + 34)?;
    if code.is_empty() || stage.is_empty() || registry.descriptor(schema).is_none() {
        return Err(ReportValidationError::Metadata);
    }
    registry.validate_typed(arguments, b)?;
    Ok(())
}
/// Shared event rules excluding the protocol-specific position resolver.
pub fn validate_event_metadata(
    schema: &crate::value::SchemaRef,
    kind: &str,
    payload: &crate::value::TypedValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ReportValidationError> {
    b.charge(Resource::Work, schema.package.len() as u64 + 34)?;
    if kind.is_empty() || registry.descriptor(schema).is_none() {
        return Err(ReportValidationError::Metadata);
    }
    registry.validate_typed(payload, b)?;
    Ok(())
}
/// Internal count consistency only; this neither authenticates nor absorbs Usage.
pub fn validate_report_usage(
    usage: crate::budget::Usage,
    diagnostics: usize,
    events: usize,
    overflow: Option<&super::TraceOverflow>,
) -> Result<(), ReportValidationError> {
    if usage.diagnostics < diagnostics as u64
        || usage.events < events as u64
        || overflow.is_some_and(|v| v.dropped == 0)
    {
        return Err(ReportValidationError::Usage);
    }
    Ok(())
}
