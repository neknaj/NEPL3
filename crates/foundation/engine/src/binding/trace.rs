//! Native same-execution provenance for built-in Reference issuance.
//! This does not establish correspondence with a different analysis or accept edits.
use super::*;
use nepl3_core::{budget::Limits, source::Digest};

#[derive(Debug)]
pub struct ReferenceIssuance {
    pub owner: CanonicalBindingTarget,
    pub name_target: CanonicalBindingTarget,
    pub package: package::PackageIdentity,
    pub execution_digest: Digest,
    pub binding: BindingId,
    /// Run-local dispatch ordinal, never a cross-revision identity.
    pub execution_step: u64,
    pub occurrence: OccurrenceId,
    pub stage: StageId,
    pub namespace_stage: StageId,
    pub namespace: NamespaceRef,
}

/// Private immutable dependencies authenticate the association, including partial runs.
#[derive(Debug, Eq, PartialEq)]
pub enum TraceAccessError {
    LimitsMismatch,
    Incomplete,
    MissingRecord,
    Facts,
    Stopped(StopReason),
}
impl From<StopReason> for TraceAccessError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct FinalReference<'a> {
    pub occurrence: &'a Occurrence,
    pub open_input: bool,
}
pub struct ReferenceTrace<'a, 'p> {
    tree: &'a crate::recovery::ParseTree,
    profile: &'a ResolvedParseProfile<'p>,
    reply: BindingReply,
    rows: Vec<ReferenceIssuance>,
    limits: Limits,
}
impl ReferenceTrace<'_, '_> {
    pub fn tree(&self) -> &crate::recovery::ParseTree {
        self.tree
    }
    pub fn profile(&self) -> &ResolvedParseProfile<'_> {
        self.profile
    }
    pub fn reply(&self) -> &BindingReply {
        &self.reply
    }
    /// Prefix evidence only when the underlying execution is Invalid or Stopped.
    pub fn rows(&self) -> &[ReferenceIssuance] {
        &self.rows
    }
    /// Resolve an actual issued ID against this envelope's completed final facts.
    /// Sparse provider IDs and later authorized resolution changes stay intact.
    pub fn final_reference(
        &self,
        index: usize,
        budget: &mut Budget,
    ) -> Result<FinalReference<'_>, TraceAccessError> {
        if budget.limits() != self.limits {
            return Err(TraceAccessError::LimitsMismatch);
        }
        budget.with_depth(|budget| {
            budget.charge(Resource::Work, 1)?;
            let BindingOutcome::Complete(analysis) = &self.reply.outcome else {
                return Err(TraceAccessError::Incomplete);
            };
            let row = self
                .rows
                .get(index)
                .ok_or(TraceAccessError::MissingRecord)?;
            let mut found = None;
            for occurrence in &analysis.facts().occurrences {
                budget.charge(Resource::Nodes, 1)?;
                budget.charge(Resource::Work, 1)?;
                if occurrence.id == row.occurrence {
                    if found.is_some() || occurrence.role != OccurrenceRole::Reference {
                        return Err(TraceAccessError::Facts);
                    }
                    found = Some(occurrence);
                }
            }
            let occurrence = found.ok_or(TraceAccessError::Facts)?;
            let mut open_input = false;
            for id in &analysis.result().open_inputs {
                budget.charge(Resource::Work, 1)?;
                if *id == row.occurrence {
                    open_input = true;
                }
            }
            Ok(FinalReference {
                occurrence,
                open_input,
            })
        })
    }
    /// Complete native issuance evidence; final resolutions remain in these facts.
    pub fn complete(&self) -> Option<(&BindingAnalysis, &[ReferenceIssuance])> {
        match &self.reply.outcome {
            BindingOutcome::Complete(analysis) => Some((analysis, &self.rows)),
            _ => None,
        }
    }
}

pub fn analyze<'a, 'p>(
    analysis_id: &str,
    tree: &'a ValidatedParseTree<'_>,
    profile: &'a ResolvedParseProfile<'p>,
    host: Option<&mut dyn BindingHost>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> ReferenceTrace<'a, 'p> {
    let (reply, rows) =
        super::analyze_inner(analysis_id, tree, profile, host, true, budget, admission);
    ReferenceTrace {
        tree: tree.tree(),
        profile,
        reply,
        rows,
        limits: budget.limits(),
    }
}
