//! A reply may refer only to the exact windows retained with its pending call.
//! Semantic payload records never participate in source resolution.
use super::{
    HeadCall, HeadError, HeadRequest, ProjectedDiagnostic, ProjectedReport, ProjectedSpan,
    SourceWindow,
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::{
        Diagnostic, Event, Fix, Related, Report,
        validation::{DiagnosticSourceResolver, ReportValidationError},
    },
    schema::SchemaRegistry,
    source::{SourceError, SourceSnapshot, Span, TextEdit},
};

impl HeadCall {
    pub fn windows(&self) -> impl Iterator<Item = &SourceWindow> {
        let children = match &self.request {
            HeadRequest::Shape => &[][..],
            HeadRequest::ChildContext { completed, .. } => &completed.windows[..],
        };
        core::iter::once(&self.head.window).chain(children)
    }
    /// Independent providers can resolve ranges with the projected bytes only.
    /// This does not construct a core Span or admit any additional source.
    pub fn slice<'a>(
        &'a self,
        span: &ProjectedSpan,
        budget: &mut Budget,
    ) -> Result<&'a str, HeadError> {
        for window in self.windows() {
            if let Some(text) = window.slice(span, budget)? {
                return Ok(text);
            }
        }
        Err(HeadError::Projection)
    }
    fn restore_span(
        &self,
        span: &ProjectedSpan,
        sources: &[&[SourceSnapshot]],
        budget: &mut Budget,
    ) -> Result<Span, HeadError> {
        let projected = self.slice(span, budget)?;
        for source in sources.iter().flat_map(|v| v.iter()) {
            budget.charge(
                Resource::Work,
                (source.identity().source.0.len() as u64)
                    .saturating_add(span.source.source_id.0.len() as u64)
                    .saturating_add(34),
            )?;
            if source.identity().source == span.source.source_id
                && source.identity().revision == span.source.revision
                && source.identity().digest == span.source.digest
            {
                let actual = source.slice_range(span.start, span.end)?;
                budget.charge(Resource::Work, projected.len() as u64)?;
                if actual != projected {
                    return Err(HeadError::Projection);
                }
                return Ok(source.span_with_budget(span.start, span.end, budget)?);
            }
        }
        Err(SourceError::MissingSnapshot.into())
    }
    fn diagnostic(
        &self,
        value: ProjectedDiagnostic,
        sources: &[&[SourceSnapshot]],
        budget: &mut Budget,
    ) -> Result<Diagnostic, HeadError> {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Diagnostic>() as u64,
        )?;
        let primary = value
            .primary
            .as_ref()
            .map(|v| self.restore_span(v, sources, budget))
            .transpose()?;
        let mut related = Vec::new();
        for value in value.related {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Related>() as u64,
            )?;
            related.push(Related {
                span: value
                    .span
                    .as_ref()
                    .map(|v| self.restore_span(v, sources, budget))
                    .transpose()?,
                code: value.code,
                arguments: value.arguments,
            });
        }
        let mut fixes = Vec::new();
        for value in value.fixes {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Fix>() as u64,
            )?;
            let mut edits = Vec::new();
            for value in value.edits {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<TextEdit>() as u64,
                )?;
                edits.push(TextEdit {
                    span: self.restore_span(&value.span, sources, budget)?,
                    expected_digest: value.expected_digest,
                    replacement: value.replacement,
                });
            }
            fixes.push(Fix {
                id: value.id,
                edits,
            });
        }
        Ok(Diagnostic {
            schema: value.schema,
            code: value.code,
            severity: value.severity,
            stage: value.stage,
            arguments: value.arguments,
            primary,
            related,
            fixes,
        })
    }
    /// Convert a provider's projected positions using explicitly supplied host
    /// snapshots, then run the common report rules against the pending windows.
    /// Returning a Report does not absorb its Usage or accept it into a collector.
    pub fn restore_report(
        &self,
        value: ProjectedReport,
        sources: &[&[SourceSnapshot]],
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<Report, HeadError> {
        let mut diagnostics = Vec::new();
        for value in value.diagnostics {
            diagnostics.push(self.diagnostic(value, sources, budget)?);
        }
        let mut events = Vec::new();
        for value in value.events {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Event>() as u64,
            )?;
            events.push(Event {
                schema: value.schema,
                kind: value.kind,
                operation_path: value.operation_path,
                span: value
                    .span
                    .as_ref()
                    .map(|v| self.restore_span(v, sources, budget))
                    .transpose()?,
                payload: value.payload,
            });
        }
        let report = Report {
            diagnostics,
            events,
            trace_overflow: value.trace_overflow,
            usage: value.usage,
        };
        report.validate_with_sources(&WindowResolver(self), registry, budget)?;
        Ok(report)
    }
}

struct WindowResolver<'a>(&'a HeadCall);
impl DiagnosticSourceResolver for WindowResolver<'_> {
    fn slice<'a>(
        &'a self,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<&'a str, ReportValidationError> {
        // Borrowed comparison avoids cloning an arbitrarily long source ID just
        // to check a provider diagnostic or edit.
        for window in self.0.windows() {
            budget.charge(
                Resource::Work,
                (window.span.source.source_id.0.len() as u64)
                    .saturating_add(span.snapshot_ref().source.0.len() as u64)
                    .saturating_add(34),
            )?;
            if window.span.source.source_id == span.snapshot_ref().source
                && window.span.source.revision == span.snapshot_ref().revision
                && window.span.source.digest == span.snapshot_ref().digest
                && span.start() >= window.span.start
                && span.end() <= window.span.end
            {
                budget.charge(Resource::Work, window.bytes.len() as u64)?;
                let text = core::str::from_utf8(&window.bytes)
                    .map_err(|_| ReportValidationError::Metadata)?;
                let start = usize::try_from(span.start() - window.span.start)
                    .map_err(|_| SourceError::Bounds)?;
                let end = usize::try_from(span.end() - window.span.start)
                    .map_err(|_| SourceError::Bounds)?;
                return text
                    .get(start..end)
                    .ok_or(SourceError::ScalarBoundary.into());
            }
        }
        Err(SourceError::MissingSnapshot.into())
    }
}
