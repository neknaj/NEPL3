use nepl3_core::{budget::*, schema::*};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn registry() -> Result<SchemaRegistry, SchemaError> {
    let mut result = SchemaRegistry::default();
    for package in ["first", "later-longer"] {
        let descriptor = SchemaDescriptor {
            package: package.into(),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        let reference = descriptor.reference(&mut budget())?;
        result.register(reference, descriptor, &mut budget())?;
    }
    Ok(result)
}
#[test]
fn metered_selected_lookup_returns_the_exact_borrowed_pair() -> Result<(), String> {
    let registry = registry().map_err(|e| format!("{e:?}"))?;
    assert!(!registry.is_finalized()); // This is a lookup, not a validation proof.
    let mut costs = Vec::new();
    for name in ["first", "later-longer"] {
        let query = name.to_owned();
        let mut b = budget();
        let (identity, descriptor) = registry
            .selected_descriptor_with_budget(&query, 1, &mut b)
            .map_err(|e| format!("{e:?}"))?
            .ok_or("missing")?;
        drop(query); // Returned references borrow only the registry.
        assert_eq!(Some(identity), registry.selected(name, 1));
        assert!(core::ptr::eq(
            descriptor,
            registry.descriptor(identity).ok_or("descriptor")?
        ));
        let usage = b.usage();
        assert_eq!(
            Usage {
                work: usage.work,
                ..Usage::default()
            },
            usage
        );
        costs.push(usage.work);
    }
    assert!(costs[1] > costs[0]);
    assert!(
        registry
            .selected_descriptor_with_budget("first", 2, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
    assert!(
        registry
            .selected_descriptor_with_budget("absent", 1, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
    Ok(())
}
#[test]
fn metered_lookup_empty_and_stopped_paths_charge_work_only() -> Result<(), String> {
    let empty = SchemaRegistry::default();
    let mut b = budget();
    assert!(
        empty
            .selected_descriptor_with_budget("none", 1, &mut b)
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
    assert_eq!(
        b.usage(),
        Usage {
            work: 1,
            ..Usage::default()
        }
    );
    let registry = registry().map_err(|e| format!("{e:?}"))?;
    let mut full = budget();
    let _ = registry
        .selected_descriptor_with_budget("later-longer", 1, &mut full)
        .map_err(|e| format!("{e:?}"))?;
    for work in [0, full.usage().work - 1] {
        let mut b = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        assert!(matches!(
            registry.selected_descriptor_with_budget("later-longer", 1, &mut b),
            Err(StopReason::WorkLimit)
        ));
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    }
    let mut b = budget();
    b.cancel();
    assert!(matches!(
        empty.selected_descriptor_with_budget("none", 1, &mut b),
        Err(StopReason::Cancelled)
    ));
    Ok(())
}

#[test]
fn exact_descriptor_lookup_is_borrowed_metered_and_not_a_validation_proof() -> Result<(), String> {
    let registry = registry().map_err(|e| format!("{e:?}"))?;
    assert!(!registry.is_finalized());
    for (name, late) in [("first", false), ("later-longer", true)] {
        let reference = registry.selected(name, 1).ok_or("reference")?.clone();
        let work = 1
            + (5 + name.len() as u64 + 9)
            + if late { 12 + name.len() as u64 + 9 } else { 0 }
            + name.len() as u64
            + 41;
        let mut b = budget();
        let descriptor = registry
            .descriptor_with_budget(&reference, &mut b)
            .map_err(|e| format!("{e:?}"))?
            .ok_or("descriptor")?;
        assert!(core::ptr::eq(
            descriptor,
            registry.descriptor(&reference).ok_or("identity")?
        ));
        assert_eq!(
            b.usage(),
            Usage {
                work,
                ..Usage::default()
            }
        );
        let mut wrong = reference.clone();
        wrong.digest.0[0] ^= 1;
        let mut b = budget();
        assert!(
            registry
                .descriptor_with_budget(&wrong, &mut b)
                .map_err(|e| format!("{e:?}"))?
                .is_none()
        );
        assert_eq!(b.usage().work, work);
        for cap in [0, 1, work - (name.len() as u64 + 41)] {
            let mut b = Budget::new(Limits {
                work: cap,
                ..budget().limits()
            });
            assert_eq!(
                registry.descriptor_with_budget(&reference, &mut b),
                Err(StopReason::WorkLimit)
            );
            let before = b.usage();
            assert_eq!(
                registry.descriptor_with_budget(&reference, &mut b),
                Err(StopReason::WorkLimit)
            );
            assert_eq!(b.usage(), before);
        }
        let mut b = budget();
        b.charge(Resource::Work, 7).map_err(|e| format!("{e:?}"))?;
        b.cancel();
        let before = b.usage();
        assert_eq!(
            registry.descriptor_with_budget(&reference, &mut b),
            Err(StopReason::Cancelled)
        );
        assert_eq!(b.usage(), before);
        for mismatch in [false, true] {
            let mut missing = reference.clone();
            if mismatch {
                missing.revision += 1;
            } else {
                missing.package = "absent".into();
            }
            assert!(
                registry
                    .descriptor_with_budget(&missing, &mut budget())
                    .map_err(|e| format!("{e:?}"))?
                    .is_none()
            );
        }
    }
    let mut b = budget();
    let reference = registry.selected("first", 1).ok_or("first")?;
    assert!(
        SchemaRegistry::default()
            .descriptor_with_budget(reference, &mut b)
            .map_err(|e| format!("{e:?}"))?
            .is_none()
    );
    assert_eq!(b.usage().work, 1);
    Ok(())
}
