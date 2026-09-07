use nepl3_core::budget::{Budget, Limits, Resource, StopReason, Usage};

fn limits(cap: u64) -> Limits {
    Limits {
        source_bytes: cap,
        work: cap,
        depth: cap,
        nodes: cap,
        allocation_units: cap,
        output_bytes: cap,
        diagnostics: cap,
        events: cap,
    }
}
fn usage(amount: u64) -> Usage {
    Usage {
        source_bytes: amount,
        work: amount,
        depth: amount,
        nodes: amount,
        allocation_units: amount,
        output_bytes: amount,
        diagnostics: amount,
        events: amount,
    }
}

#[test]
fn observed_delegated_cost_is_atomic_across_all_resources_and_preserves_stops() {
    for resource in 0..8 {
        let mut b = Budget::new(limits(10));
        assert_eq!(b.record_observed_usage(usage(3)), Ok(()));
        let before = b.usage();
        let mut observed = usage(1);
        let reason = match resource {
            0 => {
                observed.source_bytes = 8;
                StopReason::SourceLimit
            }
            1 => {
                observed.work = 8;
                StopReason::WorkLimit
            }
            2 => {
                observed.depth = 11;
                StopReason::DepthLimit
            }
            3 => {
                observed.nodes = 8;
                StopReason::NodeLimit
            }
            4 => {
                observed.allocation_units = 8;
                StopReason::AllocationLimit
            }
            5 => {
                observed.output_bytes = 8;
                StopReason::OutputLimit
            }
            6 => {
                observed.diagnostics = 8;
                StopReason::DiagnosticLimit
            }
            _ => {
                observed.events = 8;
                StopReason::EventLimit
            }
        };
        assert_eq!(b.record_observed_usage(observed), Err(reason));
        assert_eq!(b.usage(), before); // No earlier resource was partially applied.
        assert_eq!(b.record_observed_usage(usage(2)), Err(reason));
        assert_eq!(
            b.usage(),
            Usage {
                depth: 3,
                ..usage(5)
            }
        );
        assert_eq!(b.charge(Resource::Work, 0), Err(reason));
    }
    let mut b = Budget::new(limits(10));
    b.cancel();
    assert_eq!(
        b.record_observed_usage(usage(4)),
        Err(StopReason::Cancelled)
    );
    assert_eq!(b.usage(), usage(4));
    assert_eq!(
        b.record_observed_usage(usage(7)),
        Err(StopReason::Cancelled)
    );
    assert_eq!(b.usage(), usage(4));
    let mut overflow = Budget::new(limits(u64::MAX));
    assert_eq!(overflow.record_observed_usage(usage(u64::MAX)), Ok(()));
    assert_eq!(
        overflow.record_observed_usage(usage(1)),
        Err(StopReason::SourceLimit)
    );
    assert_eq!(overflow.usage(), usage(u64::MAX));
}

#[test]
fn temporary_ceilings_restore_on_result_paths_and_cannot_regrant_reserved_capacity() {
    let outer = limits(100);
    let mut b = Budget::new(outer);
    let result: Result<(), StopReason> = b.with_ceiling(limits(10), |b| {
        assert_eq!(b.limits(), limits(10));
        b.with_ceiling(outer, |b| {
            assert_eq!(b.limits(), limits(10));
            b.charge(Resource::Work, 10)
        })?;
        // An ordinary error restores ceilings without inventing a sticky stop.
        Err(StopReason::Cancelled)
    });
    assert_eq!(result, Err(StopReason::Cancelled));
    assert_eq!(b.limits(), outer);
    assert_eq!(b.poll(), Ok(()));
    assert_eq!(b.usage().work, 10);
    let mut entered = false;
    let result: Result<(), StopReason> = b.with_ceiling(limits(9), |_| {
        entered = true;
        Ok(())
    });
    assert_eq!(result, Err(StopReason::WorkLimit));
    assert!(!entered);
    assert_eq!(b.limits(), outer);
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut b = Budget::new(outer);
    let result: Result<(), StopReason> =
        b.with_ceiling(limits(10), |b| b.charge(Resource::Events, 11));
    assert_eq!(result, Err(StopReason::EventLimit));
    assert_eq!(b.limits(), outer);
    assert_eq!(b.usage(), Usage::default());
    assert_eq!(b.poll(), Err(StopReason::EventLimit));
}
