use super::*;

#[derive(Clone, Copy)]
pub(super) enum Shape {
    Scalar,
    Truth,
    Vector(u64),
    Matrix { rows: u64, cols: u64 },
}

fn known(value: &PartialOutcome) -> Option<Shape> {
    match value {
        PartialOutcome::Symbolic(_, shape) => *shape,
        PartialOutcome::Exact(value) => Some(match value {
            MathExactValue::Scalar { .. } => Shape::Scalar,
            MathExactValue::Truth { .. } => Shape::Truth,
            MathExactValue::Vector { values } => Shape::Vector(values.len() as u64),
            MathExactValue::Matrix { rows, cols, .. } => Shape::Matrix {
                rows: *rows,
                cols: *cols,
            },
        }),
    }
}

// Conservative, evaluation-local evidence. Missing scalar contents do not erase
// a constructor's dimensions. Unknown expressions are never assumed scalar.
pub(super) fn result(
    node: ExprRef,
    kind: &MathKind,
    operands: &[PartialOutcome],
    b: &mut Budget,
) -> Result<Option<Shape>, Error> {
    b.charge(Resource::Work, 1)?;
    let first = operands.first().and_then(known);
    Ok(match kind {
        MathKind::Vector { values } | MathKind::Row { values } => {
            Some(Shape::Vector(values.len() as u64))
        }
        MathKind::Matrix { rows } => match first {
            Some(Shape::Vector(cols)) => Some(Shape::Matrix {
                rows: rows.len() as u64,
                cols,
            }),
            _ => None,
        },
        MathKind::Fence { .. } | MathKind::Label { .. } | MathKind::Neg { .. } => first,
        MathKind::Transpose { .. } => match first {
            Some(Shape::Matrix { rows, cols }) => Some(Shape::Matrix {
                rows: cols,
                cols: rows,
            }),
            Some(Shape::Vector(cols)) => Some(Shape::Matrix { rows: 1, cols }),
            _ => None,
        },
        MathKind::Det { .. } => match first {
            Some(Shape::Matrix { rows, cols }) if rows == cols => Some(Shape::Scalar),
            Some(_) => return Err(at(node, arithmetic::Error::NotSquare)),
            None => None,
        },
        _ => None,
    })
}
