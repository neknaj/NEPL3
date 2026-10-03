//! Names visible at an actual reference's captured namespace stage.
//! These are declaration candidates, not spelling, insertion, or binding proofs.
use super::*;
use crate::binding::{BindingError, lookup};
use alloc::vec::Vec;
use nepl3_core::{
    diagnostic::Report,
    facts::{NamespaceRef, OccurrenceId, OccurrenceRole, ReferenceResolution},
};

pub struct ScopeCandidateRequest<'a> {
    pub key: AnalysisKey,
    pub occurrence: OccurrenceId,
    pub prefix: &'a str,
}
#[derive(Debug, Eq, PartialEq)]
pub enum CandidateError {
    Stopped(StopReason),
    Access(BindingAccessError),
    Binding(BindingError),
    NoOccurrence,
    NotReference,
    NoStage,
}
impl From<StopReason> for CandidateError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<BindingError> for CandidateError {
    fn from(error: BindingError) -> Self {
        match error {
            BindingError::Stopped(reason) => Self::Stopped(reason),
            other => Self::Binding(other),
        }
    }
}
pub struct NameCandidate {
    pub name: String,
    /// Hypothetical declaration lookup for this name at the captured stage,
    /// not the selected occurrence's possibly Custom-updated final resolution.
    pub resolution: ReferenceResolution,
}
pub struct ScopeCandidates {
    pub key: AnalysisKey,
    pub occurrence: OccurrenceId,
    pub namespace: NamespaceRef,
    pub candidates: Vec<NameCandidate>,
    pub report: Report,
}

/// The host selects an existing occurrence under its current analysis key.
/// Missing syntax, non-reference roles, partial binding, provider-specific name
/// proposals and edit generation are outside this declaration-only query.
/// Prefix matching is literal, without case folding or normalization. Names
/// follow first encounter order while walking the stage chain toward ancestors.
pub fn names(
    reply: &BoundBindingReply,
    request: &ScopeCandidateRequest<'_>,
    b: &mut Budget,
) -> Result<ScopeCandidates, CandidateError> {
    let (namespace, candidates) = b.with_depth(|b| {
        b.charge(Resource::Work, 128)?;
        if reply.key() != request.key {
            return Err(CandidateError::Access(BindingAccessError::StaleAnalysis));
        }
        let BindingOutcome::Complete(analysis) = &reply.reply().outcome else {
            return Err(CandidateError::Access(BindingAccessError::Incomplete));
        };
        let result = analysis.result();
        let mut occurrence = None;
        for value in &result.facts.occurrences {
            b.charge(Resource::Work, 1)?;
            if value.id == request.occurrence {
                occurrence = Some(value);
                break;
            }
        }
        let occurrence = occurrence.ok_or(CandidateError::NoOccurrence)?;
        if occurrence.role != OccurrenceRole::Reference {
            return Err(CandidateError::NotReference);
        }
        let mut point = None;
        for value in &result.occurrence_stages {
            b.charge(Resource::Work, 1)?;
            if value.occurrence == request.occurrence {
                point = Some(value.namespace_stage);
                break;
            }
        }
        let point = point.ok_or(CandidateError::NoStage)?;
        let candidates = collect_names(
            &result.stages,
            &result.facts,
            point,
            occurrence.namespace,
            request.prefix,
            b,
        )?;
        Ok((occurrence.namespace, candidates))
    })?;
    Ok(ScopeCandidates {
        key: request.key,
        occurrence: request.occurrence,
        namespace,
        candidates,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}

/// Internal enumeration over validated captured-stage data; callers own depth.
pub(super) fn collect_names(
    stages: &[crate::binding::BindingStage],
    facts: &nepl3_core::facts::FactSet,
    point: crate::binding::StageId,
    namespace: NamespaceRef,
    prefix: &str,
    b: &mut Budget,
) -> Result<Vec<NameCandidate>, CandidateError> {
    let mut stage = point;
    let mut candidates: Vec<NameCandidate> = Vec::new();
    loop {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        let current = lookup::stage_at(stages, stage)?;
        for id in &current.introduced {
            let entity = lookup::entity(facts, *id, b)?;
            b.charge(Resource::Work, prefix.len() as u64 + 1)?;
            if entity.namespace != namespace || !entity.name.starts_with(prefix) {
                continue;
            }
            let mut seen = false;
            for prior in &candidates {
                b.charge(Resource::Work, entity.name.len() as u64 + 1)?;
                if prior.name == entity.name {
                    seen = true;
                    break;
                }
            }
            if seen {
                continue;
            }
            // Reuse the same lookup as BindingPlan execution, preserving
            // nearest-scope shadowing and same-scope ambiguity.
            let resolution = lookup::resolve(stages, facts, point, namespace, &entity.name, b)?;
            b.charge(Resource::Work, entity.name.len() as u64 + 1)?;
            b.charge(
                Resource::AllocationUnits,
                entity.name.len() as u64 + (2 * core::mem::size_of::<NameCandidate>()) as u64,
            )?;
            candidates.push(NameCandidate {
                name: entity.name.clone(),
                resolution,
            });
        }
        match current.previous {
            Some(previous) => stage = previous,
            None => break,
        }
    }
    Ok(candidates)
}
