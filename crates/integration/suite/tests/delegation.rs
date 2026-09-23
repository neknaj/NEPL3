use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};
use nepl3_suite::suspension::delegation::{IssuedBudget, LocalFailure, SettlementError};

fn settled(result: Result<(), SettlementError>) -> Result<(), StopReason> {
    match result {
        Ok(()) => Ok(()),
        Err(SettlementError::Stopped(reason)) => Err(reason),
        Err(SettlementError::ObservationExceedsGrant) => Err(StopReason::Cancelled),
    }
}

fn limits(n: u64) -> Limits {
    Limits {
        source_bytes: n,
        work: n,
        depth: n,
        nodes: n,
        allocation_units: n,
        output_bytes: n,
        diagnostics: n,
        events: n,
    }
}
fn usage(n: u64) -> Usage {
    Usage {
        source_bytes: n,
        work: n,
        depth: n,
        nodes: n,
        allocation_units: n,
        output_bytes: n,
        diagnostics: n,
        events: n,
    }
}
fn resources() -> [(Resource, StopReason); 7] {
    [
        (Resource::SourceBytes, StopReason::SourceLimit),
        (Resource::Work, StopReason::WorkLimit),
        (Resource::Nodes, StopReason::NodeLimit),
        (Resource::AllocationUnits, StopReason::AllocationLimit),
        (Resource::OutputBytes, StopReason::OutputLimit),
        (Resource::Diagnostics, StopReason::DiagnosticLimit),
        (Resource::Events, StopReason::EventLimit),
    ]
}

#[test]
fn reserved_capacity_and_actual_usage_remain_distinct() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    parent.record_observed_usage(usage(3))?;
    let mut issued = IssuedBudget::issue(&mut parent, limits(4))?;
    assert_eq!(issued.parent_usage(), usage(3));
    assert_eq!(issued.limits(), limits(4));
    issued
        .run_local(|b| {
            for (resource, _) in resources() {
                b.charge(resource, 3)?;
            }
            b.observe_depth(5)
        })
        .map_err(|failure| failure.reason)?;
    settled(issued.settle(usage(4)))?;
    assert_eq!(
        parent.usage(),
        Usage {
            depth: 5,
            ..usage(10)
        }
    );
    assert_eq!(parent.limits(), limits(10));
    Ok(())
}

#[test]
fn local_exhaustion_keeps_remote_capacity_and_first_stop() -> Result<(), StopReason> {
    for (resource, reason) in resources() {
        let mut parent = Budget::new(limits(10));
        let mut issued = IssuedBudget::issue(&mut parent, limits(4))?;
        assert_eq!(
            issued.run_local(|b| b.charge(resource, 7)),
            Err(LocalFailure {
                reason,
                output: None
            })
        );
        // Authenticated work already completed remotely is still recorded.
        assert_eq!(
            issued.settle(usage(4)),
            Err(SettlementError::Stopped(reason))
        );
        assert_eq!(parent.usage(), usage(4));
        assert_eq!(parent.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn unused_capacity_returns_only_after_settlement() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    settled(IssuedBudget::issue(&mut parent, limits(8))?.settle(usage(2)))?;
    parent.charge(Resource::Work, 8)?;
    assert_eq!(parent.usage().work, 10);
    let mut cancelled = Budget::new(limits(10));
    drop(IssuedBudget::issue(&mut cancelled, limits(8))?);
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    assert_eq!(cancelled.usage(), Usage::default());
    Ok(())
}

#[test]
fn oversized_or_unknown_observations_never_enter_accounting() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    parent.record_observed_usage(usage(3))?;
    assert!(IssuedBudget::issue(&mut parent, limits(8)).is_err());
    assert_eq!(parent.usage(), usage(3));
    assert_eq!(parent.poll(), Err(StopReason::SourceLimit));
    let mut parent = Budget::new(limits(10));
    let grant = IssuedBudget::issue(&mut parent, limits(4))?;
    assert_eq!(
        grant.settle(usage(5)),
        Err(SettlementError::ObservationExceedsGrant)
    );
    assert_eq!(parent.usage(), Usage::default());
    let mut parent = Budget::new(limits(10));
    assert!(
        IssuedBudget::issue(
            &mut parent,
            Limits {
                depth: 11,
                ..limits(0)
            }
        )
        .is_err()
    );
    assert_eq!(parent.poll(), Err(StopReason::DepthLimit));
    Ok(())
}

#[test]
fn nested_grants_cannot_duplicate_available_capacity() -> Result<(), StopReason> {
    let mut parent = Budget::new(limits(10));
    let mut outer = IssuedBudget::issue(&mut parent, limits(4))?;
    outer
        .run_local(|local| settled(IssuedBudget::issue(local, limits(6))?.settle(usage(6))))
        .map_err(|failure| failure.reason)?;
    settled(outer.settle(usage(4)))?;
    assert_eq!(
        parent.usage(),
        Usage {
            depth: 6,
            ..usage(10)
        }
    );
    Ok(())
}

#[test]
fn zero_maximum_and_prior_stop_boundaries() -> Result<(), StopReason> {
    let mut zero = Budget::new(limits(0));
    settled(IssuedBudget::issue(&mut zero, limits(0))?.settle(usage(0)))?;
    assert_eq!(zero.poll(), Ok(()));
    let mut maximum = Budget::new(limits(u64::MAX));
    settled(IssuedBudget::issue(&mut maximum, limits(u64::MAX))?.settle(usage(u64::MAX)))?;
    assert_eq!(maximum.usage(), usage(u64::MAX));
    assert_eq!(
        maximum.charge(Resource::Work, 1),
        Err(StopReason::WorkLimit)
    );
    assert!(IssuedBudget::issue(&mut maximum, limits(0)).is_err());
    assert_eq!(maximum.poll(), Err(StopReason::WorkLimit));
    Ok(())
}

#[test]
fn every_observation_field_is_checked_before_commit() -> Result<(), StopReason> {
    for field in 0..8 {
        let mut parent = Budget::new(limits(10));
        parent.record_observed_usage(usage(2))?;
        let grant = IssuedBudget::issue(&mut parent, limits(4))?;
        let mut observation = usage(1);
        match field {
            0 => observation.source_bytes = 5,
            1 => observation.work = 5,
            2 => observation.depth = 5,
            3 => observation.nodes = 5,
            4 => observation.allocation_units = 5,
            5 => observation.output_bytes = 5,
            6 => observation.diagnostics = 5,
            _ => observation.events = 5,
        }
        assert_eq!(
            grant.settle(observation),
            Err(SettlementError::ObservationExceedsGrant)
        );
        assert_eq!(parent.usage(), usage(2));
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
    }
    Ok(())
}

#[test]
fn ancestor_scope_covers_local_and_delegated_work() -> Result<(), StopReason> {
    use nepl3_suite::suspension::execution::ExecutionScope;
    let mut parent = Budget::new(limits(100));
    let scope = ExecutionScope::root(&mut parent, limits(10))?;
    scope.run(&mut parent, |parent| {
        let mut issued = IssuedBudget::issue(parent, limits(6))?;
        issued
            .run_local(|local| local.charge(Resource::Work, 4))
            .map_err(|failure| failure.reason)?;
        let mut remote = Budget::new(issued.limits());
        remote.charge(Resource::Work, 6)?;
        remote.observe_depth(3)?;
        // A locally owned metered execution supplies an independent observation.
        settled(issued.settle(remote.usage()))
    })?;
    assert_eq!(parent.limits(), limits(100));
    assert_eq!(parent.usage().work, 10);
    assert_eq!(parent.usage().depth, 3);
    assert_eq!(
        scope.run(&mut parent, |b| b.charge(Resource::Work, 1)),
        Err(StopReason::WorkLimit)
    );
    Ok(())
}
