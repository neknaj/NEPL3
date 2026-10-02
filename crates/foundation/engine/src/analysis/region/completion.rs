//! Declaration candidates for references selected through structural regions.
//! This native operation does not generate edits or accept candidate spellings.
use super::*;
use crate::analysis::{
    BoundBindingReply,
    completion::{self as scope, CandidateError, ScopeCandidateRequest, ScopeCandidates},
};
use alloc::{boxed::Box, string::String};
use nepl3_core::facts::OccurrenceRole;

pub struct RegionCompletionRequest {
    pub region: RegionRequest,
    /// Literal prefix supplied by the host; no token-boundary inference.
    pub prefix: String,
}
#[derive(Debug)]
pub enum RegionCompletionError {
    Selection(query::RegionQueryError),
    Candidates(CandidateError),
}
impl From<query::RegionQueryError> for RegionCompletionError {
    fn from(value: query::RegionQueryError) -> Self {
        Self::Selection(value)
    }
}
impl From<StopReason> for RegionCompletionError {
    fn from(value: StopReason) -> Self {
        Self::Candidates(CandidateError::Stopped(value))
    }
}
impl RegionCompletionError {
    fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Selection(error) => error.stop_reason(),
            Self::Candidates(CandidateError::Stopped(reason)) => Some(*reason),
            _ => None,
        }
    }
}
pub enum RegionCompletionOutcome {
    Complete {
        region: Option<Box<SourceRegion>>,
        /// One group per matched Reference occurrence, in issuance order.
        /// Empty groups differ from a group with no visible declaration names.
        groups: Vec<ScopeCandidates>,
    },
    Invalid(RegionCompletionError),
    Stopped(StopReason),
}
pub struct RegionCompletionReply {
    pub key: RegionKey,
    pub capability: RegionCapability,
    pub outcome: RegionCompletionOutcome,
    pub report: Report,
    /// Snapshots used by structural selection and matched reference occurrences.
    /// Candidate groups refer to the
    /// same native binding analysis; they do not carry target locations.
    pub sources: Vec<SourceSnapshot>,
}
/// Reuses the same owner/mapping selection as definition/reference queries.
/// Missing syntax and non-reference regions yield no groups. Foreign roots and
/// multiple source correspondences remain separate; partial results are atomic.
pub fn names(
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    request: &RegionCompletionRequest,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> RegionCompletionReply {
    let mut sources = Vec::new();
    let result = b.with_depth(|b| {
        let selected =
            query::select_occurrences(input, binding, &request.region, &mut sources, b, admission)?;
        let mut groups = Vec::new();
        for occurrence in selected.occurrences {
            b.charge(Resource::Work, 1)?;
            if occurrence.role != OccurrenceRole::Reference {
                continue;
            }
            query::admit_occurrence_source(binding, &occurrence, &mut sources, b, admission)?;
            let candidate = scope::names(
                binding,
                &ScopeCandidateRequest {
                    key: request.region.key.analysis,
                    occurrence: occurrence.id,
                    prefix: &request.prefix,
                },
                b,
            )
            .map_err(RegionCompletionError::Candidates)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<ScopeCandidates>() as u64,
            )?;
            groups.push(candidate);
        }
        query::sort_sources(&mut sources, b).map_err(query::RegionQueryError::from)?;
        Ok::<_, RegionCompletionError>(RegionCompletionOutcome::Complete {
            region: selected.region,
            groups,
        })
    });
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            sources.clear();
            match error.stop_reason() {
                Some(reason) => RegionCompletionOutcome::Stopped(reason),
                None => RegionCompletionOutcome::Invalid(error),
            }
        }
    };
    RegionCompletionReply {
        key: request.region.key,
        capability: input.capability(),
        outcome,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
        sources,
    }
}
