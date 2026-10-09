use super::*;
use nepl3_core::{
    budget::{Resource, StopReason},
    schema::SchemaDescriptor,
    source::Digest,
    value::SchemaRef,
};
fn fixture(padding: usize) -> Result<(SchemaRegistry, SchemaRef, ReaderContext), String> {
    let mut setup = budget();
    let d = foundation::descriptor(&mut setup).map_err(err)?;
    let schema = d.reference(&mut setup).map_err(err)?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), d, &mut setup)
        .map_err(err)?;
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry
            .register(d.reference(&mut setup).map_err(err)?, d, &mut setup)
            .map_err(err)?;
    }
    registry.finalize(&mut setup).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&environment, &schema, &registry, &mut setup).map_err(err)?;
    let raw = ReaderContext {
        schema: schema.clone(),
        category: "test".into(),
        mode: "test".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        origins: vec![],
    };
    Ok((registry, schema, raw))
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn context_check_meters_missing_schema_before_invalid_context() -> TestResult {
    for padding in [0, 32] {
        let (registry, _, mut raw) = fixture(padding)?;
        raw.schema.package = "missing".into();
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        let mut limits = budget().limits();
        limits.work = 0;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            raw.check(&mut codec, &sources, &registry, &mut stopped),
            Err(ContextError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.usage().allocation_units, 0);
        let mut b = budget();
        assert!(matches!(
            raw.check(&mut codec, &sources, &registry, &mut b),
            Err(ContextError::InvalidContext)
        ));
        // Exact foundation hit, followed by a complete missing-context scan.
        let expected =
            1 + (16 + 16 + 9) + (16 + 41) + 1 + (16 + 7 + 9) + padding as u64 * (10 + 7 + 9);
        assert_eq!(b.usage().work, expected);
        assert_eq!(b.usage().allocation_units, 0);
    }
    Ok(())
}

#[test]
fn both_retargets_meter_destination_membership_and_keep_checked_environment() -> TestResult {
    for padding in [0, 32] {
        let (registry, schema, raw) = fixture(padding)?;
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        let checked = raw
            .check(&mut codec, &sources, &registry, &mut budget())
            .map_err(err)?;
        let mut missing = schema.clone();
        missing.package = "missing".into();
        for preserving in [false, true] {
            let mut b = budget();
            let failed = if preserving {
                checked.retarget_preserving_sources(&missing, "next", "again", &registry, &mut b)
            } else {
                checked.retarget(&missing, "next", "again", &sources, &registry, &mut b)
            };
            assert!(matches!(failed, Err(ContextError::InvalidContext)));
            assert_eq!(b.usage().work, 1 + 99 + 33 + padding as u64 * 26);
            assert_eq!(b.usage().allocation_units, 0);
            let mut limits = budget().limits();
            limits.work = 1;
            let mut b = Budget::new(limits);
            let stopped = if preserving {
                checked.retarget_preserving_sources(&schema, "next", "again", &registry, &mut b)
            } else {
                checked.retarget(&schema, "next", "again", &sources, &registry, &mut b)
            };
            assert!(matches!(
                stopped,
                Err(ContextError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(b.usage().work, 1);
            let mut b = budget();
            let next = if preserving {
                checked.retarget_preserving_sources(&schema, "next", "again", &registry, &mut b)
            } else {
                checked.retarget(&schema, "next", "again", &sources, &registry, &mut b)
            }
            .map_err(err)?;
            assert_eq!(next.category, "next");
            assert_eq!(next.mode, "again");
            assert_eq!(next.environment, checked.environment);
            assert_eq!(next.origins, checked.origins);
            assert_eq!(next.sources().len(), checked.sources().len());
            let small_usage = b.usage();
            let category = "c".repeat(1024);
            let mode = "m".repeat(2048);
            let mut long_budget = budget();
            let long = if preserving {
                checked.retarget_preserving_sources(
                    &schema,
                    &category,
                    &mode,
                    &registry,
                    &mut long_budget,
                )
            } else {
                checked.retarget(
                    &schema,
                    &category,
                    &mode,
                    &sources,
                    &registry,
                    &mut long_budget,
                )
            }
            .map_err(err)?;
            assert_eq!(long.category, category);
            assert_eq!(long.mode, mode);
            let extra = (1024 - "next".len() + 2048 - "again".len()) as u64;
            let mut expected = small_usage;
            expected.work += extra;
            expected.allocation_units += extra;
            assert_eq!(long_budget.usage(), expected);
            let mut cancelled = budget();
            cancelled.charge(Resource::Work, 7).map_err(err)?;
            cancelled.cancel();
            let before = cancelled.usage();
            let result = if preserving {
                checked.retarget_preserving_sources(
                    &schema,
                    "next",
                    "again",
                    &registry,
                    &mut cancelled,
                )
            } else {
                checked.retarget(
                    &schema,
                    "next",
                    "again",
                    &sources,
                    &registry,
                    &mut cancelled,
                )
            };
            assert!(matches!(
                result,
                Err(ContextError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(cancelled.usage(), before);
        }
        let mut tampered = raw.clone();
        tampered.environment.digest = Digest([9; 32]);
        assert!(matches!(
            tampered.check(&mut codec, &sources, &registry, &mut budget()),
            Err(ContextError::DigestMismatch)
        ));
    }
    Ok(())
}

#[test]
fn context_namespace_lookup_precedes_digest_and_argument_validation() -> TestResult {
    use nepl3_core::{
        syntax::{EnvironmentBinding, NamespaceRef},
        value::{Record, TypedValue},
    };
    let mut costs = vec![];
    for padding in [0, 32] {
        let (registry, schema, mut raw) = fixture(padding)?;
        let mut missing = schema.clone();
        missing.package = "missing".into();
        raw.environment.value.bindings.push(EnvironmentBinding {
            namespace: NamespaceRef {
                schema: missing,
                name: "Value".into(),
            },
            name: "x".into(),
            value: TypedValue::Record(Record {
                schema,
                kind: "Missing".into(),
                fields: vec![],
            }),
            origin: None,
        });
        // Keep the old empty-environment digest deliberately: namespace
        // admission must reject before either argument shape or digest checks.
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
        let mut b = budget();
        assert!(matches!(
            raw.check(&mut codec, &sources, &registry, &mut b),
            Err(ContextError::InvalidContext)
        ));
        costs.push(b.usage());
    }
    let mut expected = costs[0];
    expected.work += 32 * (10 + 7 + 9);
    assert_eq!(costs[1], expected);
    Ok(())
}
