//! Canonical free-symbol assignments; lexical binders remain expression-local.
use crate::{
    exact,
    model::{BindingEnvironment, MathExactValue},
};
use core::cmp::Ordering;
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentError {
    DuplicateName {
        index: u64,
    },
    UnsortedName {
        index: u64,
    },
    Value {
        index: u64,
        error: exact::ExactValueError,
    },
    Stopped(StopReason),
}
impl From<StopReason> for EnvironmentError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Proof of name order, uniqueness and all value shapes, borrowing exact input.
/// Does not prove that an expression has all its requirements or can evaluate.
pub struct CheckedEnvironment<'a>(&'a BindingEnvironment);
impl<'a> CheckedEnvironment<'a> {
    pub fn environment(&self) -> &'a BindingEnvironment {
        self.0
    }

    /// O(L log(N+1)) work, O(1) space, no allocation. Missing names return None;
    /// resource stops remain failures even when the environment is empty.
    pub fn get(
        &self,
        name: &str,
        budget: &mut Budget,
    ) -> Result<Option<&'a MathExactValue>, StopReason> {
        budget.poll()?;
        let mut lo = 0;
        let mut hi = self.0.assignments.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let item = &self.0.assignments[mid];
            match compare(&item.name, name, budget)? {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Ok(Some(&item.value)),
            }
        }
        Ok(None)
    }
}

fn compare(a: &str, b: &str, budget: &mut Budget) -> Result<Ordering, StopReason> {
    budget.charge(
        Resource::Work,
        (a.len().min(b.len()) as u64).saturating_add(1),
    )?;
    Ok(a.cmp(b))
}

/// O(N*L) name comparison work and O(1) additional space; values use exact::check.
/// No normalization, sorting, last-wins duplicate handling or input mutation.
pub fn check<'a>(
    input: &'a BindingEnvironment,
    budget: &mut Budget,
) -> Result<CheckedEnvironment<'a>, EnvironmentError> {
    budget.poll()?;
    for (index, assignment) in input.assignments.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        budget.charge(Resource::Nodes, 1)?;
        if index > 0 {
            match compare(&input.assignments[index - 1].name, &assignment.name, budget)? {
                Ordering::Equal => {
                    return Err(EnvironmentError::DuplicateName {
                        index: index as u64,
                    });
                }
                Ordering::Greater => {
                    return Err(EnvironmentError::UnsortedName {
                        index: index as u64,
                    });
                }
                Ordering::Less => {}
            }
        }
        exact::check(&assignment.value, budget).map_err(|error| match error {
            exact::ExactValueError::Stopped(reason) => EnvironmentError::Stopped(reason),
            error => EnvironmentError::Value {
                index: index as u64,
                error,
            },
        })?;
    }
    Ok(CheckedEnvironment(input))
}
