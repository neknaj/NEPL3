//! Bit-precise verification of the production charge transition and application.
//! No assumptions exclude overflow, lowered ceilings, or pre-existing stops.
use super::*;

fn reason() -> StopReason {
    match kani::any::<u8>() % 9 {
        0 => StopReason::Cancelled,
        1 => StopReason::SourceLimit,
        2 => StopReason::WorkLimit,
        3 => StopReason::DepthLimit,
        4 => StopReason::NodeLimit,
        5 => StopReason::AllocationLimit,
        6 => StopReason::OutputLimit,
        7 => StopReason::DiagnosticLimit,
        _ => StopReason::EventLimit,
    }
}

/// All u64 inputs, all seven counters and all nine prior stops are admissible.
/// The expected addition uses u128, independent of checked_add in production.
#[kani::proof]
fn charge_preserves_contract_and_unrelated_state() {
    let limits = Limits {
        source_bytes: kani::any(),
        work: kani::any(),
        depth: kani::any(),
        nodes: kani::any(),
        allocation_units: kani::any(),
        output_bytes: kani::any(),
        diagnostics: kani::any(),
        events: kani::any(),
    };
    let usage = Usage {
        source_bytes: kani::any(),
        work: kani::any(),
        depth: kani::any(),
        nodes: kani::any(),
        allocation_units: kani::any(),
        output_bytes: kani::any(),
        diagnostics: kani::any(),
        events: kani::any(),
    };
    let depth = kani::any();
    let observed_depth = kani::any();
    let stopped = if kani::any() { Some(reason()) } else { None };
    let mut budget = Budget {
        limits,
        usage,
        depth,
        stopped,
        observed_depth,
    };
    let resource = match kani::any::<u8>() % 7 {
        0 => Resource::SourceBytes,
        1 => Resource::Work,
        2 => Resource::Nodes,
        3 => Resource::AllocationUnits,
        4 => Resource::OutputBytes,
        5 => Resource::Diagnostics,
        _ => Resource::Events,
    };
    let amount: u64 = kani::any();
    let mut expected = usage;
    let (counter, limit, failure) = match resource {
        Resource::SourceBytes => (
            &mut expected.source_bytes,
            limits.source_bytes,
            StopReason::SourceLimit,
        ),
        Resource::Work => (&mut expected.work, limits.work, StopReason::WorkLimit),
        Resource::Nodes => (&mut expected.nodes, limits.nodes, StopReason::NodeLimit),
        Resource::AllocationUnits => (
            &mut expected.allocation_units,
            limits.allocation_units,
            StopReason::AllocationLimit,
        ),
        Resource::OutputBytes => (
            &mut expected.output_bytes,
            limits.output_bytes,
            StopReason::OutputLimit,
        ),
        Resource::Diagnostics => (
            &mut expected.diagnostics,
            limits.diagnostics,
            StopReason::DiagnosticLimit,
        ),
        Resource::Events => (&mut expected.events, limits.events, StopReason::EventLimit),
    };
    let sum = u128::from(*counter) + u128::from(amount);
    let expected_stop = match stopped {
        Some(reason) => Some(reason),
        None if sum <= u128::from(limit) => {
            *counter = sum as u64;
            None
        }
        None => Some(failure),
    };
    let result = budget.charge(resource, amount);
    assert_eq!(budget.usage, expected);
    assert_eq!(budget.stopped, expected_stop);
    assert_eq!(result, expected_stop.map_or(Ok(()), Err));
    assert_eq!(budget.limits, limits);
    assert_eq!(budget.depth, depth);
    assert_eq!(budget.observed_depth, observed_depth);
}
