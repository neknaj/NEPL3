//! Direct stored-source Missing-read selection. This does not generate edits.
//! Portable Complete replies are recomputed; failure reports are not execution proofs.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest};
use crate::package::{EntryContext, PackageIdentity, ReadSpecId};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::{SourceAdmission, SourceError, SourceRef, SourceSnapshot},
};
mod run;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadRequest {
    pub key: AnalysisKey,
    pub source: SourceRef,
    pub offset: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedReadStep {
    Child { field: u64 },
    Foreign { field: u64 },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedReadOrigin {
    Root,
    Field {
        parent_bundle: u64,
        parent_node: u64,
        field: u64,
        owner: PackageIdentity,
        declared: ReadSpecId,
        resolved_read: Option<ReadSpecId>,
        foreign: bool,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedRead {
    pub bundle: u64,
    pub node: u64,
    pub path: Vec<ExpectedReadStep>,
    pub expected: EntryContext,
    pub origin: ExpectedReadOrigin,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedReadError {
    Access(BindingAccessError),
    Source(SourceError),
    Owner,
}
impl From<StopReason> for ExpectedReadError {
    fn from(value: StopReason) -> Self {
        Self::Access(BindingAccessError::Stopped(value))
    }
}
impl From<SourceError> for ExpectedReadError {
    fn from(value: SourceError) -> Self {
        Self::Source(value)
    }
}
impl ExpectedReadError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Access(BindingAccessError::Stopped(value))
            | Self::Source(SourceError::Stopped(value)) => Some(*value),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedReadOutcome {
    Complete(Option<Box<ExpectedRead>>),
    Invalid(ExpectedReadError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedReadReply {
    pub key: AnalysisKey,
    pub outcome: ExpectedReadOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}
pub fn expected_read(
    input: &PreparedBindingRequest<'_, '_>,
    request: &ExpectedReadRequest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> ExpectedReadReply {
    if budget.limits() != input.limits {
        return ExpectedReadReply {
            key: request.key,
            outcome: ExpectedReadOutcome::Invalid(ExpectedReadError::Access(
                BindingAccessError::LimitsMismatch,
            )),
            report: Report {
                usage: budget.usage(),
                ..Report::default()
            },
            sources: Vec::new(),
        };
    }
    let mut sources = Vec::new();
    let outcome = match budget
        .with_depth(|budget| run::run(input, request, &mut sources, budget, admission))
    {
        Ok(value) => ExpectedReadOutcome::Complete(value),
        Err(error) => {
            sources.clear();
            match error.stop_reason() {
                Some(reason) => ExpectedReadOutcome::Stopped(reason),
                None => ExpectedReadOutcome::Invalid(error),
            }
        }
    };
    ExpectedReadReply {
        key: request.key,
        outcome,
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
        sources,
    }
}
fn push<T>(out: &mut Vec<T>, value: T, budget: &mut Budget) -> Result<(), ExpectedReadError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    out.push(value);
    Ok(())
}
