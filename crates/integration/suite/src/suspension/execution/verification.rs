//! Heap-free checks of production saved-scope derivation, not scheduler proof.
use super::*;
use nepl3_core::budget::Usage;

fn any_limits() -> Limits {
    Limits {
        source_bytes: kani::any(),
        work: kani::any(),
        depth: kani::any(),
        nodes: kani::any(),
        allocation_units: kani::any(),
        output_bytes: kani::any(),
        diagnostics: kani::any(),
        events: kani::any(),
    }
}
fn unlimited() -> Limits {
    Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    }
}
fn same_limits(a: Limits, b: Limits) {
    assert_eq!(a.source_bytes, b.source_bytes);
    assert_eq!(a.work, b.work);
    assert_eq!(a.depth, b.depth);
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.allocation_units, b.allocation_units);
    assert_eq!(a.output_bytes, b.output_bytes);
    assert_eq!(a.diagnostics, b.diagnostics);
    assert_eq!(a.events, b.events);
}
fn same_usage(a: Usage, b: Usage) {
    assert_eq!(a.source_bytes, b.source_bytes);
    assert_eq!(a.work, b.work);
    assert_eq!(a.depth, b.depth);
    assert_eq!(a.nodes, b.nodes);
    assert_eq!(a.allocation_units, b.allocation_units);
    assert_eq!(a.output_bytes, b.output_bytes);
    assert_eq!(a.diagnostics, b.diagnostics);
    assert_eq!(a.events, b.events);
}
fn check(parent: ExecutionScope, request: Limits, result: Result<ExecutionScope, StopReason>) {
    let depth = parent.depth as u128 + 1;
    let cap = parent.limits.depth.min(request.depth);
    if depth > u64::MAX as u128 || depth > cap as u128 {
        assert!(matches!(result, Err(StopReason::DepthLimit)));
    } else {
        assert!(result.is_ok());
        if let Ok(child) = result {
            assert_eq!(child.depth as u128, depth);
            assert_eq!(
                child.limits.source_bytes,
                parent.limits.source_bytes.min(request.source_bytes)
            );
            assert_eq!(child.limits.work, parent.limits.work.min(request.work));
            assert_eq!(child.limits.depth, cap);
            assert_eq!(child.limits.nodes, parent.limits.nodes.min(request.nodes));
            assert_eq!(
                child.limits.allocation_units,
                parent.limits.allocation_units.min(request.allocation_units)
            );
            assert_eq!(
                child.limits.output_bytes,
                parent.limits.output_bytes.min(request.output_bytes)
            );
            assert_eq!(
                child.limits.diagnostics,
                parent.limits.diagnostics.min(request.diagnostics)
            );
            assert_eq!(
                child.limits.events,
                parent.limits.events.min(request.events)
            );
        }
    }
}

#[kani::proof]
fn saved_scope_derivation_and_composition() {
    let parent = ExecutionScope {
        limits: any_limits(),
        depth: kani::any(),
    };
    let request = any_limits();
    let next_request = any_limits();
    let child = admission::transition(parent, request);
    check(parent, request, child);
    if let Ok(child) = child {
        let next = admission::transition(child, next_request);
        check(child, next_request, next);
        if let Ok(next) = next {
            assert_eq!(next.depth as u128, parent.depth as u128 + 2);
            same_limits(
                next.limits,
                Limits {
                    source_bytes: parent
                        .limits
                        .source_bytes
                        .min(request.source_bytes)
                        .min(next_request.source_bytes),
                    work: parent.limits.work.min(request.work).min(next_request.work),
                    depth: parent
                        .limits
                        .depth
                        .min(request.depth)
                        .min(next_request.depth),
                    nodes: parent
                        .limits
                        .nodes
                        .min(request.nodes)
                        .min(next_request.nodes),
                    allocation_units: parent
                        .limits
                        .allocation_units
                        .min(request.allocation_units)
                        .min(next_request.allocation_units),
                    output_bytes: parent
                        .limits
                        .output_bytes
                        .min(request.output_bytes)
                        .min(next_request.output_bytes),
                    diagnostics: parent
                        .limits
                        .diagnostics
                        .min(request.diagnostics)
                        .min(next_request.diagnostics),
                    events: parent
                        .limits
                        .events
                        .min(request.events)
                        .min(next_request.events),
                },
            );
        }
    }
}

#[kani::proof]
fn saved_scope_child_preserves_shared_budget() {
    let parent = ExecutionScope {
        limits: any_limits(),
        depth: kani::any(),
    };
    let request = any_limits();
    let mut budget = Budget::new(unlimited());
    let observed = Usage {
        source_bytes: kani::any(),
        work: kani::any(),
        depth: kani::any(),
        nodes: kani::any(),
        allocation_units: kani::any(),
        output_bytes: kani::any(),
        diagnostics: kani::any(),
        events: kani::any(),
    };
    assert!(budget.record_observed_usage(observed).is_ok());
    let prior = if kani::any::<bool>() {
        Some(match kani::any::<u8>() % 9 {
            0 => StopReason::Cancelled,
            1 => StopReason::SourceLimit,
            2 => StopReason::WorkLimit,
            3 => StopReason::DepthLimit,
            4 => StopReason::NodeLimit,
            5 => StopReason::AllocationLimit,
            6 => StopReason::OutputLimit,
            7 => StopReason::DiagnosticLimit,
            _ => StopReason::EventLimit,
        })
    } else {
        None
    };
    if let Some(reason) = prior {
        budget.stop(reason);
    }
    let before = budget.usage();
    let result = parent.child(request, &mut budget);
    same_usage(budget.usage(), before);
    same_limits(budget.limits(), unlimited());
    assert_eq!(budget.current_depth(), 0);
    if let Some(reason) = prior {
        assert!(matches!(result, Err(actual) if actual == reason));
        assert_eq!(budget.poll(), Err(reason));
    } else {
        check(parent, request, result);
        assert_eq!(budget.poll().err(), result.err());
    }
}
