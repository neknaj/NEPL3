use super::{TestResult, budget, fixture};
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaDescriptor, SchemaRegistry},
    source::Digest,
    value::OperationRef,
};
use nepl3_engine::{profile::*, selection::HeadProviderRef};
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn profile_head_schema_admission_is_distinct_from_prior_profile_scans() -> TestResult {
    let (package, original) = fixture()?;
    let identity = package
        .check(&original, &mut budget())
        .map_err(error)?
        .semantic_identity(&mut budget())
        .map_err(error)?;
    let schemas = [
        "nepl3.foundation",
        "nepl3.reader",
        "nepl3.engine",
        "fixture.syntax",
    ]
    .into_iter()
    .map(|name| original.selected(name, 1).cloned().ok_or("schema"))
    .collect::<Result<Vec<_>, _>>()?;
    let engine = schemas[2].clone();
    let shape = OperationRef {
        schema: engine.clone(),
        name: "headShape".into(),
    };
    let child = OperationRef {
        schema: engine,
        name: "headChildContext".into(),
    };
    let host = ProviderImplementation {
        provider: "head-test".into(),
        revision: 1,
        implementation_digest: Digest::of(b"head fixture"),
        operations: vec![shape.clone(), child.clone()],
    };
    let mut profile = ParseProfile {
        id: "heads".into(),
        languages: vec![LanguageRegistration {
            alias: "A".into(),
            package: identity,
            default_category: "Expr".into(),
        }],
        schemas: schemas.clone(),
        category_modes: vec![],
        head_providers: vec![HeadRegistration {
            alias: "A".into(),
            category: "Expr".into(),
            provider: HeadProviderRef {
                shape: shape.clone(),
                child_context: child.clone(),
            },
        }],
        providers: [&shape, &child]
            .into_iter()
            .map(|op| ProviderRequirement {
                provider: host.provider.clone(),
                revision: 1,
                implementation_digest: host.implementation_digest,
                operation: op.clone(),
            })
            .collect(),
        allowlist: vec![shape, child],
        resources: vec![],
        limits: budget().limits(),
    };
    let heads = profile.head_providers.clone();
    let packages = [&package];
    let hosts = [host];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &hosts,
        resources: &[],
    };
    let mut without = Vec::new();
    let mut with = Vec::new();
    let mut identities = Vec::new();
    for padding in [0, 16] {
        let mut registry = SchemaRegistry::default();
        for i in 0..padding {
            let descriptor = SchemaDescriptor {
                package: format!("padding.{i:02}.{}", "x".repeat(64)),
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            registry
                .register(
                    descriptor.reference(&mut budget()).map_err(error)?,
                    descriptor,
                    &mut budget(),
                )
                .map_err(error)?;
        }
        for schema in &schemas {
            registry
                .register(
                    schema.clone(),
                    original.descriptor(schema).ok_or("descriptor")?.clone(),
                    &mut budget(),
                )
                .map_err(error)?;
        }
        registry.finalize(&mut budget()).map_err(error)?;
        profile.head_providers.clear();
        let mut base = budget();
        let no_heads = profile
            .resolve(&catalog, &registry, &mut base)
            .map_err(error)?
            .digest();
        without.push(base.usage());
        profile.head_providers = heads.clone();
        let mut checked = budget();
        let resolved = profile
            .resolve(&catalog, &registry, &mut checked)
            .map_err(error)?;
        let actual_heads = resolved
            .head_provider("A", "Expr", &mut budget())
            .map_err(error)?;
        assert_eq!(actual_heads, Some(&heads[0].provider));
        identities.push((no_heads, resolved.digest()));
        with.push(checked.usage());
        if padding != 0 {
            // Credit all padding work already needed without head registrations.
            // A raw head descriptor scan would still succeed at this exact cap.
            let old_cap = with[0].work + (without[1].work - without[0].work);
            let mut stopped = Budget::new(Limits {
                work: old_cap,
                ..budget().limits()
            });
            assert!(matches!(
                profile.resolve(&catalog, &registry, &mut stopped),
                Err(ProfileError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        }
    }
    assert_eq!(identities[0], identities[1]);
    // Difference-of-differences cancels schema closure, provider admission,
    // package checks, execution identity, and all other earlier registry scans.
    // One head registration checks shape and child_context: two extra scans.
    let expected = 2 * 16 * (("padding.00.".len() + 64) + "nepl3.engine".len() + 9) as u64;
    assert_eq!(
        with[1].work - with[0].work - (without[1].work - without[0].work),
        expected
    );
    let mut expected_usage = with[0];
    expected_usage.work = with[1].work;
    assert_eq!(with[1], expected_usage);
    Ok(())
}
