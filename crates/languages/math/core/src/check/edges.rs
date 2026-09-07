use super::Category;
use crate::model::{MathKind, MathRoot};
pub(crate) fn root(root: MathRoot) -> (u64, Category) {
    match root {
        MathRoot::Expr(v) => (v.0, Category::Expr),
        MathRoot::Row(v) => (v.0, Category::Row),
        MathRoot::DocGuest(v) => (v.0, Category::DocGuest),
    }
}
pub(crate) fn accepts(kind: &MathKind, category: Category) -> bool {
    match category {
        Category::Row => matches!(kind, MathKind::Row { .. }),
        Category::DocGuest => matches!(kind, MathKind::DocGuest { .. }),
        Category::Expr => !matches!(kind, MathKind::Row { .. } | MathKind::DocGuest { .. }),
    }
}
pub(crate) fn edge(kind: &MathKind, index: usize) -> Option<(u64, Category)> {
    use MathKind::*;
    let expr = match kind {
        Number { .. } | Symbol { .. } | Text { .. } | DocGuest { .. } => return None,
        Add { left, right }
        | Sub { left, right }
        | Mul { left, right }
        | Frac { left, right }
        | Pow { left, right }
        | Equal { left, right }
        | Lt { left, right }
        | Le { left, right }
        | Subscript { left, right }
        | Superscript { left, right } => match index {
            0 => Some(left.0),
            1 => Some(right.0),
            _ => None,
        },
        Neg { value }
        | Sqrt { value }
        | Transpose { value }
        | Det { value }
        | Fence { value, .. } => (index == 0).then_some(value.0),
        Root { degree, radicand } => match index {
            0 => Some(degree.0),
            1 => Some(radicand.0),
            _ => None,
        },
        Scripts { base, sub, sup } => match index {
            0 => Some(base.0),
            1 => Some(sub.0),
            2 => Some(sup.0),
            _ => None,
        },
        Sequence { values } | Vector { values } | Row { values } => values.get(index).map(|v| v.0),
        Matrix { rows } => return rows.get(index).map(|v| (v.0, Category::Row)),
        Let { init, body, .. } => match index {
            0 => Some(init.0),
            1 => Some(body.0),
            _ => None,
        },
        Sum {
            lower, upper, body, ..
        }
        | Integral {
            lower, upper, body, ..
        } => match index {
            0 => Some(lower.0),
            1 => Some(upper.0),
            2 => Some(body.0),
            _ => None,
        },
        Call {
            function,
            arguments,
        } => {
            if index == 0 {
                Some(function.0)
            } else {
                arguments.get(index - 1).map(|v| v.0)
            }
        }
        Label { value, annotation } => {
            return match index {
                0 => Some((value.0, Category::Expr)),
                1 => Some((annotation.0, Category::DocGuest)),
                _ => None,
            };
        }
    };
    expr.map(|v| (v, Category::Expr))
}
