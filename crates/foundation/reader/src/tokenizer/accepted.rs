//! Native proof for a collector produced by the tokenizer. Raw wire values cannot forge it.
use super::*;
use alloc::{rc::Rc, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits},
    diagnostic::Report,
    origin::Mapping,
    source::SourceSnapshot,
};

#[derive(Debug)]
pub struct AcceptedTokenizationReport {
    pub(super) scope: Rc<TokenizationScope>,
    pub(super) report: Report,
    pub(super) sources: Vec<SourceSnapshot>,
    pub(super) source_maps: Vec<Mapping>,
    pub(super) limits: Limits,
}
impl AcceptedTokenizationReport {
    pub fn empty(
        scope: TokenizationScope,
        budget: &mut Budget,
    ) -> Result<Self, crate::runtime::ReaderError> {
        if scope.operation_id.is_empty() {
            return Err(crate::runtime::ReaderError::Context);
        }
        crate::runtime::copy::slot::<TokenizationScope>(budget)?;
        Ok(Self {
            scope: Rc::new(scope),
            report: Report {
                usage: budget.usage(),
                ..Report::default()
            },
            sources: Vec::new(),
            source_maps: Vec::new(),
            limits: budget.limits(),
        })
    }
    pub fn scope(&self) -> &TokenizationScope {
        &self.scope
    }
    /// Validate and append a complete operation report. `sources` explicitly
    /// declares its referenced snapshots; payload values grant no source access.
    /// The previous collector remains owned by `self` on every failed charge.
    pub fn append_report(
        &mut self,
        report: Report,
        saved: nepl3_core::budget::Usage,
        sources: &nepl3_core::source::SourceStore,
        registry: &nepl3_core::schema::SchemaRegistry,
        budget: &mut Budget,
        admission: &mut nepl3_core::source::SourceAdmission,
    ) -> Result<(), crate::runtime::ReaderError> {
        use nepl3_core::budget::Resource;
        if self.limits != budget.limits()
            || !crate::runtime::usage_at_least(budget.usage(), self.report.usage)
            || !crate::runtime::usage_at_least(saved, self.report.usage)
        {
            return Err(crate::runtime::ReaderError::Continuation);
        }
        let result = (|| {
            crate::runtime::validate::accepted_report(
                &report,
                sources,
                &self.sources,
                registry,
                budget,
            )?;
            if !crate::runtime::usage_at_least(report.usage, saved)
                || report.usage.diagnostics.saturating_sub(saved.diagnostics)
                    < report.diagnostics.len() as u64
                || report.usage.events.saturating_sub(saved.events) < report.events.len() as u64
            {
                return Err(crate::runtime::ReaderError::ProviderContract);
            }
            let mut captured = Vec::new();
            let spans = report
                .diagnostics
                .iter()
                .flat_map(|v| {
                    v.primary
                        .iter()
                        .chain(v.related.iter().filter_map(|v| v.span.as_ref()))
                        .chain(v.fixes.iter().flat_map(|v| v.edits.iter().map(|v| &v.span)))
                })
                .chain(report.events.iter().filter_map(|v| v.span.as_ref()));
            for span in spans {
                let mut found = None;
                for source in self.sources.iter().chain(sources.snapshots()) {
                    budget.charge(
                        Resource::Work,
                        (source.identity().source.0.len() as u64)
                            .saturating_add(span.snapshot_ref().source.0.len() as u64)
                            .saturating_add(34),
                    )?;
                    if source.identity() == span.snapshot_ref() {
                        found = Some(source);
                        break;
                    }
                }
                let source = found.ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
                admission.admit_existing(source, budget)?;
                let mut present = false;
                for prior in self.sources.iter().chain(&captured) {
                    budget.charge(
                        Resource::Work,
                        (prior.identity().source.0.len() as u64)
                            .saturating_add(source.identity().source.0.len() as u64)
                            .saturating_add(34),
                    )?;
                    if prior.identity() == source.identity() {
                        present = true;
                        break;
                    }
                }
                if !present {
                    captured.push(source.clone_with_budget(budget)?);
                }
            }
            let dropped = self
                .report
                .trace_overflow
                .as_ref()
                .map_or(0, |v| v.dropped)
                .checked_add(report.trace_overflow.as_ref().map_or(0, |v| v.dropped))
                .ok_or(crate::runtime::ReaderError::ProviderContract)?;
            budget.charge(
                Resource::AllocationUnits,
                (captured.len() as u64)
                    .saturating_mul(core::mem::size_of::<SourceSnapshot>() as u64)
                    .saturating_add((report.diagnostics.len() as u64).saturating_mul(
                        core::mem::size_of::<nepl3_core::diagnostic::Diagnostic>() as u64,
                    ))
                    .saturating_add((report.events.len() as u64).saturating_mul(
                        core::mem::size_of::<nepl3_core::diagnostic::Event>() as u64,
                    )),
            )?;
            self.sources.extend(captured);
            self.report.diagnostics.extend(report.diagnostics);
            self.report.events.extend(report.events);
            if dropped > 0 {
                self.report.trace_overflow =
                    Some(nepl3_core::diagnostic::TraceOverflow { dropped });
            }
            Ok(())
        })();
        if result.is_ok() {
            self.report.usage = budget.usage();
        }
        result
    }
    /// Explicitly admit the snapshots referenced by this diagnostic into this operation.
    /// `sources` supplies declarations chosen by the caller, including additional documents.
    /// Referenced snapshots are validated, charged once, and owned before publication.
    pub fn diagnostic(
        &mut self,
        diagnostic: nepl3_core::diagnostic::Diagnostic,
        sources: &nepl3_core::source::SourceStore,
        registry: &nepl3_core::schema::SchemaRegistry,
        budget: &mut Budget,
        admission: &mut nepl3_core::source::SourceAdmission,
    ) -> Result<(), crate::runtime::ReaderError> {
        if self.limits != budget.limits()
            || !crate::runtime::usage_at_least(budget.usage(), self.report.usage)
        {
            return Err(crate::runtime::ReaderError::Continuation);
        }
        let result = (|| {
            crate::runtime::validate::accepted_diagnostic(
                &diagnostic,
                sources,
                &self.sources,
                registry,
                budget,
            )?;
            let mut captured = Vec::new();
            for span in diagnostic
                .primary
                .iter()
                .chain(diagnostic.related.iter().filter_map(|v| v.span.as_ref()))
                .chain(
                    diagnostic
                        .fixes
                        .iter()
                        .flat_map(|fix| fix.edits.iter().map(|edit| &edit.span)),
                )
            {
                budget.charge(
                    nepl3_core::budget::Resource::Work,
                    ((sources.snapshots().len() + self.sources.len() + captured.len()) as u64)
                        .saturating_mul(span.snapshot_ref().source.0.len() as u64 + 33),
                )?;
                let source = self
                    .sources
                    .iter()
                    .chain(sources.snapshots())
                    .find(|source| source.identity() == span.snapshot_ref())
                    .ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
                admission.admit_existing(source, budget)?;
                if !self
                    .sources
                    .iter()
                    .chain(&captured)
                    .any(|prior: &SourceSnapshot| prior.identity() == source.identity())
                {
                    crate::runtime::copy::slot::<SourceSnapshot>(budget)?;
                    captured.push(crate::runtime::copy::copy(source, budget)?);
                }
            }
            budget.charge(
                nepl3_core::budget::Resource::AllocationUnits,
                captured.len() as u64 * core::mem::size_of::<SourceSnapshot>() as u64,
            )?;
            crate::runtime::copy::slot::<nepl3_core::diagnostic::Diagnostic>(budget)?;
            budget.charge(nepl3_core::budget::Resource::Diagnostics, 1)?;
            self.sources.extend(captured);
            self.report.diagnostics.push(diagnostic);
            Ok(())
        })();
        self.report.usage = budget.usage();
        result
    }
    /// Copy a rollback checkpoint without re-counting accepted diagnostics or source admission.
    pub fn checkpoint(&self, budget: &mut Budget) -> Result<Self, crate::runtime::ReaderError> {
        if self.limits != budget.limits()
            || !crate::runtime::usage_at_least(budget.usage(), self.report.usage)
        {
            return Err(crate::runtime::ReaderError::Continuation);
        }
        let mut report = crate::runtime::copy::copy(&self.report, budget)?;
        let sources = crate::runtime::copy::copy(&self.sources, budget)?;
        let source_maps = crate::runtime::copy::copy(&self.source_maps, budget)?;
        report.usage = budget.usage();
        Ok(Self {
            scope: Rc::clone(&self.scope),
            report,
            sources,
            source_maps,
            limits: self.limits,
        })
    }
    pub fn into_parts(self) -> (Report, Vec<SourceSnapshot>, Vec<Mapping>) {
        (self.report, self.sources, self.source_maps)
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
    pub fn sources(&self) -> &[SourceSnapshot] {
        &self.sources
    }
    pub fn source_maps(&self) -> &[Mapping] {
        &self.source_maps
    }
}
#[derive(Debug)]
pub struct AcceptedTokenizationReply {
    pub outcome: TokenizationOutcome,
    pub cursor: u64,
    pub new_state: Option<nepl3_core::value::NdfValue>,
    pub trivia: Vec<nepl3_core::view::Trivia>,
    pub facts: Vec<crate::model::ReaderFact>,
    pub accepted: AcceptedTokenizationReport,
}
impl AcceptedTokenizationReply {
    pub(super) fn from_native(
        reply: TokenizationReply,
        scope: Rc<TokenizationScope>,
        budget: &Budget,
    ) -> Self {
        Self {
            outcome: reply.outcome,
            cursor: reply.cursor,
            new_state: reply.new_state,
            trivia: reply.trivia,
            facts: reply.facts,
            accepted: AcceptedTokenizationReport {
                scope,
                report: reply.report,
                sources: reply.sources,
                source_maps: reply.source_maps,
                limits: budget.limits(),
            },
        }
    }
    pub fn into_raw(self) -> TokenizationReply {
        TokenizationReply {
            outcome: self.outcome,
            cursor: self.cursor,
            new_state: self.new_state,
            trivia: self.trivia,
            facts: self.facts,
            report: self.accepted.report,
            sources: self.accepted.sources,
            source_maps: self.accepted.source_maps,
        }
    }
}
