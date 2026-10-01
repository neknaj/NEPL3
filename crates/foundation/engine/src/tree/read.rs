//! Resolve one incoming field using the same rules as persistent-tree validation.
//! This is an internal building block, not a completion or insertion API.
use super::TreeError;
use crate::{
    package::{EntryContext, ReadSpec},
    profile::{ResolvedParseProfile, ResolvedRead},
    selection::{NodeSelection, ShapeSelection},
};
use nepl3_core::budget::{Budget, Resource};

pub(crate) fn entry_copy(
    entry: &EntryContext,
    budget: &mut Budget,
) -> Result<EntryContext, TreeError> {
    let bytes = entry.alias.len()
        + entry.category.len()
        + entry.mode.len()
        + entry.package.schema.package.len();
    budget.charge(Resource::Work, bytes as u64 + 80)?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    Ok(entry.clone())
}

/// Original parent-owned declaration, before WithMode/category resolution.
pub(crate) fn declared(
    selected: &NodeSelection,
    field: usize,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
) -> Result<crate::package::ReadSpecId, TreeError> {
    budget.charge(Resource::Work, 1)?;
    let package = profile.language(&selected.entry.alias, budget)?;
    match &selected.shape {
        ShapeSelection::Form { index } => package
            .forms
            .get(usize::try_from(*index).map_err(|_| TreeError::Selection)?)
            .and_then(|form| form.fields.get(field))
            .map(|field| field.read)
            .ok_or(TreeError::Selection),
        ShapeSelection::Dynamic { shape, .. } => shape
            .fields
            .get(field)
            .map(|field| field.read)
            .ok_or(TreeError::Selection),
        ShapeSelection::List { read, cons: true } => {
            let ReadSpec::ListOf { element, .. } = package.read(*read)? else {
                return Err(TreeError::Selection);
            };
            match field {
                0 => Ok(*element),
                1 => Ok(*read),
                _ => Err(TreeError::Selection),
            }
        }
        _ => Err(TreeError::Selection),
    }
}

/// The caller must first validate the parent identity, execution and shape.
/// This helper is shared resolution logic, not a replacement for tree validation.
pub(crate) fn child(
    selected: &NodeSelection,
    field: usize,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
) -> Result<ResolvedRead, TreeError> {
    budget.charge(Resource::Work, 1)?;
    let package = profile.language(&selected.entry.alias, budget)?;
    let (declared, tail, actual) = match &selected.shape {
        ShapeSelection::Form { index } => {
            let form = package
                .forms
                .get(usize::try_from(*index).map_err(|_| TreeError::Selection)?)
                .ok_or(TreeError::Selection)?;
            (
                form.fields.get(field).ok_or(TreeError::Selection)?.read,
                false,
                None,
            )
        }
        ShapeSelection::Dynamic {
            shape,
            child_contexts,
            ..
        } => {
            if shape.fields.len() != child_contexts.len() {
                return Err(TreeError::Selection);
            }
            (
                shape.fields.get(field).ok_or(TreeError::Selection)?.read,
                false,
                Some(child_contexts.get(field).ok_or(TreeError::Selection)?),
            )
        }
        ShapeSelection::List { read, cons: true } => {
            let ReadSpec::ListOf { element, .. } = package.read(*read)? else {
                return Err(TreeError::Selection);
            };
            match field {
                0 => (*element, false, None),
                1 => (*read, true, None),
                _ => return Err(TreeError::Selection),
            }
        }
        _ => return Err(TreeError::Selection),
    };
    if let Some(actual) = actual {
        profile.validate_entry(actual, budget)?;
    }
    let mut resolved = if tail {
        // A tail continues the resolved spine; category defaults must not reset its mode.
        ResolvedRead {
            entry: entry_copy(&selected.entry, budget)?,
            read: Some(declared),
            foreign: false,
        }
    } else {
        profile.read_entry(&selected.entry, declared, budget)?
    };
    if let Some(actual) = actual {
        budget.charge(
            Resource::Work,
            (actual.alias.len()
                + actual.category.len()
                + actual.mode.len()
                + actual.package.schema.package.len()
                + selected.entry.alias.len()) as u64
                + 80,
        )?;
        if resolved.read.is_some() {
            if actual != &resolved.entry {
                return Err(TreeError::Selection);
            }
        } else if !resolved.foreign && actual.alias != selected.entry.alias {
            return Err(TreeError::Selection);
        }
        resolved.entry = entry_copy(actual, budget)?;
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::PackageIdentity;
    use nepl3_core::{
        budget::{Limits, StopReason},
        source::Digest,
        value::SchemaRef,
    };

    #[test]
    fn context_copy_charges_utf8_bytes_before_allocation_and_keeps_stops() -> Result<(), TreeError>
    {
        let entry = EntryContext {
            package: PackageIdentity {
                schema: SchemaRef {
                    package: "fixture".into(),
                    revision: 1,
                    digest: Digest::of(b"schema"),
                },
                semantic_digest: Digest::of(b"package"),
            },
            alias: "名".repeat(1000),
            category: "Expr".into(),
            mode: "Alt".into(),
        };
        let bytes = (entry.alias.len()
            + entry.category.len()
            + entry.mode.len()
            + entry.package.schema.package.len()) as u64;
        let limits = Limits {
            work: bytes + 80,
            allocation_units: bytes,
            ..Limits::default()
        };
        let mut exact = Budget::new(limits);
        assert_eq!(entry_copy(&entry, &mut exact)?, entry);
        assert_eq!(exact.usage().work, bytes + 80);
        assert_eq!(exact.usage().allocation_units, bytes);
        let mut no_work = Budget::new(Limits {
            work: bytes + 79,
            ..limits
        });
        assert_eq!(
            entry_copy(&entry, &mut no_work),
            Err(TreeError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(no_work.usage().allocation_units, 0);
        assert_eq!(no_work.poll(), Err(StopReason::WorkLimit));
        let mut no_space = Budget::new(Limits {
            allocation_units: bytes - 1,
            ..limits
        });
        assert_eq!(
            entry_copy(&entry, &mut no_space),
            Err(TreeError::Stopped(StopReason::AllocationLimit))
        );
        assert_eq!(no_space.poll(), Err(StopReason::AllocationLimit));
        let mut cancelled = Budget::new(limits);
        cancelled.cancel();
        assert_eq!(
            entry_copy(&entry, &mut cancelled),
            Err(TreeError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(cancelled.usage().work, 0);
        assert_eq!(cancelled.usage().allocation_units, 0);
        Ok(())
    }
}
