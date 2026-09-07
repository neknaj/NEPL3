//! Pure declaration-order execution of the selected Grammar BindingPlan.
use crate::{
    package::{self, Binding, BindingId, FieldSpec, LanguagePackage, NameSelector},
    profile::ResolvedParseProfile,
    selection::{NodeSelection, ShapeSelection},
    tree::ValidatedParseTree,
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    facts::*,
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, Span},
    syntax::{FieldValue, NodeRef, SyntaxBundle, SyntaxNode},
    value::NdfValue,
};
mod host;
mod model;
pub use host::*;
mod prepare;
mod runtime;
mod scope;
pub use model::*;
#[derive(Debug, Eq, PartialEq)]
pub enum BindingError {
    Stopped(StopReason),
    Tree(crate::tree::TreeError),
    Profile(crate::profile::ProfileError),
    Fact(FactError),
    Source(nepl3_core::source::SourceError),
    Schema(nepl3_core::schema::SchemaError),
    Facts(crate::facts::FactsError),
    ProviderInvalid,
    AnalysisId,
    Target,
    Name,
    MissingNamespace,
    DuplicateGlobal,
    NamespaceBoundary,
    RecoveredTree,
    MissingProvider,
    UnsupportedPlan,
}
impl From<StopReason> for BindingError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
macro_rules! error_from {
    ($ty:ty,$case:ident) => {
        impl From<$ty> for BindingError {
            fn from(v: $ty) -> Self {
                type ErrorType = $ty;
                match v {
                    ErrorType::Stopped(reason) => Self::Stopped(reason),
                    value => Self::$case(value),
                }
            }
        }
    };
}
error_from!(crate::tree::TreeError, Tree);
error_from!(crate::profile::ProfileError, Profile);
error_from!(FactError, Fact);
error_from!(nepl3_core::source::SourceError, Source);
error_from!(nepl3_core::schema::SchemaError, Schema);
impl From<crate::facts::FactsError> for BindingError {
    fn from(value: crate::facts::FactsError) -> Self {
        match value.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Facts(value),
        }
    }
}
fn push<T>(values: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), BindingError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    values.push(value);
    Ok(())
}
fn text(value: &str, budget: &mut Budget) -> Result<String, BindingError> {
    budget.charge(Resource::Work, value.len() as u64 + 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}
struct LocalBundle<'a> {
    bundle: &'a SyntaxBundle,
    selections: &'a [NodeSelection],
    origin_base: u64,
    root: StageId,
}
struct Machine<'a, 'p> {
    profile: &'a ResolvedParseProfile<'p>,
    registry: &'a SchemaRegistry,
    bundles: Vec<LocalBundle<'a>>,
    progress: BindingProgress,
    report: Report,
}
/// Analyze one checked tree in a new analysis identity and the caller's shared
/// budget/admission operation. Concrete package selections are revalidated against
/// `profile`; a proof from another arena layout cannot reinterpret their indices.
pub fn analyze(
    analysis_id: &str,
    tree: &ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> BindingReply {
    analyze_inner(analysis_id, tree, profile, None, budget, admission)
}
/// Execute Custom plans with a host-selected provider and checked authority.
/// Ordinary analyze leaves unregistered Custom plans as MissingProvider.
pub fn analyze_with_host(
    analysis_id: &str,
    tree: &ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    host: &mut dyn BindingHost,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> BindingReply {
    analyze_inner(analysis_id, tree, profile, Some(host), budget, admission)
}
fn analyze_inner(
    analysis_id: &str,
    tree: &ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    mut host: Option<&mut dyn BindingHost>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> BindingReply {
    let mut machine = Machine {
        profile,
        registry: profile.registry(),
        bundles: Vec::new(),
        progress: BindingProgress::empty(),
        report: Report::default(),
    };
    let result = budget.with_depth(|budget| -> Result<(), BindingError> {
        budget.charge(Resource::Work, analysis_id.len() as u64 + 1)?;
        if analysis_id.is_empty() {
            return Err(BindingError::AnalysisId);
        }
        let analysis_id = text(analysis_id, budget)?;
        tree.tree().validate(profile, budget, admission)?;
        if tree.is_recovered() {
            return Err(BindingError::RecoveredTree);
        }
        let root = machine.prepare(tree.tree(), analysis_id, budget, admission)?;
        machine.run(root, tree.tree(), &mut host, budget, admission)?;
        machine
            .facts()?
            .validate(machine.registry, budget, admission)?;
        Ok(())
    });
    machine.report.usage = budget.usage();
    let outcome = match result {
        Ok(()) => match machine.progress.facts.take() {
            Some(facts) => BindingOutcome::Complete(BindingAnalysis {
                result: BindingResult {
                    facts,
                    sources: machine.progress.sources,
                    source_maps: machine.progress.source_maps,
                    stages: machine.progress.stages,
                    occurrence_stages: machine.progress.occurrence_stages,
                    open_inputs: machine.progress.open_inputs,
                    exports: machine.progress.exports,
                    resolution_history: machine.progress.resolution_history,
                },
            }),
            None => BindingOutcome::Invalid {
                error: BindingError::Target,
                progress: machine.progress,
            },
        },
        Err(BindingError::Stopped(reason)) => BindingOutcome::Stopped {
            reason: budget.stop(reason),
            progress: machine.progress,
        },
        Err(error) => BindingOutcome::Invalid {
            error,
            progress: machine.progress,
        },
    };
    BindingReply {
        outcome,
        report: machine.report,
    }
}

impl Machine<'_, '_> {
    fn entity(&self, id: EntityId, budget: &mut Budget) -> Result<&Entity, BindingError> {
        let values = &self.facts()?.entities;
        budget.charge(Resource::Work, 1)?;
        if let Some(value) = usize::try_from(id.0)
            .ok()
            .and_then(|i| values.get(i))
            .filter(|v| v.id == id)
        {
            return Ok(value);
        }
        budget.charge(Resource::Work, values.len() as u64)?;
        values
            .iter()
            .find(|v| v.id == id)
            .ok_or(BindingError::Target)
    }

    fn facts(&self) -> Result<&FactSet, BindingError> {
        self.progress.facts.as_ref().ok_or(BindingError::Target)
    }
    fn facts_mut(&mut self) -> Result<&mut FactSet, BindingError> {
        self.progress.facts.as_mut().ok_or(BindingError::Target)
    }
}

fn span(value: &Span, budget: &mut Budget) -> Result<Span, BindingError> {
    budget.charge(
        Resource::Work,
        value.snapshot_ref().source.0.len() as u64 + 41,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        value.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(value.clone())
}

// External FactDelta reservations permit sparse IDs. Vector position is only a
// checked fast path, never the meaning of an EntityId/ScopeId/OccurrenceId.
fn next_id<T>(
    values: &[T],
    id: impl Fn(&T) -> u64,
    budget: &mut Budget,
) -> Result<u64, BindingError> {
    budget.charge(Resource::Work, values.len() as u64 + 1)?;
    match values.iter().map(id).max() {
        None => Ok(0),
        Some(value) => value
            .checked_add(1)
            .ok_or(BindingError::Fact(FactError::Reservation)),
    }
}
