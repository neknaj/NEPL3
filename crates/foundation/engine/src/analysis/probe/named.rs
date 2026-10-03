//! Keyed first-Missing prefix with coupled built-in Entity birth records.
use super::*;
use crate::{
    analysis::trace::named::EntityBirthAccessError,
    binding::trace::birth::{self, BirthAccessError, BirthLookup, EntityBirth},
    profile::ResolvedParseProfile,
};
use alloc::vec::Vec;
use nepl3_core::facts::EntityId;

pub struct BoundNamedProbe<'a, 'p> {
    probe: BoundProbeReply<'a>,
    births: Vec<EntityBirth>,
    profile: &'a ResolvedParseProfile<'p>,
}
impl PreparedBindingRequest<'_, '_> {
    pub fn probe_missing_reference_with_births(
        &self,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundNamedProbe<'_, '_>, BindingAccessError> {
        if b.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        let native = crate::binding::probe::named::first_missing_reference(
            self.analysis_id,
            &self.tree,
            self.profile,
            b,
            admission,
        );
        let (reply, births) = native.into_parts();
        Ok(BoundNamedProbe {
            probe: BoundProbeReply {
                key: self.key,
                limits: self.limits,
                tree: self.tree.tree(),
                reply,
            },
            births,
            profile: self.profile,
        })
    }
}
impl<'a, 'p> BoundNamedProbe<'a, 'p> {
    pub fn probe(&self) -> &BoundProbeReply<'a> {
        &self.probe
    }
    pub fn births(&self) -> &[EntityBirth] {
        &self.births
    }
    pub fn profile(&self) -> &'a ResolvedParseProfile<'p> {
        self.profile
    }
    pub fn entity_birth(
        &self,
        key: &AnalysisKey,
        id: EntityId,
        b: &mut Budget,
    ) -> Result<BirthLookup<'_>, EntityBirthAccessError> {
        if b.limits() != self.probe.limits {
            return Err(EntityBirthAccessError::Access(
                BindingAccessError::LimitsMismatch,
            ));
        }
        b.charge(Resource::Work, 128)
            .map_err(|s| EntityBirthAccessError::Access(BindingAccessError::Stopped(s)))?;
        if *key != self.probe.key {
            return Err(EntityBirthAccessError::Access(
                BindingAccessError::StaleAnalysis,
            ));
        }
        let ProbeOutcome::Hit(hit) = &self.probe.reply.outcome else {
            return Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete));
        };
        birth::lookup(
            hit.facts()
                .map_err(|_| EntityBirthAccessError::Birth(BirthAccessError::Facts))?,
            &self.births,
            id,
            b,
        )
        .map_err(EntityBirthAccessError::Birth)
    }
}
