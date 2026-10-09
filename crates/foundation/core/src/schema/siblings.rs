//! Borrowed sibling groups keep validation storage proportional to nesting.
use super::frontier::Frontier;
use super::{FieldDescriptor, SchemaError, TypeDescriptor};
use crate::budget::{Budget, Resource};
use crate::value::NdfValue;

#[derive(Clone, Copy)]
pub(super) enum Siblings<'d, 'v> {
    Uniform(&'d TypeDescriptor, &'v [NdfValue], u64),
    Fields(&'d [FieldDescriptor], &'v [NdfValue], u64),
}

type Visit<'d, 'v> = (
    &'d TypeDescriptor,
    &'v NdfValue,
    u64,
    Option<Siblings<'d, 'v>>,
);

pub(super) fn enqueue<'d, 'v>(
    pending: &mut Frontier<Siblings<'d, 'v>>,
    group: Siblings<'d, 'v>,
    budget: &mut Budget,
) -> Result<(), SchemaError> {
    // Individual charges preserve the successful Work prefix on failure.
    // All siblings are admitted before visiting any child node or depth.
    for _ in 0..group.len() {
        budget.charge(Resource::Work, 1)?;
    }
    if group.len() != 0 {
        pending.push(group, budget)?;
    }
    Ok(())
}

impl<'d, 'v> Siblings<'d, 'v> {
    pub(super) fn len(self) -> usize {
        match self {
            Self::Uniform(_, values, _) | Self::Fields(_, values, _) => values.len(),
        }
    }

    pub(super) fn split_first(self) -> Result<Option<Visit<'d, 'v>>, SchemaError> {
        match self {
            Self::Uniform(ty, values, depth) => Ok(values.split_first().map(|(value, rest)| {
                (
                    ty,
                    value,
                    depth,
                    (!rest.is_empty()).then_some(Self::Uniform(ty, rest, depth)),
                )
            })),
            Self::Fields(fields, values, depth) => {
                // Every group is admitted only after the public field-count check.
                // Keep this internal slicing boundary checked as well.
                if fields.len() != values.len() {
                    return Err(SchemaError::FieldCount);
                }
                Ok(fields.split_first().zip(values.split_first()).map(
                    |((field, fields), (value, values))| {
                        (
                            &field.ty,
                            value,
                            depth,
                            (!values.is_empty()).then_some(Self::Fields(fields, values, depth)),
                        )
                    },
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{Limits, StopReason};
    use crate::schema::SchemaRegistry;
    use alloc::vec;

    #[test]
    fn deep_branching_groups_charge_exact_spill_and_keep_sticky_stops() -> Result<(), SchemaError> {
        let limits = Limits {
            work: 100_000,
            nodes: 100_000,
            depth: 100,
            allocation_units: 100_000,
            ..Limits::default()
        };
        let mut registry = SchemaRegistry::default();
        registry.finalize(&mut Budget::new(limits))?;
        let slot = core::mem::size_of::<Siblings<'_, '_>>() as u64;
        for (levels, slots) in [(1, 0), (8, 0), (9, 8), (16, 8), (17, 16)] {
            let mut value = NdfValue::Unit;
            for _ in 0..levels {
                value = NdfValue::List(vec![value, NdfValue::Unit]);
            }
            let exact = Limits {
                allocation_units: slots * slot,
                ..limits
            };
            let mut b = Budget::new(exact);
            registry.validate(&TypeDescriptor::NdfValue, &value, &mut b)?;
            assert_eq!(b.usage().allocation_units, slots * slot);
            assert_eq!(b.usage().work, 2 * levels + 1);
            assert_eq!(b.usage().nodes, 2 * levels + 1);
            assert_eq!(b.usage().depth, levels + 1);
            if slots != 0 {
                let mut b = Budget::new(Limits {
                    allocation_units: slots * slot - 1,
                    ..exact
                });
                assert_eq!(
                    registry
                        .validate(&TypeDescriptor::NdfValue, &value, &mut b)
                        .err(),
                    Some(SchemaError::Stopped(StopReason::AllocationLimit))
                );
                let used = b.usage();
                assert_eq!(used.allocation_units, if slots == 8 { 0 } else { 8 * slot });
                assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
                assert_eq!(
                    registry
                        .validate(&TypeDescriptor::NdfValue, &NdfValue::Unit, &mut b)
                        .err(),
                    Some(SchemaError::Stopped(StopReason::AllocationLimit))
                );
                assert_eq!(used, b.usage());
            }
        }
        Ok(())
    }
}
