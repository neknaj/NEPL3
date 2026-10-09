use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason, Usage},
    schema::{SchemaDescriptor, SchemaError, SchemaRegistry},
    value::NdfValue,
};
use nepl3_engine::{
    analysis::{BindingAccessError, completion::CandidateError},
    portable::{PortableError, analysis::access_error_to_value, completion::error::to_value},
};
const OWNERS: [&str; 3] = ["nepl3.reader", "nepl3.engine", "nepl3.foundation"];
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 100_000,
        nodes: 1000,
        depth: 100,
        ..Limits::default()
    })
}
fn err(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn encode(
    route: usize,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<core::convert::Infallible>> {
    match route {
        0 => access_error_to_value(BindingAccessError::Incomplete, registry, b),
        _ => to_value(&CandidateError::NoStage, registry, b),
    }
}
#[test]
fn value_owner_selections_admit_every_scan_before_errors_or_allocation() -> Result<(), String> {
    for available in 0..=3 {
        let padding = "unrelated".repeat(128);
        let mut registry = SchemaRegistry::default();
        let names: Vec<_> = core::iter::once(padding.as_str())
            .chain(OWNERS[..available].iter().copied())
            .collect();
        for name in &names {
            let descriptor = SchemaDescriptor {
                package: (*name).into(),
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            let reference = descriptor.reference(&mut budget()).map_err(err)?;
            registry
                .register(reference, descriptor, &mut budget())
                .map_err(err)?;
        }
        registry.finalize(&mut budget()).map_err(err)?;
        // Independently model each admitted entry/comparison, in registration
        // order, ending after the first missing owner or all three selections.
        let mut charges = Vec::new();
        for wanted in OWNERS.iter().take((available + 1).min(3)) {
            charges.push(1);
            for candidate in &names {
                charges.push((candidate.len() + wanted.len() + 9) as u64);
                if candidate == wanted {
                    break;
                }
            }
        }
        let full: u64 = charges.iter().sum();
        let mut caps = vec![0, full];
        let mut prefix = 0;
        for charge in &charges {
            prefix += charge;
            caps.push(prefix - 1);
            caps.push(prefix);
        }
        caps.sort_unstable();
        caps.dedup();
        for route in 0..2 {
            for prior in [0, 7] {
                for cap in &caps {
                    let mut accepted = 0;
                    for charge in &charges {
                        if accepted + charge > *cap {
                            break;
                        }
                        accepted += charge;
                    }
                    let mut limits = budget().limits();
                    limits.work = prior + cap;
                    limits.allocation_units = 0;
                    let mut b = Budget::new(limits);
                    b.charge(Resource::Work, prior).map_err(err)?;
                    let result = encode(route, &registry, &mut b);
                    if *cap < full {
                        assert!(
                            matches!(result, Err(PortableError::Stopped(StopReason::WorkLimit))),
                            "route {route}, available {available}, cap {cap}: {result:?}"
                        );
                        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                    } else if available < 3 {
                        assert!(matches!(
                            result,
                            Err(PortableError::Schema(SchemaError::UnknownSchema))
                        ));
                        assert_eq!(b.poll(), Ok(()));
                    } else {
                        assert!(matches!(
                            result,
                            Err(PortableError::Stopped(StopReason::AllocationLimit))
                        ));
                        assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
                    }
                    assert_eq!(
                        b.usage(),
                        Usage {
                            work: prior + accepted,
                            ..Usage::default()
                        }
                    );
                }
            }
            let mut b = budget();
            b.cancel();
            assert!(matches!(
                encode(route, &registry, &mut b),
                Err(PortableError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(b.usage(), Usage::default());
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
        }
    }
    Ok(())
}
