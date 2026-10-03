//! Host-free prefix execution up to the first missing Reference name field.
//! This operation never constructs complete Binding or editing authority.
use super::*;
use alloc::boxed::Box;

#[derive(Debug)]
pub struct MissingReferenceSite {
    pub owner: CanonicalBindingTarget,
    pub name_target: CanonicalBindingTarget,
    pub binding: BindingId,
    /// Run-local frame-dispatch ordinal, including restore and completion actions.
    /// It is not a portable semantic identifier.
    pub execution_step: u64,
    pub stage: StageId,
    pub namespace_stage: StageId,
    pub namespace: NamespaceRef,
    pub anchor: Span,
}

/// Native prefix evidence. Stage indices stay tied to these private dependencies.
#[derive(Debug)]
pub struct MissingReference<'a> {
    tree: &'a crate::recovery::ParseTree,
    site: MissingReferenceSite,
    progress: BindingProgress,
}
impl MissingReference<'_> {
    pub(crate) fn facts(&self) -> Result<&FactSet, BindingError> {
        self.progress.facts.as_ref().ok_or(BindingError::Target)
    }
    pub fn tree(&self) -> &crate::recovery::ParseTree {
        self.tree
    }
    pub fn site(&self) -> &MissingReferenceSite {
        &self.site
    }
    pub fn stages(&self) -> &[BindingStage] {
        &self.progress.stages
    }
    pub fn namespace(&self) -> Option<&FactNamespace> {
        self.progress
            .facts
            .as_ref()?
            .namespaces
            .get(self.site.namespace.0 as usize)
    }
    pub fn sources(&self) -> &[nepl3_core::source::SourceSnapshot] {
        &self.progress.sources
    }
}
#[derive(Debug)]
pub enum ProbeOutcome<'a> {
    Hit(Box<MissingReference<'a>>),
    NoHit,
    Blocked(BindingError),
    Stopped(StopReason),
}
#[derive(Debug)]
pub struct ProbeReply<'a> {
    pub outcome: ProbeOutcome<'a>,
    pub report: Report,
    progress: BindingProgress,
}
impl ProbeReply<'_> {
    pub fn sources(&self) -> &[nepl3_core::source::SourceSnapshot] {
        match &self.outcome {
            ProbeOutcome::Hit(hit) => hit.sources(),
            _ => &self.progress.sources,
        }
    }
}

/// "First" is action execution order, not syntax DFS or source offset order.
/// An earlier executed recovery or Custom blocks. Ignored recovery may remain.
pub fn first_missing_reference<'a>(
    analysis_id: &str,
    tree: &'a ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> ProbeReply<'a> {
    let mut machine = Machine {
        profile,
        registry: profile.registry(),
        bundles: Vec::new(),
        progress: BindingProgress::empty(),
        report: Report::default(),
    };
    let result = budget.with_depth(|budget| {
        budget.charge(Resource::Work, analysis_id.len() as u64 + 1)?;
        if analysis_id.is_empty() {
            return Err(BindingError::AnalysisId);
        }
        let analysis_id = text(analysis_id, budget)?;
        tree.tree().validate(profile, budget, admission)?;
        let root = machine.prepare(tree.tree(), analysis_id, budget, admission)?;
        let site = machine.run_probe(root, tree.tree(), budget, admission)?;
        if site.is_some() {
            machine
                .facts()?
                .validate(machine.registry, budget, admission)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<MissingReference>() as u64,
            )?;
        }
        Ok(site)
    });
    let outcome = match result {
        Ok(Some(site)) => ProbeOutcome::Hit(Box::new(MissingReference {
            tree: tree.tree(),
            site,
            progress: core::mem::replace(&mut machine.progress, BindingProgress::empty()),
        })),
        Ok(None) => ProbeOutcome::NoHit,
        Err(BindingError::Stopped(reason)) => ProbeOutcome::Stopped(budget.stop(reason)),
        Err(error) => ProbeOutcome::Blocked(error),
    };
    machine.report.usage = budget.usage();
    ProbeReply {
        outcome,
        report: machine.report,
        progress: machine.progress,
    }
}
