//! Structural display inventory, not source admission or namespace adoption.
//! The current Doc HTML variant policy is shared with actual preparation.
use crate::{LocalPreparationError, RenderOptions};
use alloc::vec::Vec;
use core::convert::Infallible;
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_doc_core::{
    check::ValidatedDocShape,
    model::{DocKind, EmbedRef},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceOccurrence {
    /// Index in the complete structural_occurrences sequence for this shape.
    /// Distinct expanded occurrences may have identical node and syntax values.
    pub structural_index: u64,
    pub node: u64,
    pub syntax: EmbedRef,
    /// Constructor depth, not generated HTML insertion depth.
    pub depth: u64,
}
#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Language,
    MissingVariant { node: u64 },
    ListStart { node: u64, start: u64 },
    InternalShape,
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
fn push<T>(v: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    v.try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    v.push(value);
    Ok(())
}
/// Inventory structural Sentence insertions selected by the Doc display policy.
/// Unsupported roots or constructs may still have inventory entries: this is
/// not a promise that preparation or rendering can invoke their callbacks.
/// All variants still undergo ordinary option validation. Image alt subtrees
/// are excluded because they require text projection, not HTML guest insertion.
/// This does not enter foreign closures or authorize rendering from this list.
/// The caller must independently retain/admit owners and decide namespace adoption.
pub fn sentence_occurrences(
    shape: &ValidatedDocShape<'_>,
    options: &RenderOptions,
    b: &mut Budget,
) -> Result<Vec<SentenceOccurrence>, Error> {
    b.poll()?;
    let value = shape.value();
    let selections =
        crate::prepare::selection_table::<Infallible>(value, options, b).map_err(|e| match e {
            LocalPreparationError::Stopped(s) => Error::Stopped(s),
            LocalPreparationError::Language => Error::Language,
            LocalPreparationError::MissingVariant { node } => Error::MissingVariant { node },
            LocalPreparationError::ListStart { node, start } => Error::ListStart { node, start },
            _ => Error::InternalShape,
        })?;
    let structure = shape.structural_occurrences(b)?;
    let mut visible = Vec::new();
    let mut output = Vec::new();
    for (index, occurrence) in structure.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let active = if let Some(parent) = occurrence.parent {
            let parent_index =
                usize::try_from(parent.occurrence).map_err(|_| Error::InternalShape)?;
            let ancestor = structure.get(parent_index).ok_or(Error::InternalShape)?;
            let shown = *visible.get(parent_index).ok_or(Error::InternalShape)?;
            shown
                && match &value.nodes[ancestor.node as usize].kind {
                    DocKind::Image { .. } => parent.child == 1,
                    DocKind::InlineImage { .. } => false,
                    DocKind::Parallel { .. } => selections[ancestor.node as usize]
                        .is_none_or(|selected| selected.0 == occurrence.node),
                    _ => true,
                }
        } else {
            true
        };
        push(&mut visible, active, b)?;
        if active
            && let DocKind::SentenceGuest { syntax } = &value.nodes[occurrence.node as usize].kind
        {
            push(
                &mut output,
                SentenceOccurrence {
                    structural_index: index as u64,
                    node: occurrence.node,
                    syntax: *syntax,
                    depth: occurrence.depth,
                },
                b,
            )?;
        }
    }
    b.poll()?;
    Ok(output)
}
