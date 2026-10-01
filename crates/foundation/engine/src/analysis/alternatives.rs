//! Declared grammar alternatives, not reader-validated completions or edits.
//! Returned form spellings require separate reader and insertion validation.
use super::{AnalysisKey, BindingAccessError, PreparedBindingRequest, expected::*};
use crate::{package::ReadSpec, profile::ProfileError};
use alloc::{boxed::Box, string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::{SourceAdmission, SourceSnapshot},
};
use nepl3_reader::builtin::BuiltinReader;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredFormAlternative {
    /// Index in the selected entry's concrete package, not its sorted lookup index.
    pub index: u64,
    pub spelling: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredReadAlternatives {
    Category {
        forms: Vec<DeclaredFormAlternative>,
        /// Declared leaves are not an inventory of accepted token spellings.
        leaf_declarations: u64,
        /// Registration presence only; possible dynamic heads are not enumerated.
        dynamic_fallback_registered: bool,
    },
    Builtin {
        reader: BuiltinReader,
    },
    /// The parser's fixed structural alternatives are `cons` and `nil`.
    /// Tokenization under the selected mode has not been executed here.
    List,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredAlternativeSet {
    pub read: Box<ExpectedRead>,
    pub alternatives: DeclaredReadAlternatives,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredAlternativesOutcome {
    Complete(Option<Box<DeclaredAlternativeSet>>),
    Invalid(ExpectedReadError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredAlternativesReply {
    pub key: AnalysisKey,
    pub outcome: DeclaredAlternativesOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}
pub fn declared_alternatives(
    input: &PreparedBindingRequest<'_, '_>,
    request: &ExpectedReadRequest,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> DeclaredAlternativesReply {
    // expected_read establishes exact limits before any charge or depth entry.
    let selected = expected_read(input, request, b, admission);
    let mut sources = selected.sources;
    let outcome = match selected.outcome {
        ExpectedReadOutcome::Invalid(error) => DeclaredAlternativesOutcome::Invalid(error),
        ExpectedReadOutcome::Stopped(reason) => DeclaredAlternativesOutcome::Stopped(reason),
        ExpectedReadOutcome::Complete(None) => DeclaredAlternativesOutcome::Complete(None),
        ExpectedReadOutcome::Complete(Some(read)) => {
            match b.with_depth(|b| enumerate(input, read, b)) {
                Ok(value) => DeclaredAlternativesOutcome::Complete(Some(value)),
                Err(error) => {
                    sources.clear();
                    match error.stop_reason() {
                        Some(reason) => DeclaredAlternativesOutcome::Stopped(reason),
                        None => DeclaredAlternativesOutcome::Invalid(error),
                    }
                }
            }
        }
    };
    DeclaredAlternativesReply {
        key: selected.key,
        outcome,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
        sources,
    }
}
fn profile_error(error: ProfileError) -> ExpectedReadError {
    match error {
        ProfileError::Stopped(reason) => reason.into(),
        _ => ExpectedReadError::Owner,
    }
}
fn enumerate(
    input: &PreparedBindingRequest<'_, '_>,
    read: Box<ExpectedRead>,
    b: &mut Budget,
) -> Result<Box<DeclaredAlternativeSet>, ExpectedReadError> {
    if b.limits() != input.limits {
        return Err(ExpectedReadError::Access(
            BindingAccessError::LimitsMismatch,
        ));
    }
    let entry = &read.expected;
    let package = input
        .profile
        .language(&entry.alias, b)
        .map_err(profile_error)?;
    let explicit = match read.origin {
        ExpectedReadOrigin::Root => None,
        ExpectedReadOrigin::Field { resolved_read, .. } => resolved_read,
    };
    let alternatives = if let Some(id) = explicit {
        b.charge(Resource::Work, 1)?;
        match package.read(id).map_err(|error| match error {
            crate::package::PackageError::Stopped(reason) => reason.into(),
            _ => ExpectedReadError::Owner,
        })? {
            ReadSpec::Builtin { reader, .. } => {
                DeclaredReadAlternatives::Builtin { reader: *reader }
            }
            ReadSpec::ListOf { .. } => DeclaredReadAlternatives::List,
            _ => return Err(ExpectedReadError::Owner),
        }
    } else {
        let mut forms = Vec::new();
        for (index, form) in package.forms.iter().enumerate() {
            b.charge(
                Resource::Work,
                (form.category.len() as u64)
                    .saturating_add(entry.category.len() as u64)
                    .saturating_add(1),
            )?;
            if form.category != entry.category {
                continue;
            }
            b.charge(Resource::Work, form.spelling.len() as u64)?;
            b.charge(
                Resource::AllocationUnits,
                (form.spelling.len() as u64)
                    .saturating_add(core::mem::size_of::<DeclaredFormAlternative>() as u64),
            )?;
            forms.push(DeclaredFormAlternative {
                index: index as u64,
                spelling: form.spelling.clone(),
            });
        }
        let mut leaves = 0u64;
        for leaf in &package.leaves {
            b.charge(
                Resource::Work,
                (leaf.category.len() as u64)
                    .saturating_add(entry.category.len() as u64)
                    .saturating_add(1),
            )?;
            if leaf.category == entry.category {
                leaves = leaves
                    .checked_add(1)
                    .ok_or_else(|| b.stop(StopReason::NodeLimit))?;
            }
        }
        let dynamic = input
            .profile
            .head_provider(&entry.alias, &entry.category, b)
            .map_err(profile_error)?
            .is_some();
        DeclaredReadAlternatives::Category {
            forms,
            leaf_declarations: leaves,
            dynamic_fallback_registered: dynamic,
        }
    };
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<DeclaredAlternativeSet>() as u64,
    )?;
    Ok(Box::new(DeclaredAlternativeSet { read, alternatives }))
}
