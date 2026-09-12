//! Structural proof only. Binding, free-symbol requirements and evaluation are
//! separate operations and cannot be inferred from this proof.
mod document;
pub(crate) mod edges;
mod graph;
use crate::model::{MathBindings, MathRoot, MathValue};
use alloc::vec::Vec;
pub use document::{StructureError, ValidatedMathSyntax};
use nepl3_core::budget::{Budget, StopReason};

/// A checked Math expression, borrowing the exact immutable input it proves.
/// Free symbols are valid. This is not evaluation, source-bundle admission, or
/// preparation of foreign annotations; those remain separate operations.
pub struct CheckedExpression<'a> {
    shape: ValidatedMathShape<'a>,
    bindings: MathBindings,
}

impl<'a> CheckedExpression<'a> {
    pub fn value(&self) -> &'a MathValue {
        self.shape.value()
    }

    pub fn shape(&self) -> &ValidatedMathShape<'a> {
        &self.shape
    }

    pub fn bindings(&self) -> &MathBindings {
        &self.bindings
    }
}

/// Validate structure/local constraints and resolve lexical binding using one
/// caller-owned budget. A serialized report never bypasses either validation.
pub fn expression<'a>(
    value: &'a MathValue,
    budget: &mut Budget,
) -> Result<CheckedExpression<'a>, ShapeError> {
    budget.poll()?;
    let MathRoot::Expr(_) = value.root else {
        return Err(ShapeError::Category {
            node: edges::root(value.root).0,
            expected: Category::Expr,
        });
    };
    let shape = value.validate_shape(budget)?;
    let bindings = crate::binding::analyze(&shape, budget)?;
    Ok(CheckedExpression { shape, bindings })
}

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
