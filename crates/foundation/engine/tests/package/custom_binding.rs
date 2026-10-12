use super::*;
use nepl3_core::{budget::StopReason, value::OperationRef};
fn custom() -> Result<(LanguagePackage, SchemaRegistry), String> {
    let (mut package, registry) = fixture()?;
    let ty = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.engine".into(),
            revision: 1,
            name: name.into(),
        })
    };
    let op = OperationRef {
        schema: registry
            .selected("nepl3.engine", 1)
            .ok_or("engine")?
            .clone(),
        name: "bindingFacts".into(),
    };
    package.extensions = vec![ExtensionRequirement {
        alias: "facts".into(),
        provider: "fixture".into(),
        signature: "facts/v1".into(),
        operation: op.clone(),
        input: ty("FactsRequest"),
        output: ty("FactsReply"),
        pure: true,
    }];
    package.bindings = vec![Binding::Custom(op)];
    package.forms[0].binding = BindingId(0);
    package.leaves[0].binding = BindingId(0);
    Ok((package, registry))
}
fn owner_work(package: &LanguagePackage, registry: &SchemaRegistry) -> Result<u64, String> {
    let mut b = budget();
    package
        .check_binding_owner(BindingOwner::Form(0), registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    Ok(b.usage().work)
}
#[test]
fn custom_binding_counts_schema_and_operation_bytes() -> TestResult {
    for change_package in [true, false] {
        let mut costs = Vec::new();
        for name in ["x".into(), "界".repeat(4096)] {
            let (mut package, registry) = custom()?;
            let Binding::Custom(op) = &mut package.bindings[0] else {
                return Err("custom".into());
            };
            if change_package {
                op.schema.package = name.clone();
            } else {
                op.name = name.clone();
            }
            package.extensions[0].operation = op.clone();
            costs.push(owner_work(&package, &registry)?);
        }
        assert_eq!(costs[1] - costs[0], 3 * (3 * 4096 - 1));
    }
    Ok(())
}
#[test]
fn custom_binding_ignores_unvisited_extension_suffix() -> TestResult {
    let (mut package, registry) = custom()?;
    let work = owner_work(&package, &registry)?;
    for _ in 0..4096 {
        package.extensions.push(package.extensions[0].clone());
    }
    let mut b = Budget::new(Limits {
        work,
        ..budget().limits()
    });
    package
        .check_binding_owner(BindingOwner::Form(0), &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(b.usage().work, work);
    Ok(())
}

#[test]
fn custom_binding_limits_and_full_package_attribution_are_preserved() -> TestResult {
    let (package, registry) = custom()?;
    let work = owner_work(&package, &registry)?;
    for owner in [BindingOwner::Form(0), BindingOwner::Leaf(0)] {
        let mut exact = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        package
            .check_binding_owner(owner, &registry, &mut exact)
            .map_err(|e| format!("{e:?}"))?;
        let mut short = Budget::new(Limits {
            work: work - 1,
            ..budget().limits()
        });
        let failure = package
            .check_binding_owner(owner, &registry, &mut short)
            .err()
            .ok_or("short")?;
        assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
        assert_eq!(failure.binding, Some(BindingId(0)));
        let used = short.usage();
        assert_eq!(
            package
                .check_binding_owner(owner, &registry, &mut short)
                .err()
                .ok_or("sticky")?
                .error,
            PackageError::Stopped(StopReason::WorkLimit)
        );
        assert_eq!(short.usage(), used);
    }
    package
        .check_detailed(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut package = package;
    let Binding::Custom(op) = &mut package.bindings[0] else {
        return Err("custom".into());
    };
    op.name = "bindingFactx".into();
    let mut measured = budget();
    let failure = package
        .check_detailed(&registry, &mut measured)
        .err()
        .ok_or("missing")?;
    assert_eq!(failure.error, PackageError::MissingExtension);
    let subject = Some(PackageSubject::Binding {
        owner: Some(BindingOwner::Form(0)),
        binding: Some(BindingId(0)),
    });
    assert_eq!(failure.subject, subject);
    let mut short = Budget::new(Limits {
        work: measured.usage().work - 1,
        ..budget().limits()
    });
    let failure = package
        .check_detailed(&registry, &mut short)
        .err()
        .ok_or("short")?;
    assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
    assert_eq!(failure.subject, subject);
    Ok(())
}

#[test]
fn custom_binding_requires_complete_identity_and_facts_signature() -> TestResult {
    for change in 0..5 {
        let (mut package, registry) = custom()?;
        match change {
            0 => package.extensions[0].operation.schema.revision += 1,
            1 => package.extensions[0].operation.schema.digest.0[31] ^= 1,
            2 => package.extensions[0].operation.schema.package.push('x'),
            3 => package.extensions[0].operation.name.push('x'),
            _ => package.extensions[0].signature = "other/v1".into(),
        }
        let failure = package
            .check_binding_owner(BindingOwner::Form(0), &registry, &mut budget())
            .err()
            .ok_or("nonmatch")?;
        assert_eq!(failure.error, PackageError::MissingExtension);
        assert_eq!(failure.binding, Some(BindingId(0)));
    }
    let (mut package, registry) = custom()?;
    let valid = package.extensions[0].clone();
    package.extensions[0].signature = "other/v1".into();
    package.extensions.push(valid);
    package
        .check_binding_owner(BindingOwner::Form(0), &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut cancelled = budget();
    cancelled.stop(StopReason::Cancelled);
    let used = cancelled.usage();
    assert_eq!(
        package
            .check_binding_owner(BindingOwner::Form(0), &registry, &mut cancelled)
            .err()
            .ok_or("cancelled")?
            .error,
        PackageError::Stopped(StopReason::Cancelled)
    );
    assert_eq!(cancelled.usage(), used);
    Ok(())
}

#[test]
fn custom_binding_stops_inside_long_identity_and_short_circuits_prefixes() -> TestResult {
    let (mut package, registry) = custom()?;
    let long = "界".repeat(4096);
    let Binding::Custom(op) = &mut package.bindings[0] else {
        return Err("custom".into());
    };
    op.name = format!("a{long}");
    package.extensions[0].operation = op.clone();
    let mut schema = budget();
    assert!(
        op.schema
            .equal_with_budget(&package.extensions[0].operation.schema, &mut schema)
            .map_err(|e| format!("{e:?}"))?
    );
    // One cache slot and one graph entry precede this exact schema comparison.
    let prefix = 2 + schema.usage().work;
    let mut short = Budget::new(Limits {
        work: prefix + 1 + 3 * 5,
        ..budget().limits()
    });
    let failure = package
        .check_binding_owner(BindingOwner::Form(0), &registry, &mut short)
        .err()
        .ok_or("long name stopped")?;
    assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
    assert_eq!(failure.binding, Some(BindingId(0)));
    assert_eq!(short.usage().work, prefix + 1 + 3 * 5);
    package.extensions[0].operation.name = format!("b{long}");
    let mut first = Budget::new(Limits {
        work: prefix + 4,
        ..budget().limits()
    });
    let failure = package
        .check_binding_owner(BindingOwner::Form(0), &registry, &mut first)
        .err()
        .ok_or("first byte mismatch")?;
    assert_eq!(failure.error, PackageError::MissingExtension);
    assert_eq!(failure.binding, Some(BindingId(0)));
    assert_eq!(first.usage().work, prefix + 4);
    Ok(())
}
