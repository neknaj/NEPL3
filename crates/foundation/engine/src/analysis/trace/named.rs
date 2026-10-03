//! Prepared-request issuance of one coupled Reference/Entity-birth execution.
use super::*;
use crate::binding::{
    BindingOutcome,
    trace::birth::{self, BirthAccessError, BirthLookup, EntityBirth},
};
use alloc::vec::Vec;
use nepl3_core::facts::EntityId;

pub struct BoundNamedTrace<'a, 'p> {
    references: BoundReferenceTrace<'a, 'p>,
    births: Vec<EntityBirth>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum EntityBirthAccessError {
    Access(BindingAccessError),
    Birth(BirthAccessError),
}
impl PreparedBindingRequest<'_, '_> {
    pub fn trace_named(
        &self,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundNamedTrace<'_, '_>, BindingAccessError> {
        self.trace_named_inner(None, b, admission)
    }
    pub fn trace_named_with_host(
        &self,
        host: &mut dyn BindingHost,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundNamedTrace<'_, '_>, BindingAccessError> {
        self.trace_named_inner(Some(host), b, admission)
    }
    fn trace_named_inner(
        &self,
        host: Option<&mut dyn BindingHost>,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundNamedTrace<'_, '_>, BindingAccessError> {
        if b.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        let native = crate::binding::trace::named::analyze(
            self.analysis_id,
            &self.tree,
            self.profile,
            host,
            b,
            admission,
        );
        let (trace, births) = native.into_parts();
        Ok(BoundNamedTrace {
            references: BoundReferenceTrace {
                key: self.key,
                limits: self.limits,
                trace,
            },
            births,
        })
    }
}
impl<'a, 'p> BoundNamedTrace<'a, 'p> {
    pub fn references(&self) -> &BoundReferenceTrace<'a, 'p> {
        &self.references
    }
    /// Raw metadata from exactly the same native execution as references().
    pub fn births(&self) -> &[EntityBirth] {
        &self.births
    }
    pub fn entity_birth(
        &self,
        key: &AnalysisKey,
        id: EntityId,
        b: &mut Budget,
    ) -> Result<BirthLookup<'_>, EntityBirthAccessError> {
        let native = self
            .references
            .for_key(key, b)
            .map_err(EntityBirthAccessError::Access)?;
        let BindingOutcome::Complete(analysis) = &native.reply().outcome else {
            return Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete));
        };
        birth::lookup(analysis.facts(), &self.births, id, b).map_err(EntityBirthAccessError::Birth)
    }
}
