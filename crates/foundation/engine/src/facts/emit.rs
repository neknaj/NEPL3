//! Native callbacks publish formal reports before returning their fact delta.
//! A stopped callback cannot turn its remaining unchecked delta into proof.
use super::FactsError;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{
        Diagnostic, Event, Report, TraceOverflow,
        validation::{DiagnosticSourceResolver, ReportValidationError, validate_event_metadata},
    },
    origin::{Mapping, SourceMap},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceError, SourceSnapshot, SourceStore, Span},
};

/// An operation-owned report sink. Only the binding executor constructs it;
/// providers can add diagnostics/events and explicitly declare their sources.
/// Accepted values are already in the operation's collector when a later
/// callback action stops. No provider-claimed Usage is absorbed.
pub struct FactsEmitter<'a> {
    registry: &'a SchemaRegistry,
    report: &'a mut Report,
    sources: &'a mut Vec<SourceSnapshot>,
    maps: &'a mut Vec<Mapping>,
    map_owner: &'a mut Vec<u64>,
    budget: &'a mut Budget,
    admission: &'a mut SourceAdmission,
}
struct Declared<'a>(&'a [SourceSnapshot]);
impl DiagnosticSourceResolver for Declared<'_> {
    fn slice<'a>(
        &'a self,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<&'a str, ReportValidationError> {
        for source in self.0 {
            budget.charge(
                Resource::Work,
                source.identity().source.0.len() as u64
                    + span.snapshot_ref().source.0.len() as u64
                    + 41,
            )?;
            if source.identity() == span.snapshot_ref() {
                return Ok(source.slice(span)?);
            }
        }
        Err(SourceError::MissingSnapshot.into())
    }
}
impl<'a> FactsEmitter<'a> {
    pub(crate) fn new(
        registry: &'a SchemaRegistry,
        report: &'a mut Report,
        sources: &'a mut Vec<SourceSnapshot>,
        maps: (&'a mut Vec<Mapping>, &'a mut Vec<u64>),
        budget: &'a mut Budget,
        admission: &'a mut SourceAdmission,
    ) -> Self {
        Self {
            registry,
            report,
            sources,
            maps: maps.0,
            map_owner: maps.1,
            budget,
            admission,
        }
    }
    /// All callback computation uses the same operation's sticky budget.
    pub fn budget(&mut self) -> &mut Budget {
        self.budget
    }
    /// A synchronous portable adapter may borrow the same operation ledger
    /// while converting a request/reply. This does not declare sources in the
    /// formal report; `source` and checked delta acceptance still do that.
    pub fn budget_and_admission(&mut self) -> (&mut Budget, &mut SourceAdmission) {
        (self.budget, self.admission)
    }

    /// Explicitly admit a diagnostic source. This grants no fact/scope authority.
    /// Repeated identical declarations do not duplicate storage or SourceBytes.
    pub fn source(&mut self, source: SourceSnapshot) -> Result<(), FactsError> {
        self.budget.poll()?;
        self.admission.admit_existing(&source, self.budget)?;
        for prior in self.sources.iter() {
            self.budget.charge(
                Resource::Work,
                prior.identity().source.0.len() as u64
                    + source.identity().source.0.len() as u64
                    + 41,
            )?;
            if prior.identity().source == source.identity().source
                && prior.identity().revision == source.identity().revision
            {
                self.budget.charge(
                    Resource::Work,
                    prior.uri().len() as u64 + source.uri().len() as u64 + 1,
                )?;
                if prior.identity() != source.identity() || prior.uri() != source.uri() {
                    return Err(SourceError::IdentityConflict.into());
                }
                return Ok(());
            }
        }
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<SourceSnapshot>() as u64,
        )?;
        self.sources.push(source);
        Ok(())
    }
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) -> Result<(), FactsError> {
        diagnostic.validate_with_sources(&Declared(self.sources), self.registry, self.budget)?;
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Diagnostic>() as u64,
        )?;
        self.budget.charge(Resource::Diagnostics, 1)?;
        self.report.diagnostics.push(diagnostic);
        self.report.usage = self.budget.usage();
        Ok(())
    }
    /// Publish a checked mapping independently of the later fact delta. Both
    /// endpoints must already be declared; cycles are checked with prior maps.
    pub fn source_map(&mut self, mapping: Mapping) -> Result<(), FactsError> {
        self.budget.poll()?;
        let mut store = SourceStore::default();
        for source in self.sources.iter() {
            self.admission.admit_existing(source, self.budget)?;
            store.insert(source.clone_with_budget(self.budget)?)?;
        }
        let mut maps = Vec::new();
        for prior in self.maps.iter() {
            self.budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Mapping>() as u64,
            )?;
            maps.push(prior.clone_with_budget(self.budget)?);
        }
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Mapping>() as u64,
        )?;
        maps.push(mapping.clone_with_budget(self.budget)?);
        SourceMap::validate_mappings(&maps, &store, self.budget)?;
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Mapping>() as u64,
        )?;
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<u64>() as u64,
        )?;
        self.map_owner.push(self.maps.len() as u64);
        self.maps.push(mapping);
        Ok(())
    }
    pub fn event(&mut self, event: Event) -> Result<(), FactsError> {
        validate_event_metadata(
            &event.schema,
            &event.kind,
            &event.payload,
            self.registry,
            self.budget,
        )?;
        if let Some(span) = &event.span {
            Declared(self.sources).slice(span, self.budget)?;
        }
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Event>() as u64,
        )?;
        if let Err(reason) = self.budget.charge(Resource::Events, 1) {
            if reason == StopReason::EventLimit {
                self.report.trace_overflow = Some(TraceOverflow { dropped: 1 });
            }
            return Err(reason.into());
        }
        self.report.events.push(event);
        self.report.usage = self.budget.usage();
        Ok(())
    }
}
