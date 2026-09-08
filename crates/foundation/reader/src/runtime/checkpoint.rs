//! Private rollback marks over append-only collectors. Portable continuations
//! still materialize the complete owned checkpoint at the suspension boundary.
use super::*;
use nepl3_core::diagnostic::TraceOverflow;

#[derive(Debug)]
pub(super) enum Saved {
    Owned(ReaderCheckpoint),
    Mark {
        cursor: u64,
        state: NdfValue,
        lengths: [usize; 7],
        trace_overflow: Option<TraceOverflow>,
    },
}

impl Saved {
    pub fn capture(current: &ReaderCheckpoint, budget: &mut Budget) -> Result<Self, ReaderError> {
        slot::<Self>(budget)?;
        Ok(Self::Mark {
            cursor: current.cursor,
            state: copy(&current.state, budget)?,
            lengths: lengths(current),
            trace_overflow: current.trace_overflow.clone(),
        })
    }
    fn sizes(&self) -> [usize; 7] {
        match self {
            Self::Owned(c) => lengths(c),
            Self::Mark { lengths, .. } => *lengths,
        }
    }
    pub fn elements(&self) -> usize {
        self.sizes()[0]
    }
    pub fn roots(&self) -> usize {
        self.sizes()[1]
    }

    pub fn restore(self, current: &mut ReaderCheckpoint) -> Result<(), ReaderError> {
        match self {
            Self::Owned(value) => *current = value,
            Self::Mark {
                cursor,
                state,
                lengths: saved,
                trace_overflow,
            } => {
                if saved
                    .iter()
                    .zip(lengths(current))
                    .any(|(saved, actual)| *saved > actual)
                {
                    return Err(ReaderError::Continuation);
                }
                current.cursor = cursor;
                current.state = state;
                current.trace_overflow = trace_overflow;
                current.view.elements.truncate(saved[0]);
                current.view.roots.truncate(saved[1]);
                current.facts.truncate(saved[2]);
                current.diagnostics.truncate(saved[3]);
                current.events.truncate(saved[4]);
                current.sources.truncate(saved[5]);
                current.source_maps.truncate(saved[6]);
            }
        }
        Ok(())
    }
    pub fn restore_again(
        &self,
        current: &mut ReaderCheckpoint,
        budget: &mut Budget,
    ) -> Result<(), ReaderError> {
        let saved = match self {
            Self::Owned(value) => Self::Owned(copy(value, budget)?),
            Self::Mark {
                cursor,
                state,
                lengths,
                trace_overflow,
            } => Self::Mark {
                cursor: *cursor,
                state: copy(state, budget)?,
                lengths: *lengths,
                trace_overflow: trace_overflow.clone(),
            },
        };
        saved.restore(current)
    }
    pub fn materialize(
        &self,
        current: &ReaderCheckpoint,
        budget: &mut Budget,
    ) -> Result<ReaderCheckpoint, ReaderError> {
        match self {
            Self::Owned(value) => Ok(copy(value, budget)?),
            Self::Mark {
                cursor,
                state,
                lengths: saved,
                trace_overflow,
            } => {
                if saved
                    .iter()
                    .zip(lengths(current))
                    .any(|(saved, actual)| *saved > actual)
                {
                    return Err(ReaderError::Continuation);
                }
                slot::<ReaderCheckpoint>(budget)?;
                Ok(ReaderCheckpoint {
                    cursor: *cursor,
                    state: copy(state, budget)?,
                    view: ViewBundle {
                        elements: prefix(&current.view.elements, saved[0], budget)?,
                        roots: prefix(&current.view.roots, saved[1], budget)?,
                    },
                    facts: prefix(&current.facts, saved[2], budget)?,
                    diagnostics: prefix(&current.diagnostics, saved[3], budget)?,
                    events: prefix(&current.events, saved[4], budget)?,
                    sources: prefix(&current.sources, saved[5], budget)?,
                    source_maps: prefix(&current.source_maps, saved[6], budget)?,
                    trace_overflow: trace_overflow.clone(),
                })
            }
        }
    }
}
fn lengths(c: &ReaderCheckpoint) -> [usize; 7] {
    [
        c.view.elements.len(),
        c.view.roots.len(),
        c.facts.len(),
        c.diagnostics.len(),
        c.events.len(),
        c.sources.len(),
        c.source_maps.len(),
    ]
}
fn prefix<T: CopyCost + Clone>(
    values: &[T],
    n: usize,
    budget: &mut Budget,
) -> Result<Vec<T>, ReaderError> {
    let values = values.get(..n).ok_or(ReaderError::Continuation)?;
    storage::<T>(n, budget)?;
    let mut result = Vec::with_capacity(n);
    for value in values {
        result.push(copy(value, budget)?);
    }
    Ok(result)
}
pub(super) fn storage<T>(n: usize, budget: &mut Budget) -> Result<(), ReaderError> {
    let bytes = n
        .checked_mul(core::mem::size_of::<T>())
        .filter(|size| *size <= isize::MAX as usize)
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::AllocationUnits, bytes as u64)?;
    Ok(())
}

#[derive(Debug)]
pub(super) struct Frame {
    pub expression: ReaderId,
    pub start: u64,
    pub checkpoint: Saved,
    pub phase: FramePhase,
}
impl Frame {
    pub fn owned(value: ReaderFrame) -> Self {
        Self {
            expression: value.expression,
            start: value.start,
            checkpoint: Saved::Owned(value.checkpoint),
            phase: value.phase,
        }
    }
    pub fn materialize(
        &self,
        current: &ReaderCheckpoint,
        budget: &mut Budget,
    ) -> Result<ReaderFrame, ReaderError> {
        slot::<ReaderFrame>(budget)?;
        Ok(ReaderFrame {
            expression: self.expression,
            start: self.start,
            checkpoint: self.checkpoint.materialize(current, budget)?,
            phase: copy(&self.phase, budget)?,
        })
    }
}
