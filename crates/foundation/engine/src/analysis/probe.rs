//! Keyed native probe issuance and non-exhaustive checked position views.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest};
use crate::{
    binding::{
        BindingError,
        probe::{MissingReference, ProbeOutcome, ProbeReply},
    },
    recovery::ParseTree,
};
use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    source::{SourceAdmission, SourceError, SourceRef},
    syntax::canonical::{BundleMappings, CanonicalError},
};

pub struct BoundProbeReply<'a> {
    key: AnalysisKey,
    limits: Limits,
    tree: &'a ParseTree,
    reply: ProbeReply<'a>,
}
#[derive(Debug)]
pub enum ProbePosition<'a, 'tree> {
    HitAtPosition(&'a MissingReference<'tree>),
    /// The global first hit is elsewhere; later positions were not searched.
    OtherHit(&'a MissingReference<'tree>),
    NoHit,
    Blocked(&'a BindingError),
    Stopped(StopReason),
}
#[derive(Debug, Eq, PartialEq)]
pub enum ProbeAccessError {
    Access(BindingAccessError),
    Source(SourceError),
    Structure,
}
impl From<StopReason> for ProbeAccessError {
    fn from(reason: StopReason) -> Self {
        Self::Access(BindingAccessError::Stopped(reason))
    }
}
impl From<SourceError> for ProbeAccessError {
    fn from(error: SourceError) -> Self {
        match error {
            SourceError::Stopped(reason) => reason.into(),
            error => Self::Source(error),
        }
    }
}
impl PreparedBindingRequest<'_, '_> {
    /// There is intentionally no way to adopt a raw reply plus an asserted key.
    pub fn probe_missing_reference(
        &self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<BoundProbeReply<'_>, BindingAccessError> {
        if budget.limits() != self.limits {
            return Err(BindingAccessError::LimitsMismatch);
        }
        let reply = crate::binding::probe::first_missing_reference(
            self.analysis_id,
            &self.tree,
            self.profile,
            budget,
            admission,
        );
        Ok(BoundProbeReply {
            key: self.key,
            limits: self.limits,
            tree: self.tree.tree(),
            reply,
        })
    }
}
impl<'tree> BoundProbeReply<'tree> {
    pub fn key(&self) -> AnalysisKey {
        self.key
    }
    pub fn reply(&self) -> &ProbeReply<'tree> {
        &self.reply
    }
    /// Discards keyed evidence; raw outcomes and reports remain ordinary native data.
    pub fn into_reply(self) -> ProbeReply<'tree> {
        self.reply
    }

    /// Checks the entire retained source closure before returning a position view.
    /// A local gate stop takes precedence over returning any native outcome.
    /// Limits mismatch is checked before budget use. With matching limits,
    /// cancellation or resource exhaustion can precede stale-key classification.
    /// A continued stopped budget cannot validate the retained source closure;
    /// a fresh query budget/admission may do so as a separate operation.
    /// Query failure leaves the original native outcome and report unchanged.
    pub fn for_position(
        &self,
        expected: &AnalysisKey,
        source: &SourceRef,
        offset: u64,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ProbePosition<'_, 'tree>, ProbeAccessError> {
        if budget.limits() != self.limits {
            return Err(ProbeAccessError::Access(BindingAccessError::LimitsMismatch));
        }
        budget.with_depth(|budget| {
            budget.charge(Resource::Work, 128)?;
            if self.key != *expected {
                return Err(ProbeAccessError::Access(BindingAccessError::StaleAnalysis));
            }
            let mappings =
                BundleMappings::new(&self.tree.bundle, budget).map_err(|error| match error {
                    CanonicalError::Stopped(reason) => reason.into(),
                    _ => ProbeAccessError::Structure,
                })?;
            let mut found = false;
            for mapping in mappings.entries() {
                for snapshot in &mapping.bundle().sources {
                    // Identity and locator conflicts must be checked before deduplication.
                    admission.admit_existing(snapshot, budget)?;
                    let identity = snapshot.identity();
                    budget.charge(
                        Resource::Work,
                        (identity.source.0.len() as u64)
                            .saturating_add(source.source_id.0.len() as u64)
                            .saturating_add(40),
                    )?;
                    if identity.source == source.source_id
                        && identity.revision == source.revision
                        && identity.digest == source.digest
                    {
                        snapshot.check_range(offset, offset)?;
                        found = true;
                    }
                }
            }
            if !found {
                return Err(ProbeAccessError::Access(BindingAccessError::MissingSource));
            }
            Ok(match &self.reply.outcome {
                ProbeOutcome::Hit(hit) => {
                    let anchor = &hit.site().anchor;
                    let identity = anchor.snapshot_ref();
                    budget.charge(
                        Resource::Work,
                        (identity.source.0.len() as u64)
                            .saturating_add(source.source_id.0.len() as u64)
                            .saturating_add(40),
                    )?;
                    if identity.source == source.source_id
                        && identity.revision == source.revision
                        && identity.digest == source.digest
                        && anchor.start() == offset
                    {
                        ProbePosition::HitAtPosition(hit)
                    } else {
                        ProbePosition::OtherHit(hit)
                    }
                }
                ProbeOutcome::NoHit => ProbePosition::NoHit,
                ProbeOutcome::Blocked(error) => ProbePosition::Blocked(error),
                ProbeOutcome::Stopped(reason) => ProbePosition::Stopped(*reason),
            })
        })
    }
}
