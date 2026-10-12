use super::*;
use nepl3_core::budget::StopReason;

fn named(namespace: &str) -> Result<(LanguagePackage, SchemaRegistry), String> {
    let (mut package, registry) = fixture()?;
    package.namespaces[0].name = namespace.into();
    for binding in &mut package.bindings {
        if let Binding::Bind {
            namespace: name, ..
        }
        | Binding::Reference {
            namespace: name, ..
        } = binding
        {
            *name = namespace.into();
        }
    }
    Ok((package, registry))
}

#[test]
fn namespace_bytes_are_metered_in_owner_and_package_validation() -> TestResult {
    let mut costs = Vec::new();
    for namespace in ["x".into(), "界".repeat(4096)] {
        let (package, registry) = named(&namespace)?;
        let mut owner = budget();
        package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut owner)
            .map_err(|e| format!("{e:?}"))?;
        let mut full = budget();
        package
            .check_detailed(&registry, &mut full)
            .map_err(|e| format!("{e:?}"))?;
        costs.push((owner.usage().work, full.usage().work));
    }
    // One leaf namespace comparison, two owner comparisons in the full package.
    // Each UTF-8 byte pair requires three Work, admitted before inspection.
    let delta = 3 * (3 * 4096 - 1);
    assert_eq!(costs[1].0 - costs[0].0, delta);
    assert_eq!(costs[1].1 - costs[0].1, delta * 2);
    Ok(())
}

#[test]
fn namespace_limit_is_exact_sticky_and_attributed() -> TestResult {
    let (package, registry) = named(&"界".repeat(1024))?;
    let mut measured = budget();
    package
        .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    let work = measured.usage().work;
    let mut exact = Budget::new(Limits {
        work,
        ..budget().limits()
    });
    assert!(
        package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut exact)
            .is_ok()
    );
    for limit in [0, 100, work - 1] {
        let mut short = Budget::new(Limits {
            work: limit,
            ..budget().limits()
        });
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut short)
            .err()
            .ok_or("namespace comparison must fit before success")?;
        assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
        if limit > 0 {
            assert_eq!(failure.binding, Some(BindingId(3)));
        }
        let used = short.usage();
        assert_eq!(
            package
                .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut short)
                .err()
                .ok_or("stop is sticky")?
                .error,
            PackageError::Stopped(StopReason::WorkLimit)
        );
        assert_eq!(short.usage(), used);
    }
    Ok(())
}

#[test]
fn namespace_lookup_charges_only_visited_candidates_and_prefixes() -> TestResult {
    let (mut package, registry) = named("target")?;
    let mut measured = budget();
    package
        .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    let work = measured.usage().work;
    for _ in 0..4096 {
        package.namespaces.push(Namespace {
            name: "unvisited".repeat(128),
            policy: NamespacePolicy::Lexical,
        });
    }
    // Direct owner validation does not inspect later namespace declarations.
    let mut exact = Budget::new(Limits {
        work,
        ..budget().limits()
    });
    package
        .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut exact)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(exact.usage().work, work);
    for name in [
        "zarget".into(),
        "z".into(),
        format!("z{}", "x".repeat(65536)),
    ] {
        package.namespaces = vec![Namespace {
            name,
            policy: NamespacePolicy::Lexical,
        }];
        let mut b = budget();
        let failure = package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("different first byte cannot match")?;
        assert_eq!(failure.error, PackageError::MissingNamespace);
        assert_eq!(failure.binding, Some(BindingId(3)));
        // Unequal lengths reject before bytes; equal lengths inspect one pair.
        assert_eq!(
            b.usage().work,
            4 + 1
                + 1
                + if package.namespaces[0].name.len() == 6 {
                    3
                } else {
                    0
                }
        );
    }
    Ok(())
}

#[test]
fn namespace_late_mismatch_and_full_package_stop_keep_subject() -> TestResult {
    let long = "界".repeat(1024);
    let (mut package, registry) = named(&format!("{long}a"))?;
    package.namespaces[0].name = format!("{long}b");
    let mut b = budget();
    let failure = package
        .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
        .err()
        .ok_or("last byte differs")?;
    assert_eq!(failure.error, PackageError::MissingNamespace);
    assert_eq!(b.usage().work, 4 + 1 + 1 + 3 * (long.len() as u64 + 1));
    let mut measured = budget();
    let missing = package
        .check_detailed(&registry, &mut measured)
        .map(|_| ())
        .err()
        .ok_or("whole package must reject the namespace too")?;
    assert_eq!(missing.error, PackageError::MissingNamespace);
    let mut short = Budget::new(Limits {
        work: measured.usage().work - 1,
        ..budget().limits()
    });
    let failure = package
        .check_detailed(&registry, &mut short)
        .map(|_| ())
        .err()
        .ok_or("exact budget is required")?;
    assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
    assert_eq!(
        failure.subject,
        Some(PackageSubject::Binding {
            owner: Some(BindingOwner::Form(0)),
            binding: Some(BindingId(0))
        })
    );
    let mut cancelled = budget();
    cancelled.stop(StopReason::Cancelled);
    let used = cancelled.usage();
    assert_eq!(
        package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut cancelled)
            .err()
            .ok_or("cancelled")?
            .error,
        PackageError::Stopped(StopReason::Cancelled)
    );
    assert_eq!(cancelled.usage(), used);
    Ok(())
}

#[test]
fn field_lookup_does_not_charge_untouched_name_suffixes() -> TestResult {
    let (mut package, registry) = fixture()?;
    package.bindings = vec![Binding::Visit("z".into())];
    package.forms[0].binding = BindingId(0);
    for name in ["x".into(), format!("x{}", "y".repeat(65536))] {
        package.forms[0].fields[0].name = name;
        let mut b = Budget::new(Limits {
            work: 20,
            ..budget().limits()
        });
        let failure = package
            .check_binding_owner(BindingOwner::Form(0), &registry, &mut b)
            .err()
            .ok_or("unknown field")?;
        assert_eq!(failure.error, PackageError::InvalidSelector);
        assert_eq!(failure.binding, Some(BindingId(0)));
        assert_eq!(
            b.usage().work,
            if package.forms[0].fields[0].name.len() == 1 {
                1 + 1 + 4 + 1
            } else {
                1 + 1 + 1 + 1
            }
        );
    }
    Ok(())
}

#[test]
fn unequal_length_common_prefix_names_do_not_scan_bytes() -> TestResult {
    let prefix = "界".repeat(4096);
    let (mut package, registry) = named(&format!("{prefix}x"))?;
    package.namespaces[0].name = prefix.clone();
    let mut b = Budget::new(Limits {
        work: 100,
        ..budget().limits()
    });
    assert_eq!(
        package
            .check_binding_owner(BindingOwner::Leaf(0), &registry, &mut b)
            .err()
            .ok_or("different lengths")?
            .error,
        PackageError::MissingNamespace
    );
    assert_eq!(b.usage().work, 4 + 1 + 1);
    package.bindings = vec![Binding::Visit(format!("{prefix}x"))];
    package.forms[0].fields[0].name = prefix;
    package.forms[0].binding = BindingId(0);
    let mut b = Budget::new(Limits {
        work: 100,
        ..budget().limits()
    });
    assert_eq!(
        package
            .check_binding_owner(BindingOwner::Form(0), &registry, &mut b)
            .err()
            .ok_or("different field lengths")?
            .error,
        PackageError::InvalidSelector
    );
    assert_eq!(b.usage().work, 1 + 1 + 1 + 1);
    Ok(())
}
