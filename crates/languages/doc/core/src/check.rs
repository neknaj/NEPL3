//! Arena shape checking. This proof does not resolve labels or prepare embeds.
mod document;
pub(crate) mod edges;
mod graph;
use crate::model::DocValue;
use alloc::vec::Vec;
pub use document::{StructureError, ValidatedDocumentSyntax};
use nepl3_core::budget::StopReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Article,
    Body,
    Block,
    Flow,
    Sentence,
    Inline,
    Variant,
    Row,
    ListItem,
    Alignment,
    ListStyle,
    Check,
    Target,
    Asset,
    OptionalRow,
    OptionalSentence,
    OptionalText,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeError {
    Stopped(StopReason),
    Reference(u64),
    Category { node: u64, expected: Category },
    Cycle(u64),
    Unreachable(u64),
    Embed(u64),
    UnusedEmbed(u64),
    EmptyAnnotationPart(u64),
    AnnotationNotes(u64),
    ParallelArity(u64),
    DuplicateLanguage { node: u64, first: u64, second: u64 },
    TableWidth(u64),
    FieldLocation(u64),
}
impl From<StopReason> for ShapeError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
/// Only category, graph and local Doc constraints have been checked. Source
/// closure, labels and foreign requirements remain separate operations.
pub struct ValidatedDocShape<'a> {
    value: &'a DocValue,
    order: Vec<usize>,
}
impl<'a> ValidatedDocShape<'a> {
    pub fn value(&self) -> &'a DocValue {
        self.value
    }
    /// Children precede their owner; each node occurs exactly once.
    pub fn postorder(&self) -> &[usize] {
        &self.order
    }
    pub(crate) fn into_order(self) -> Vec<usize> {
        self.order
    }
}
