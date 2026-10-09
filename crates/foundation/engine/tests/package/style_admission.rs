use super::*;
use nepl3_core::{
    budget::{StopReason, Usage},
    view::{FallbackRole, PresentationClass},
};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn presentation_owner_schema_scan_obeys_budget_and_preserves_failure_binding() -> Result<(), String>
{
    for suffix in [false, true] {
        let (mut package, _) = fixture()?;
        package.bindings = vec![Binding::None];
        package.leaves[0].binding = BindingId(0);
        let target = SchemaDescriptor {
            package: "test.style".into(),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        let schema = target.reference(&mut budget()).map_err(err)?;
        let padding = SchemaDescriptor {
            package: "padding".repeat(128),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        let descriptors = if suffix {
            vec![target, padding]
        } else {
            vec![padding, target]
        };
        let mut registry = SchemaRegistry::default();
        let mut fee = 1;
        let mut found = false;
        for d in descriptors {
            if !found {
                fee += (d.package.len() + schema.package.len() + 9) as u64;
                found = d.package == schema.package;
            }
            registry
                .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        fee += schema.package.len() as u64 + 41;
        registry.finalize(&mut budget()).map_err(err)?;
        package.leaves[0].styles = vec![StyleRule {
            selector: StyleSelector::Head,
            class: PresentationClass {
                schema: schema.clone(),
                name: "head".into(),
                fallback: FallbackRole::Content,
            },
        }];
        // One Binding::None, one style entry, one selector; the only allocation
        // is the pending BindingId. Schema lookup borrows registry storage.
        let allocation = core::mem::size_of::<BindingId>() as u64;
        let mut limits = budget().limits();
        limits.work = 3 + fee;
        let mut b = Budget::new(limits);
        package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .map_err(err)?;
        assert_eq!(
            b.usage(),
            Usage {
                work: 3 + fee,
                allocation_units: allocation,
                ..Usage::default()
            }
        );
        for cap in [2, 3, 2 + fee - 1] {
            let mut limits = budget().limits();
            limits.work = cap;
            let mut b = Budget::new(limits);
            let failure = package
                .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
                .err()
                .ok_or("expected lookup stop")?;
            assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
            assert_eq!(failure.binding, None);
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
            let accepted = if cap == 2 {
                2
            } else if cap == 3 {
                3
            } else {
                2 + fee - schema.package.len() as u64 - 41
            };
            assert_eq!(
                b.usage(),
                Usage {
                    work: accepted,
                    allocation_units: allocation,
                    ..Usage::default()
                }
            );
        }
        package.leaves[0].styles[0].class.schema.digest.0[0] ^= 1;
        let mut b = budget();
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("expected invalid selector")?;
        assert_eq!(failure.error, PackageError::InvalidSelector);
        assert_eq!(failure.binding, None);
        assert_eq!(
            b.usage(),
            Usage {
                work: 2 + fee,
                allocation_units: allocation,
                ..Usage::default()
            }
        );
        package.leaves[0].styles[0].class.schema.package = "absent".into();
        let mut b = budget();
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("expected missing class owner")?;
        assert_eq!(failure.error, PackageError::InvalidSelector);
        assert_eq!(failure.binding, None);
        let missing = 1
            + ("padding".len() * 128 + "absent".len() + 9) as u64
            + (schema.package.len() + "absent".len() + 9) as u64;
        assert_eq!(
            b.usage(),
            Usage {
                work: 2 + missing,
                allocation_units: allocation,
                ..Usage::default()
            }
        );
        let mut b = budget();
        b.cancel();
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("expected cancellation")?;
        assert_eq!(failure.error, PackageError::Stopped(StopReason::Cancelled));
        assert_eq!(b.usage(), Usage::default());
        assert_eq!(b.poll(), Err(StopReason::Cancelled));
        package.leaves[0].styles[0].class.name.clear();
        let mut limits = budget().limits();
        limits.work = 2;
        let mut b = Budget::new(limits);
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("expected empty name rejection")?;
        assert_eq!(failure.error, PackageError::InvalidSelector);
        assert_eq!(failure.binding, None);
        assert_eq!(b.usage().work, 2);
        assert_eq!(b.poll(), Ok(()));
    }
    Ok(())
}
