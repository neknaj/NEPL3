//! Budgeted comparison of persistent choices, excluding local node numbering.
use super::*;
use nepl3_core::budget::{Budget, Resource, StopReason};

pub(crate) fn entry_equal(
    a: &EntryContext,
    c: &EntryContext,
    b: &mut Budget,
) -> Result<bool, StopReason> {
    let bytes = [
        a.alias.len(),
        c.alias.len(),
        a.category.len(),
        c.category.len(),
        a.mode.len(),
        c.mode.len(),
        a.package.schema.package.len(),
        c.package.schema.package.len(),
    ]
    .iter()
    .fold(84u64, |sum, len| sum.saturating_add(*len as u64));
    b.charge(Resource::Work, bytes)?;
    Ok(a == c)
}
fn selector_bytes(v: &crate::package::StyleSelector) -> u64 {
    match v {
        crate::package::StyleSelector::Field(v) | crate::package::StyleSelector::Capture(v) => {
            v.len() as u64
        }
        _ => 0,
    }
}
impl ShapeSelection {
    /// Recovery is not a resolved choice and deliberately never compares equal.
    pub(crate) fn same_resolved_with_budget(
        &self,
        other: &Self,
        b: &mut Budget,
    ) -> Result<bool, StopReason> {
        b.charge(Resource::Work, 1)?;
        Ok(match (self, other) {
            (Self::Form { index: a }, Self::Form { index: c })
            | (Self::Leaf { index: a }, Self::Leaf { index: c }) => a == c,
            (Self::Builtin { read: a }, Self::Builtin { read: c }) => a == c,
            (Self::List { read: a, cons: x }, Self::List { read: c, cons: y }) => a == c && x == y,
            (
                Self::Dynamic {
                    provider: a,
                    shape: x,
                    child_contexts: u,
                },
                Self::Dynamic {
                    provider: c,
                    shape: y,
                    child_contexts: v,
                },
            ) => {
                for operation in [&a.shape, &a.child_context, &c.shape, &c.child_context] {
                    b.charge(
                        Resource::Work,
                        (operation.schema.package.len() as u64)
                            .saturating_add(operation.name.len() as u64)
                            .saturating_add(42),
                    )?;
                }
                if a != c || u.len() != v.len() {
                    return Ok(false);
                }
                for shape in [x, y] {
                    b.charge(
                        Resource::Work,
                        (shape.kind.schema.package.len() as u64).saturating_add(42),
                    )?;
                    for field in &shape.fields {
                        b.charge(Resource::Work, (field.name.len() as u64).saturating_add(9))?;
                    }
                    for rule in &shape.selection_rules {
                        b.charge(
                            Resource::Work,
                            selector_bytes(&rule.selector).saturating_add(9),
                        )?;
                    }
                    for style in &shape.styles {
                        b.charge(
                            Resource::Work,
                            selector_bytes(&style.selector)
                                .saturating_add(style.class.schema.package.len() as u64)
                                .saturating_add(style.class.name.len() as u64)
                                .saturating_add(43),
                        )?;
                    }
                }
                if x != y {
                    return Ok(false);
                }
                for (a, c) in u.iter().zip(v) {
                    if !entry_equal(a, c, b)? {
                        return Ok(false);
                    }
                }
                true
            }
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests;
