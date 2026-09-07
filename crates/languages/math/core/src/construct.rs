//! Explicit constructors of fresh notation. Lower never invokes this helper
//! to fold or rewrite the author's existing Frac expressions.
use crate::{
    model::*,
    number::{self, ArithmeticError},
};
use alloc::vec;
use nepl3_core::{
    budget::{Budget, Resource},
    value::Rational,
};
pub fn expression_from_rational(
    value: &Rational,
    b: &mut Budget,
) -> Result<MathValue, ArithmeticError> {
    let finite = number::finite_decimal(value, b)?;
    // The arena is flat in memory, but the constructed notation still has a
    // logical root-to-leaf depth relative to the caller's current operation.
    b.observe_depth(if finite { 1 } else { 2 })?;
    let count = if finite { 1 } else { 3 };
    b.charge(Resource::Work, count)?;
    b.charge(Resource::Nodes, count)?;
    b.charge(
        Resource::AllocationUnits,
        count * core::mem::size_of::<MathNode>() as u64,
    )?;
    let node = |kind| MathNode {
        kind,
        origin: None,
        span: None,
        locations: vec![],
    };
    let (root, nodes) = if finite {
        (
            0,
            vec![node(MathKind::Number {
                value: number::clone_with_budget(value, b)?,
                spelling: None,
            })],
        )
    } else {
        let (numerator, denominator) = number::integer_components(value, b)?;
        (
            2,
            vec![
                node(MathKind::Number {
                    value: numerator,
                    spelling: None,
                }),
                node(MathKind::Number {
                    value: denominator,
                    spelling: None,
                }),
                node(MathKind::Frac {
                    left: ExprRef(0),
                    right: ExprRef(1),
                }),
            ],
        )
    };
    Ok(MathValue {
        root: MathRoot::Expr(ExprRef(root)),
        nodes,
        embeds: vec![],
    })
}
