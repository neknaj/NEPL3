//! Host-free first-Missing prefix with same-run built-in Entity births.
use super::*;
use crate::binding::trace::birth::{self, BirthAccessError, BirthLookup, EntityBirth};
use nepl3_core::budget::Limits;

pub struct NamedProbeReply<'a, 'p> {
    probe: ProbeReply<'a>,
    births: Vec<EntityBirth>,
    profile: &'a ResolvedParseProfile<'p>,
    limits: Limits,
}
impl<'a, 'p> NamedProbeReply<'a, 'p> {
    pub fn probe(&self) -> &ProbeReply<'a> {
        &self.probe
    }
    pub fn births(&self) -> &[EntityBirth] {
        &self.births
    }
    pub fn profile(&self) -> &'a ResolvedParseProfile<'p> {
        self.profile
    }
    /// Only a successful Hit authenticates a validated visible prefix.
    pub fn entity_birth(
        &self,
        id: EntityId,
        b: &mut Budget,
    ) -> Result<BirthLookup<'_>, BirthAccessError> {
        if b.limits() != self.limits {
            return Err(BirthAccessError::LimitsMismatch);
        }
        b.charge(Resource::Work, 1)?;
        let ProbeOutcome::Hit(hit) = &self.probe.outcome else {
            return Err(BirthAccessError::Incomplete);
        };
        birth::lookup(
            hit.facts().map_err(|_| BirthAccessError::Facts)?,
            &self.births,
            id,
            b,
        )
    }
    pub(crate) fn into_parts(self) -> (ProbeReply<'a>, Vec<EntityBirth>) {
        (self.probe, self.births)
    }
}
pub fn first_missing_reference<'a, 'p>(
    analysis_id: &str,
    tree: &'a ValidatedParseTree<'_>,
    profile: &'a ResolvedParseProfile<'p>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> NamedProbeReply<'a, 'p> {
    let (probe, births) =
        super::first_missing_reference_inner(analysis_id, tree, profile, true, budget, admission);
    NamedProbeReply {
        probe,
        births,
        profile,
        limits: budget.limits(),
    }
}
