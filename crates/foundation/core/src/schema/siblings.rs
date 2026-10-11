//! Borrowed sibling groups keep validation storage proportional to nesting.
use super::frontier::Frontier;
use super::{FieldDescriptor, SchemaError, TypeDescriptor};
use crate::budget::{Budget, Resource};
use crate::value::NdfValue;

/// Admitted sibling sequence. Its representation cannot be constructed by
/// callers; heterogeneous field/value lengths are checked by [`Self::fields`].
#[derive(Clone, Copy)]
pub(super) struct Siblings<'d, 'v>(Kind<'d, 'v>);

#[derive(Clone, Copy)]
enum Kind<'d, 'v> {
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
    /// Borrow a homogeneous sequence at one traversal depth without allocation.
    pub(super) fn uniform(ty: &'d TypeDescriptor, values: &'v [NdfValue], depth: u64) -> Self {
        Self(Kind::Uniform(ty, values, depth))
    }

    /// Admit a field/value pairing before any child Work or storage is charged.
    /// Equal lengths establish the representation invariant; mismatch returns
    /// FieldCount without producing a group. Empty equal slices are valid.
    pub(super) fn fields(
        fields: &'d [FieldDescriptor],
        values: &'v [NdfValue],
        depth: u64,
    ) -> Result<Self, SchemaError> {
        if fields.len() != values.len() {
            return Err(SchemaError::FieldCount);
        }
        Ok(Self(Kind::Fields(fields, values, depth)))
    }

    /// Number of unvisited siblings, in constant time and without allocation.
    pub(super) fn len(self) -> usize {
        match self.0 {
            Kind::Uniform(_, values, _) | Kind::Fields(_, values, _) => values.len(),
        }
    }

    /// Return the first visit and nonempty remainder, preserving alignment,
    /// ordering and depth. This pure constant-time step allocates nothing.
    /// The defensive alignment check also detects an internal invariant bug;
    /// callers cannot construct a misaligned representation directly.
    pub(super) fn split_first(self) -> Result<Option<Visit<'d, 'v>>, SchemaError> {
        match self.0 {
            Kind::Uniform(ty, values, depth) => Ok(values.split_first().map(|(value, rest)| {
                (
                    ty,
                    value,
                    depth,
                    (!rest.is_empty()).then_some(Self(Kind::Uniform(ty, rest, depth))),
                )
            })),
            Kind::Fields(fields, values, depth) => {
                // Construction is sealed behind fields(); slicing both tails
                // preserves its invariant. Keep a defensive boundary check.
                if fields.len() != values.len() {
                    return Err(SchemaError::FieldCount);
                }
                Ok(fields.split_first().zip(values.split_first()).map(
                    |((field, fields), (value, values))| {
                        (
                            &field.ty,
                            value,
                            depth,
                            (!values.is_empty())
                                .then_some(Self(Kind::Fields(fields, values, depth))),
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

    /// Both mismatch directions fail before a group exists. Successful tails
    /// preserve each descriptor/value pairing and never retain an empty frame.
    #[test]
    fn field_admission_and_remainders_preserve_alignment() -> Result<(), SchemaError> {
        let fields = [
            FieldDescriptor {
                name: "first".into(),
                ty: TypeDescriptor::U64,
            },
            FieldDescriptor {
                name: "second".into(),
                ty: TypeDescriptor::Bool,
            },
        ];
        let values = [NdfValue::U64(7), NdfValue::Bool(true)];
        for (field_count, value_count) in [(0, 1), (1, 0), (1, 2), (2, 1)] {
            assert!(matches!(
                Siblings::fields(&fields[..field_count], &values[..value_count], 4),
                Err(SchemaError::FieldCount)
            ));
        }
        assert!(Siblings::fields(&[], &[], 4)?.split_first()?.is_none());
        let group = Siblings::fields(&fields, &values, 4)?;
        assert_eq!(group.len(), 2);
        let Some((ty, value, depth, Some(rest))) = group.split_first()? else {
            return Err(SchemaError::FieldCount);
        };
        assert_eq!(ty, &TypeDescriptor::U64);
        assert_eq!(value, &values[0]);
        assert_eq!(depth, 4);
        assert_eq!(rest.len(), 1);
        let Some((ty, value, depth, None)) = rest.split_first()? else {
            return Err(SchemaError::FieldCount);
        };
        assert_eq!(ty, &TypeDescriptor::Bool);
        assert_eq!(value, &values[1]);
        assert_eq!(depth, 4);
        assert_eq!(
            core::mem::size_of::<Siblings<'_, '_>>(),
            core::mem::size_of::<Kind<'_, '_>>()
        );
        Ok(())
    }

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
