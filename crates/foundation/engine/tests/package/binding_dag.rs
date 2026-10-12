use super::*;

#[test]
fn unused_shared_binding_dag_does_not_expand_paths() -> TestResult {
    let (mut package, registry) = fixture()?;
    package.forms.clear();
    package.leaves.clear();
    package.bindings = vec![Binding::None];
    for i in 1..18 {
        package
            .bindings
            .push(Binding::Group(vec![BindingId(i - 1), BindingId(i - 1)]));
    }
    let mut measured = budget();
    package
        .check_detailed(&registry, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(measured.usage().depth, 18);
    Ok(())
}

fn shared(count: u64) -> Result<(LanguagePackage, SchemaRegistry), String> {
    let (mut package, registry) = fixture()?;
    package.forms.clear();
    package.leaves.clear();
    package.bindings = vec![Binding::None];
    for i in 1..count {
        package
            .bindings
            .push(Binding::Scope(vec![BindingId(i - 1), BindingId(i - 1)]));
    }
    Ok((package, registry))
}
#[test]
fn binding_graph_cost_tracks_nodes_and_edges_not_paths() -> TestResult {
    let mut costs = Vec::new();
    for count in [18, 36, 72] {
        let (package, registry) = shared(count)?;
        let mut b = budget();
        package
            .check_detailed(&registry, &mut b)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(b.usage().depth, count);
        costs.push(b.usage().work);
    }
    // Each added node is initialized, entered as a root and completed once;
    // each of its two child edges has one scheduling and one entry charge.
    assert_eq!(costs[1] - costs[0], 18 * 7);
    assert_eq!(costs[2] - costs[1], 36 * 7);
    Ok(())
}
#[test]
fn cached_binding_height_preserves_depth_and_caller_scope() -> TestResult {
    use nepl3_core::budget::StopReason;
    let (package, registry) = shared(18)?;
    for (limit, expected) in [(21, None), (20, Some(StopReason::DepthLimit))] {
        let mut b = Budget::new(Limits {
            depth: limit,
            ..budget().limits()
        });
        let result = b.with_depth_at_least(3, |b| package.check(&registry, b).map(|_| ()));
        assert_eq!(b.current_depth(), 0);
        match expected {
            None => {
                assert_eq!(result, Ok(()));
                assert_eq!(b.usage().depth, 21);
            }
            Some(reason) => assert_eq!(result, Err(PackageError::Stopped(reason))),
        }
    }
    Ok(())
}
#[test]
fn shared_graph_still_checks_unused_cycles_and_invalid_indices() -> TestResult {
    for (children, error, at) in [
        (
            vec![BindingId(17)],
            PackageError::DirectCycle,
            BindingId(17),
        ),
        (
            vec![BindingId(u64::MAX)],
            PackageError::InvalidBinding,
            BindingId(u64::MAX),
        ),
    ] {
        let (mut package, registry) = shared(18)?;
        package.bindings[17] = Binding::Group(children);
        let Err(failure) = package.check_detailed(&registry, &mut budget()) else {
            return Err("invalid graph accepted".into());
        };
        assert_eq!(failure.error, error);
        assert_eq!(
            failure.subject,
            Some(PackageSubject::Binding {
                owner: None,
                binding: Some(at)
            })
        );
    }
    let (mut package, registry) = shared(18)?;
    package.bindings[0] = Binding::Group(vec![BindingId(17)]);
    assert_eq!(
        package.check(&registry, &mut budget()).map(|_| ()),
        Err(PackageError::DirectCycle)
    );
    Ok(())
}
#[test]
fn binding_graph_limits_are_exact_and_sticky() -> TestResult {
    use nepl3_core::budget::{Resource, StopReason};
    let (package, registry) = shared(18)?;
    let mut full = budget();
    package
        .check(&registry, &mut full)
        .map_err(|e| format!("{e:?}"))?;
    for allocation in [false, true] {
        let exact = full.usage();
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = exact.allocation_units;
        } else {
            limits.work = exact.work;
        }
        assert_eq!(
            package
                .check(&registry, &mut Budget::new(limits))
                .map(|_| ()),
            Ok(())
        );
        let reason = if allocation {
            limits.allocation_units -= 1;
            StopReason::AllocationLimit
        } else {
            limits.work -= 1;
            StopReason::WorkLimit
        };
        let mut stopped = Budget::new(limits);
        assert_eq!(
            package.check(&registry, &mut stopped).map(|_| ()),
            Err(PackageError::Stopped(reason))
        );
        let used = stopped.usage();
        assert_eq!(stopped.charge(Resource::Work, 0), Err(reason));
        assert_eq!(stopped.usage(), used);
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        package.check(&registry, &mut b).map(|_| ()),
        Err(PackageError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn cached_depth_stop_names_the_admitted_entry_without_fabricated_traversal() -> TestResult {
    use nepl3_core::budget::StopReason;
    let (mut package, registry) = shared(1)?;
    package.bindings = vec![
        Binding::Group(vec![BindingId(1)]),
        Binding::Group(vec![BindingId(2)]),
        Binding::None,
        Binding::Group(vec![BindingId(4)]),
        Binding::Scope(vec![BindingId(0)]),
    ];
    let mut b = Budget::new(Limits {
        depth: 4,
        ..budget().limits()
    });
    let Err(failure) = package.check_detailed(&registry, &mut b) else {
        return Err("cached height accepted above ceiling".into());
    };
    assert_eq!(failure.error, PackageError::Stopped(StopReason::DepthLimit));
    assert_eq!(
        failure.subject,
        Some(PackageSubject::Binding {
            owner: None,
            binding: Some(BindingId(0))
        })
    );
    // Earlier root 0 established height 3. The later cached subtree is
    // admitted in one observation; its unvisited depth-4 prefix is not claimed.
    assert_eq!(b.usage().depth, 3);
    Ok(())
}
