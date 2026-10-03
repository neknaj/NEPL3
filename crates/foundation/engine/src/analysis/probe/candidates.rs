//! Declaration-only candidates at the exact keyed first missing Reference hit.
use super::{BoundProbeReply, ProbeAccessError, ProbePosition};
use crate::{
    analysis::{
        AnalysisKey,
        completion::{CandidateError, NameCandidate, collect_names},
    },
    binding::{BindingError, probe::MissingReference},
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, StopReason},
    diagnostic::Report,
    source::{SourceAdmission, SourceRef},
};

pub struct ProbeCandidateRequest<'a> {
    pub key: AnalysisKey,
    pub source: &'a SourceRef,
    pub offset: u64,
    pub prefix: &'a str,
}
pub enum ProbeCandidateOutcome<'a, 'tree> {
    Hit {
        hit: &'a MissingReference<'tree>,
        candidates: Vec<NameCandidate>,
    },
    OtherHit(&'a MissingReference<'tree>),
    NoHit,
    Blocked(&'a BindingError),
    Stopped(StopReason),
}
pub struct ProbeCandidates<'a, 'tree> {
    key: AnalysisKey,
    outcome: ProbeCandidateOutcome<'a, 'tree>,
    /// Cumulative query usage; the original native report remains unchanged.
    report: Report,
}
#[derive(Debug, Eq, PartialEq)]
pub enum ProbeCandidateError {
    Access(ProbeAccessError),
    Candidate(CandidateError),
}

/// Reuses ordinary declaration lookup at retained namespace_stage, without
/// inventing an occurrence, spelling, insertion, or completed Binding result.
/// OtherHit is non-exhaustive; only the exact first hit enumerates names.
pub fn names<'a, 'tree>(
    reply: &'a BoundProbeReply<'tree>,
    request: &ProbeCandidateRequest<'_>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ProbeCandidates<'a, 'tree>, ProbeCandidateError> {
    // The gate owns LimitsMismatch-before-budget precedence, even for outcomes
    // that have no candidate enumeration. Do not charge or enter depth first.
    let position = reply
        .for_position(
            &request.key,
            request.source,
            request.offset,
            budget,
            admission,
        )
        .map_err(ProbeCandidateError::Access)?;
    let outcome = match position {
        ProbePosition::HitAtPosition(hit) => {
            let candidates = budget
                .with_depth(|budget| {
                    let facts = hit.facts().map_err(CandidateError::from)?;
                    collect_names(
                        hit.stages(),
                        facts,
                        hit.site().namespace_stage,
                        hit.site().namespace,
                        request.prefix,
                        budget,
                    )
                })
                .map_err(ProbeCandidateError::Candidate)?;
            ProbeCandidateOutcome::Hit { hit, candidates }
        }
        ProbePosition::OtherHit(hit) => ProbeCandidateOutcome::OtherHit(hit),
        ProbePosition::NoHit => ProbeCandidateOutcome::NoHit,
        ProbePosition::Blocked(error) => ProbeCandidateOutcome::Blocked(error),
        ProbePosition::Stopped(reason) => ProbeCandidateOutcome::Stopped(reason),
    };
    Ok(ProbeCandidates {
        key: reply.key(),
        outcome,
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })
}

impl<'a, 'tree> ProbeCandidates<'a, 'tree> {
    pub fn key(&self) -> AnalysisKey {
        self.key
    }
    pub fn outcome(&self) -> &ProbeCandidateOutcome<'a, 'tree> {
        &self.outcome
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
