//! Structural proof only. Binding, free-symbol requirements and evaluation are
//! separate operations and cannot be inferred from this proof.
mod document;
pub(crate) mod edges;
mod graph;
use crate::model::MathValue;
use alloc::vec::Vec;
pub use document::{StructureError, ValidatedMathSyntax};
use nepl3_core::budget::StopReason;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Expr,
    Row,
    DocGuest,
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
    GuestCategory(u64),
    FieldLocation(u64),
    NonFiniteDecimalNumber(u64),
    EmptyVector(u64),
    EmptyMatrix(u64),
    MatrixWidth { node: u64, row: u64 },
    FenceWidth(u64),
    InvalidRootDegree(u64),
}
impl From<StopReason> for ShapeError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
pub struct ValidatedMathShape<'a> {
    value: &'a MathValue,
    order: Vec<usize>,
}
impl<'a> ValidatedMathShape<'a> {
    pub fn value(&self) -> &'a MathValue {
        self.value
    }
    pub fn postorder(&self) -> &[usize] {
        &self.order
    }
}
