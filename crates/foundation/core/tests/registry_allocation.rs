use nepl3_core::{budget::*, schema::*};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 128,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

#[test]
fn registry_slot_reservation_stops_before_publishing_and_allows_retry() -> Result<(), String> {
    let descriptor = SchemaDescriptor {
        package: "test.empty".into(),
        revision: 1,
        types: vec![],
        operations: vec![],
    };
    let mut identity_budget = budget();
    let identity = descriptor
        .reference(&mut identity_budget)
        .map_err(|e| format!("{e:?}"))?;
    // Permit identity calculation exactly; no capacity remains for a registry slot.
    let mut constrained = Budget::new(Limits {
        allocation_units: identity_budget.usage().allocation_units,
        ..budget().limits()
    });
    let mut registry = SchemaRegistry::default();
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        registry.register(identity.clone(), descriptor.clone(), &mut constrained),
        Err(SchemaError::Stopped(StopReason::AllocationLimit))
    ));
    assert!(registry.is_finalized());
    assert!(registry.selected("test.empty", 1).is_none());
    registry
        .register(identity.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(!registry.is_finalized());
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(registry.selected("test.empty", 1), Some(&identity));
    Ok(())
}
