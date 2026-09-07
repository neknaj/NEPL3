use alloc::vec::Vec;
use nepl3_core::{
    budget::StopReason,
    diagnostic::Report,
    facts::{EntityId, FactSet, OccurrenceId, ScopeId},
    origin::Mapping,
    source::SourceSnapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageId(pub u64);
/// A persistent visibility point. `previous` may be a prior stage in the same
/// lexical scope or the parent's stage captured when this scope was entered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingStage {
    pub scope: ScopeId,
    pub previous: Option<StageId>,
    pub introduced: Vec<EntityId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OccurrenceStage {
    pub occurrence: OccurrenceId,
    pub stage: StageId,
}
/// Raw partial data; this is neither the structural FactSet proof nor a completed
/// name analysis. Before initialization, `facts` is absent; admitted sources
/// and maps remain available without constructing an invalid FactSet.
#[derive(Debug)]
pub struct BindingProgress {
    pub facts: Option<FactSet>,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
    pub stages: Vec<BindingStage>,
    pub occurrence_stages: Vec<OccurrenceStage>,
    pub open_inputs: Vec<OccurrenceId>,
    pub exports: Vec<EntityId>,
}
impl BindingProgress {
    pub(super) fn empty() -> Self {
        Self {
            facts: None,
            sources: Vec::new(),
            source_maps: Vec::new(),
            stages: Vec::new(),
            occurrence_stages: Vec::new(),
            open_inputs: Vec::new(),
            exports: Vec::new(),
        }
    }
}
/// Portable data retains resolutions but is not itself proof of visibility.
#[derive(Debug)]
pub struct BindingResult {
    pub facts: FactSet,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
    pub stages: Vec<BindingStage>,
    pub occurrence_stages: Vec<OccurrenceStage>,
    pub open_inputs: Vec<OccurrenceId>,
    pub exports: Vec<EntityId>,
}
/// Only execution of a validated plan can construct this semantic proof.
#[derive(Debug)]
pub struct BindingAnalysis {
    pub(super) result: BindingResult,
}
impl BindingAnalysis {
    pub fn result(&self) -> &BindingResult {
        &self.result
    }
    pub fn facts(&self) -> &FactSet {
        &self.result.facts
    }
    pub fn into_facts(self) -> FactSet {
        self.result.facts
    }
}
#[derive(Debug)]
pub enum BindingOutcome {
    Complete(BindingAnalysis),
    Invalid {
        error: super::BindingError,
        progress: BindingProgress,
    },
    Stopped {
        reason: StopReason,
        progress: BindingProgress,
    },
}
/// Every outcome retains the fact source/map closure used by its report, even
/// when preparation or publication stops before a semantic proof exists.
#[derive(Debug)]
pub struct BindingReply {
    pub outcome: BindingOutcome,
    pub report: Report,
}
impl BindingReply {
    pub fn facts(&self) -> Option<&FactSet> {
        match &self.outcome {
            BindingOutcome::Complete(v) => Some(v.facts()),
            BindingOutcome::Invalid { progress, .. } | BindingOutcome::Stopped { progress, .. } => {
                progress.facts.as_ref()
            }
        }
    }
    pub fn sources(&self) -> &[SourceSnapshot] {
        match &self.outcome {
            BindingOutcome::Complete(v) => &v.result.sources,
            BindingOutcome::Invalid { progress, .. } | BindingOutcome::Stopped { progress, .. } => {
                &progress.sources
            }
        }
    }
    pub fn source_maps(&self) -> &[Mapping] {
        match &self.outcome {
            BindingOutcome::Complete(v) => &v.result.source_maps,
            BindingOutcome::Invalid { progress, .. } | BindingOutcome::Stopped { progress, .. } => {
                &progress.source_maps
            }
        }
    }
}
