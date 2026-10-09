use nepl3_core::{
    budget::{Budget, Limits, StopReason, Usage},
    schema::{SchemaDescriptor, SchemaError, SchemaRegistry},
};
use nepl3_engine::{
    binding::BindingError,
    portable::{PortableError, binding::failure_to_value},
};
const OWNERS: [&str; 3] = ["nepl3.engine", "nepl3.foundation", "nepl3.reader"];
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 100_000,
        nodes: 1000,
        depth: 100,
        ..Limits::default()
    })
}
fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn registry(available: usize) -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    for name in &OWNERS[..available] {
        let descriptor = SchemaDescriptor {
            package: (*name).into(),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        let reference = descriptor.reference(&mut budget()).map_err(error)?;
        registry
            .register(reference, descriptor, &mut budget())
            .map_err(error)?;
    }
    registry.finalize(&mut budget()).map_err(error)?;
    Ok(registry)
}
#[test]
fn typed_failure_owner_selection_obeys_budget_before_missing_schema() -> Result<(), String> {
    for available in 0..3 {
        let registry = registry(available)?;
        let mut lookup = budget();
        for name in &OWNERS[..available] {
            assert!(
                registry
                    .selected_descriptor_with_budget(name, 1, &mut lookup)
                    .map_err(error)?
                    .is_some()
            );
        }
        let prefix = lookup.usage().work;
        assert!(
            registry
                .selected_descriptor_with_budget(OWNERS[available], 1, &mut lookup)
                .map_err(error)?
                .is_none()
        );
        let full = lookup.usage().work;
        let cause = BindingError::MissingProvider;
        let mut b = budget();
        assert!(matches!(
            failure_to_value(&cause, &registry, &mut b),
            Err(PortableError::Schema(SchemaError::UnknownSchema))
        ));
        assert_eq!(
            b.usage(),
            Usage {
                work: full,
                ..Usage::default()
            }
        );
        let mut limits = budget().limits();
        limits.work = prefix;
        let mut b = Budget::new(limits);
        assert!(matches!(
            failure_to_value(&cause, &registry, &mut b),
            Err(PortableError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(
            b.usage(),
            Usage {
                work: prefix,
                ..Usage::default()
            }
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        if available > 0 {
            let mut limits = budget().limits();
            limits.work = prefix + 1;
            let mut b = Budget::new(limits);
            assert!(matches!(
                failure_to_value(&cause, &registry, &mut b),
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(b.usage().work, prefix + 1);
        }
        let mut b = budget();
        b.cancel();
        assert!(matches!(
            failure_to_value(&cause, &registry, &mut b),
            Err(PortableError::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(b.usage(), Usage::default());
    }
    Ok(())
}
