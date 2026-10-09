use super::*;
use crate::profile::{ParseProfile, RuntimeCatalog};
use nepl3_core::{
    budget::{Limits, Usage},
    schema::{SchemaDescriptor, SchemaError},
};
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
    alloc::format!("{value:?}")
}
#[test]
fn diagnostic_owner_lookup_admits_work_before_allocation_or_missing_schema() -> Result<(), String> {
    for present in [false, true] {
        let mut registry = SchemaRegistry::default();
        // A long unrelated owner makes comparison admission observable, rather
        // than relying on a coincidentally failing later one-unit charge.
        let padding = "p".repeat(1024);
        for owner in [Some(padding.as_str()), present.then_some("nepl3.engine")]
            .into_iter()
            .flatten()
        {
            let descriptor = SchemaDescriptor {
                package: owner.into(),
                revision: 1,
                types: Vec::new(),
                operations: Vec::new(),
            };
            let reference = descriptor.reference(&mut budget()).map_err(err)?;
            registry
                .register(reference, descriptor, &mut budget())
                .map_err(err)?;
        }
        registry.finalize(&mut budget()).map_err(err)?;
        let profile = ParseProfile {
            id: "diagnostic-admission".into(),
            languages: Vec::new(),
            schemas: Vec::new(),
            category_modes: Vec::new(),
            head_providers: Vec::new(),
            providers: Vec::new(),
            allowlist: Vec::new(),
            resources: Vec::new(),
            limits: budget().limits(),
        };
        let resolved = profile
            .resolve(
                &RuntimeCatalog {
                    packages: &[],
                    providers: &[],
                    resources: &[],
                },
                &registry,
                &mut budget(),
            )
            .map_err(err)?;
        // Entry + each package/revision comparison. No exact-reference
        // comparison belongs to selected-owner lookup.
        let first = 1 + padding.len() as u64 + "nepl3.engine".len() as u64 + 9;
        let full = first
            + if present {
                2 * "nepl3.engine".len() as u64 + 9
            } else {
                0
            };
        for code in ["UndefinedName", "AmbiguousName", "DuplicateGlobalName"] {
            for prior in [0, 7] {
                for (cap, accepted) in [
                    (0, 0),
                    (1, 1),
                    (first - 1, 1),
                    (full - 1, if present { first } else { 1 }),
                    (full, full),
                ] {
                    let mut limits = budget().limits();
                    limits.work = prior + cap;
                    limits.allocation_units = 0;
                    let mut b = Budget::new(limits);
                    b.charge(Resource::Work, prior).map_err(err)?;
                    let mut machine = Machine {
                        profile: &resolved,
                        registry: &registry,
                        bundles: Vec::new(),
                        progress: BindingProgress::empty(),
                        report: Report::default(),
                        reference_trace: None,
                        birth_trace: None,
                    };
                    let result = machine.diagnostic_optional_at(
                        code,
                        NamespaceRef(0),
                        "x",
                        (None, None),
                        &mut b,
                    );
                    let expected = if cap < full {
                        BindingError::Stopped(StopReason::WorkLimit)
                    } else if present {
                        BindingError::Stopped(StopReason::AllocationLimit)
                    } else {
                        BindingError::Schema(SchemaError::UnknownSchema)
                    };
                    assert_eq!(result, Err(expected));
                    assert_eq!(
                        b.usage(),
                        Usage {
                            work: prior + accepted,
                            ..Usage::default()
                        }
                    );
                    if cap < full {
                        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                    } else if present {
                        assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
                    } else {
                        assert_eq!(b.poll(), Ok(()));
                    }
                    assert_eq!(machine.report, Report::default());
                    assert!(machine.progress.facts.is_none());
                }
            }
            let mut b = budget();
            b.cancel();
            let mut machine = Machine {
                profile: &resolved,
                registry: &registry,
                bundles: Vec::new(),
                progress: BindingProgress::empty(),
                report: Report::default(),
                reference_trace: None,
                birth_trace: None,
            };
            assert_eq!(
                machine.diagnostic_optional_at(code, NamespaceRef(0), "x", (None, None), &mut b),
                Err(BindingError::Stopped(StopReason::Cancelled))
            );
            assert_eq!(b.usage(), Usage::default());
            assert_eq!(machine.report, Report::default());
        }
    }
    Ok(())
}
