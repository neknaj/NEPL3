//! Strict whole-snapshot consumption from a retained native execution.
//! No whitespace is skipped here; reader plans own every consumed byte.
use super::{ExecutionKind, RetainedParse};
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WholeInputError {
    Stopped(StopReason),
    Recovered,
    OpenInput,
    PartialRange,
    Unconsumed { cursor: u64, limit: u64 },
}

/// Borrows the immutable execution and its exact input provenance. This proves
/// complete parsing and byte consumption, not Binding or editing authority.
pub struct WholeInput<'a, 'p> {
    parsed: &'a RetainedParse<'p>,
}
impl<'a, 'p> WholeInput<'a, 'p> {
    pub fn parsed(&self) -> &'a RetainedParse<'p> {
        self.parsed
    }
}

pub fn check<'a, 'p>(
    parsed: &'a RetainedParse<'p>,
    budget: &mut Budget,
) -> Result<WholeInput<'a, 'p>, WholeInputError> {
    budget
        .charge(Resource::Work, 1)
        .map_err(WholeInputError::Stopped)?;
    if parsed.execution().kind() != ExecutionKind::Complete {
        return Err(WholeInputError::Recovered);
    }
    let input = parsed.seed().request();
    if !input.final_input {
        return Err(WholeInputError::OpenInput);
    }
    if input.start != 0 || input.limit != input.snapshot.text().len() as u64 {
        return Err(WholeInputError::PartialRange);
    }
    let cursor = parsed.execution().cursor();
    if cursor != input.limit {
        return Err(WholeInputError::Unconsumed {
            cursor,
            limit: input.limit,
        });
    }
    Ok(WholeInput { parsed })
}
