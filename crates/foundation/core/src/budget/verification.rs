//! Bit-precise verification of production resource transitions and applications.
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

fn arbitrary_budget() -> Budget {
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
    Budget {
        limits,
        usage,
        depth,
        stopped,
        observed_depth,
    }
}

/// All u64 inputs, all seven counters and all nine prior stops are admissible.
/// The expected addition uses u128, independent of checked_add in production.
#[kani::proof]
fn charge_preserves_contract_and_unrelated_state() {
    let mut budget = arbitrary_budget();
    let limits = budget.limits;
    let usage = budget.usage;
    let depth = budget.depth;
    let observed_depth = budget.observed_depth;
    let stopped = budget.stopped;
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

/// No assumptions relate active depth, old marks or the current limit.
/// Use u128 addition to specify the production observation independently.
#[kani::proof]
fn depth_observation_preserves_contract_and_unrelated_state() {
    let mut budget = arbitrary_budget();
    let limits = budget.limits;
    let usage = budget.usage;
    let depth = budget.depth;
    let observed_depth = budget.observed_depth;
    let stopped = budget.stopped;
    let relative: u64 = kani::any();
    let sum = u128::from(depth) + u128::from(relative);
    let mut expected_usage = usage;
    let mut expected_observed = observed_depth;
    let expected_stop = match stopped {
        Some(reason) => Some(reason),
        None if sum <= u128::from(limits.depth) => {
            expected_usage.depth = usage.depth.max(sum as u64);
            expected_observed = observed_depth.max(sum as u64);
            None
        }
        None => Some(StopReason::DepthLimit),
    };
    let result = budget.observe_depth(relative);
    assert_eq!(budget.usage, expected_usage);
    assert_eq!(budget.observed_depth, expected_observed);
    assert_eq!(budget.stopped, expected_stop);
    assert_eq!(result, expected_stop.map_or(Ok(()), Err));
    assert_eq!(budget.limits, limits);
    assert_eq!(budget.depth, depth);
}

/// Every field can independently overflow or exceed a lowered ceiling.
/// Unlike ordinary charging, admitted completed work is recorded after stop.
#[kani::proof]
#[kani::unwind(10)]
fn observed_usage_is_atomic_and_preserves_prior_stop() {
    let mut budget = arbitrary_budget();
    let limits = budget.limits;
    let usage = budget.usage;
    let depth = budget.depth;
    let measured = budget.observed_depth;
    let stopped = budget.stopped;
    let observed = arbitrary_budget().usage;
    let fields = [
        (
            u128::from(usage.source_bytes) + u128::from(observed.source_bytes),
            limits.source_bytes,
            StopReason::SourceLimit,
        ),
        (
            u128::from(usage.work) + u128::from(observed.work),
            limits.work,
            StopReason::WorkLimit,
        ),
        (
            u128::from(usage.depth.max(observed.depth)),
            limits.depth,
            StopReason::DepthLimit,
        ),
        (
            u128::from(usage.nodes) + u128::from(observed.nodes),
            limits.nodes,
            StopReason::NodeLimit,
        ),
        (
            u128::from(usage.allocation_units) + u128::from(observed.allocation_units),
            limits.allocation_units,
            StopReason::AllocationLimit,
        ),
        (
            u128::from(usage.output_bytes) + u128::from(observed.output_bytes),
            limits.output_bytes,
            StopReason::OutputLimit,
        ),
        (
            u128::from(usage.diagnostics) + u128::from(observed.diagnostics),
            limits.diagnostics,
            StopReason::DiagnosticLimit,
        ),
        (
            u128::from(usage.events) + u128::from(observed.events),
            limits.events,
            StopReason::EventLimit,
        ),
    ];
    let mut failure = None;
    for (sum, limit, reason) in fields {
        if failure.is_none() && sum > u128::from(limit) {
            failure = Some(reason);
        }
    }
    let expected = if failure.is_none() {
        Usage {
            source_bytes: fields[0].0 as u64,
            work: fields[1].0 as u64,
            depth: fields[2].0 as u64,
            nodes: fields[3].0 as u64,
            allocation_units: fields[4].0 as u64,
            output_bytes: fields[5].0 as u64,
            diagnostics: fields[6].0 as u64,
            events: fields[7].0 as u64,
        }
    } else {
        usage
    };
    let expected_measured = if failure.is_none() {
        measured.max(observed.depth)
    } else {
        measured
    };
    let expected_stop = stopped.or(failure);
    let result = budget.record_observed_usage(observed);
    assert_eq!(budget.usage, expected);
    assert_eq!(budget.observed_depth, expected_measured);
    assert_eq!(budget.stopped, expected_stop);
    assert_eq!(result, expected_stop.map_or(Ok(()), Err));
    assert_eq!(budget.limits, limits);
    assert_eq!(budget.depth, depth);
}

/// Model an arbitrary callback's terminating post-state and Result; this proves
/// the wrapper boundary, not callback computations or unwind restoration.
fn depth_scope_contract(base: Option<u64>) {
    let mut budget = arbitrary_budget();
    let limits = budget.limits;
    let usage = budget.usage;
    let active = budget.depth;
    let measured = budget.observed_depth;
    let stopped = budget.stopped;
    let target = match base {
        Some(base) => u128::from(active.max(base)),
        None => u128::from(active) + 1,
    };
    let rejected = stopped.or(if target > u128::from(limits.depth) {
        Some(StopReason::DepthLimit)
    } else {
        None
    });
    let replacement = arbitrary_budget();
    let post_limits = replacement.limits;
    let post_usage = replacement.usage;
    let post_measured = replacement.observed_depth;
    let post_stopped = replacement.stopped;
    let callback_result: Result<u64, StopReason> = if kani::any() {
        Ok(kani::any())
    } else {
        Err(reason())
    };
    let mut calls = 0u8;
    let callback = |inner: &mut Budget| {
        assert_eq!(calls, 0);
        calls += 1;
        assert_eq!(rejected, None);
        let mut entered_usage = usage;
        entered_usage.depth = usage.depth.max(target as u64);
        assert_eq!(inner.limits, limits);
        assert_eq!(inner.usage, entered_usage);
        assert_eq!(inner.depth, target as u64);
        assert_eq!(inner.observed_depth, measured.max(target as u64));
        assert_eq!(inner.stopped, None);
        *inner = replacement;
        callback_result
    };
    let result = match base {
        Some(base) => budget.with_depth_at_least(base, callback),
        None => budget.with_depth(callback),
    };
    assert_eq!(budget.depth, active);
    if let Some(reason) = rejected {
        assert_eq!(calls, 0);
        assert_eq!(result, Err(reason));
        assert_eq!(budget.limits, limits);
        assert_eq!(budget.usage, usage);
        assert_eq!(budget.observed_depth, measured);
        assert_eq!(budget.stopped, Some(reason));
    } else {
        assert_eq!(calls, 1);
        assert_eq!(result, callback_result);
        assert_eq!(budget.limits, post_limits);
        assert_eq!(budget.usage, post_usage);
        assert_eq!(budget.observed_depth, post_measured);
        assert_eq!(budget.stopped, post_stopped);
    }
}

#[kani::proof]
fn next_depth_scope_restores_only_saved_active_depth() {
    depth_scope_contract(None);
}

#[kani::proof]
fn restored_depth_scope_restores_only_saved_active_depth() {
    depth_scope_contract(Some(kani::any()));
}

/// Arbitrary callback replacement is modeled: only the saved outer limits
/// are restored. Result-returning callbacks are modeled, not panic or divergence.
#[kani::proof]
#[kani::unwind(10)]
fn ceiling_scope_preserves_priority_and_restores_only_limits() {
    let mut budget = arbitrary_budget();
    let requested = arbitrary_budget().limits;
    let outer = budget.limits;
    let usage = budget.usage;
    let active = budget.depth;
    let measured = budget.observed_depth;
    let stopped = budget.stopped;
    // An independent field-wise oracle specifies intersection and priority.
    let pairs = [
        (
            outer.source_bytes,
            requested.source_bytes,
            usage.source_bytes,
            StopReason::SourceLimit,
        ),
        (
            outer.work,
            requested.work,
            usage.work,
            StopReason::WorkLimit,
        ),
        (
            outer.nodes,
            requested.nodes,
            usage.nodes,
            StopReason::NodeLimit,
        ),
        (
            outer.allocation_units,
            requested.allocation_units,
            usage.allocation_units,
            StopReason::AllocationLimit,
        ),
        (
            outer.output_bytes,
            requested.output_bytes,
            usage.output_bytes,
            StopReason::OutputLimit,
        ),
        (
            outer.diagnostics,
            requested.diagnostics,
            usage.diagnostics,
            StopReason::DiagnosticLimit,
        ),
        (
            outer.events,
            requested.events,
            usage.events,
            StopReason::EventLimit,
        ),
        (
            outer.depth,
            requested.depth,
            usage.depth,
            StopReason::DepthLimit,
        ),
    ];
    let mut effective = [0u64; 8];
    let mut rejected = stopped;
    for (i, (old, new, used, reason)) in pairs.into_iter().enumerate() {
        effective[i] = if old < new { old } else { new };
        if rejected.is_none() && (used > old || used > new) {
            rejected = Some(reason);
        }
    }
    let expected_limits = Limits {
        source_bytes: effective[0],
        work: effective[1],
        nodes: effective[2],
        allocation_units: effective[3],
        output_bytes: effective[4],
        diagnostics: effective[5],
        events: effective[6],
        depth: effective[7],
    };
    let replacement = arbitrary_budget();
    let post_usage = replacement.usage;
    let post_active = replacement.depth;
    let post_measured = replacement.observed_depth;
    let post_stopped = replacement.stopped;
    let callback_result: Result<u64, StopReason> = if kani::any() {
        Ok(kani::any())
    } else {
        Err(reason())
    };
    let mut calls = 0u8;
    let result = budget.with_ceiling(requested, |inner| {
        assert_eq!(calls, 0);
        calls += 1;
        assert_eq!(rejected, None);
        assert_eq!(inner.limits, expected_limits);
        assert_eq!(inner.usage, usage);
        assert_eq!(inner.depth, active);
        assert_eq!(inner.observed_depth, measured);
        assert_eq!(inner.stopped, None);
        *inner = replacement;
        callback_result
    });
    assert_eq!(budget.limits, outer);
    if let Some(reason) = rejected {
        assert_eq!(calls, 0);
        assert_eq!(result, Err(reason));
        assert_eq!(budget.usage, usage);
        assert_eq!(budget.depth, active);
        assert_eq!(budget.observed_depth, measured);
        assert_eq!(budget.stopped, Some(reason));
    } else {
        assert_eq!(calls, 1);
        assert_eq!(result, callback_result);
        assert_eq!(budget.usage, post_usage);
        assert_eq!(budget.depth, post_active);
        assert_eq!(budget.observed_depth, post_measured);
        assert_eq!(budget.stopped, post_stopped);
    }
}

/// The callback is modeled by arbitrary post-state, not proved or authorized.
/// In particular, this boundary model does not weaken the public restriction
/// to pure validation callbacks that never replace the Budget.
#[kani::proof]
fn measurement_scope_merges_marks_and_preserves_error_priority() {
    let mut budget = arbitrary_budget();
    let limits = budget.limits;
    let usage = budget.usage;
    let base = budget.depth;
    let previous = budget.observed_depth;
    let stopped = budget.stopped;
    let replacement = arbitrary_budget();
    let post_limits = replacement.limits;
    let post_usage = replacement.usage;
    let post_active = replacement.depth;
    let post_measured = replacement.observed_depth;
    let post_stopped = replacement.stopped;
    let callback_result: Result<u64, StopReason> = if kani::any() {
        Ok(kani::any())
    } else {
        Err(reason())
    };
    let mut calls = 0u8;
    let result = budget.measure_depth(|inner| {
        assert_eq!(calls, 0);
        calls += 1;
        assert_eq!(stopped, None);
        assert_eq!(inner.limits, limits);
        assert_eq!(inner.usage, usage);
        assert_eq!(inner.depth, base);
        assert_eq!(inner.observed_depth, base);
        assert_eq!(inner.stopped, None);
        *inner = replacement;
        callback_result
    });
    if let Some(reason) = stopped {
        assert_eq!(calls, 0);
        assert_eq!(result, Err(reason));
        assert_eq!(budget.limits, limits);
        assert_eq!(budget.usage, usage);
        assert_eq!(budget.depth, base);
        assert_eq!(budget.observed_depth, previous);
        assert_eq!(budget.stopped, stopped);
    } else {
        assert_eq!(calls, 1);
        assert_eq!(budget.limits, post_limits);
        assert_eq!(budget.usage, post_usage);
        assert_eq!(budget.depth, post_active);
        assert_eq!(budget.stopped, post_stopped);
        let merged = if previous > post_measured {
            previous
        } else {
            post_measured
        };
        assert_eq!(budget.observed_depth, merged);
        let difference = i128::from(post_measured) - i128::from(base);
        let relative = if difference > 0 { difference as u64 } else { 0 };
        let expected = match callback_result {
            Err(reason) => Err(reason),
            Ok(value) => match post_stopped {
                Some(reason) => Err(reason),
                None => Ok((value, relative)),
            },
        };
        assert_eq!(result, expected);
    }
}
