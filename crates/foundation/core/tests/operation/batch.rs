use super::*;
use nepl3_core::operation::{
    Invoke,
    lifetime::{LifetimeError, RequestLifetimes, RequestPhase},
};

fn fixture() -> (RequestLifetimes, Invoke, Vec<Invoke>) {
    let saved = saved();
    let parent = Invoke {
        request_id: 7,
        operation: saved.provider.clone(),
        input: saved.state.clone(),
        environment: saved.state,
        sources: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let mut table = RequestLifetimes::default();
    assert_eq!(
        table.begin_call(&parent, saved.snapshot_digest, None, &mut budget()),
        Ok(())
    );
    assert_eq!(
        table.begin(
            9,
            parent.operation.clone(),
            saved.snapshot_digest,
            &mut budget()
        ),
        Ok(())
    );
    assert_eq!(table.finish(9, &mut budget()), Ok(()));
    let calls = [11, 3]
        .into_iter()
        .map(|id| {
            let mut call = parent.clone();
            call.request_id = id;
            call.operation.name = "child".into();
            call
        })
        .collect();
    (table, parent, calls)
}

fn unchanged(table: &RequestLifetimes) {
    assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Running));
    assert_eq!(table.phase(9, &mut budget()), Ok(RequestPhase::Finished));
    for id in [3, 11] {
        assert_eq!(
            table.phase(id, &mut budget()),
            Err(LifetimeError::UnknownRequest)
        );
    }
}

#[test]
fn batch_suspends_parent_and_preserves_child_ancestry_in_id_order() {
    let (mut table, _, calls) = fixture();
    let snapshot = saved().snapshot_digest;
    assert_eq!(
        table.suspend_calls(
            7,
            saved(),
            &[(&calls[0], snapshot), (&calls[1], snapshot)],
            &mut budget()
        ),
        Ok(())
    );
    assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Awaiting));
    for id in [3, 11] {
        assert_eq!(table.phase(id, &mut budget()), Ok(RequestPhase::Running));
    }
    let mut cancelled = vec![];
    assert_eq!(
        table.cancel_tree(7, &mut budget(), |id| cancelled.push(id)),
        Ok(())
    );
    assert_eq!(cancelled, [3, 7, 11]);
    assert_eq!(table.phase(9, &mut budget()), Ok(RequestPhase::Finished));
}

#[test]
fn batch_rejects_late_duplicates_cycles_and_binding_without_publishing_prefix() {
    for mode in 0..4 {
        let (mut table, parent, mut calls) = fixture();
        let mut continuation = saved();
        let expected = match mode {
            0 => {
                calls[1].request_id = calls[0].request_id;
                LifetimeError::DuplicateRequest
            }
            1 => {
                calls[1].request_id = 9;
                LifetimeError::DuplicateRequest
            }
            2 => {
                calls[1].operation = parent.operation;
                LifetimeError::CyclicOperation
            }
            _ => {
                continuation.snapshot_digest = Digest::of(b"wrong");
                LifetimeError::Binding(ContinuationError::Snapshot)
            }
        };
        let snapshot = saved().snapshot_digest;
        assert_eq!(
            table.suspend_calls(
                7,
                continuation,
                &[(&calls[0], snapshot), (&calls[1], snapshot)],
                &mut budget()
            ),
            Err(expected)
        );
        unchanged(&table);
    }
}

#[test]
fn every_insufficient_batch_budget_leaves_all_request_states_unchanged() {
    let (mut table, _, calls) = fixture();
    let snapshot = saved().snapshot_digest;
    let batch = [(&calls[0], snapshot), (&calls[1], snapshot)];
    let mut measured = budget();
    assert_eq!(
        table.suspend_calls(7, saved(), &batch, &mut measured),
        Ok(())
    );
    for (allocation, amount) in [
        (false, measured.usage().work),
        (true, measured.usage().allocation_units),
    ] {
        for limit in 0..amount {
            let (mut table, _, _) = fixture();
            let mut limits = budget().limits();
            let reason = if allocation {
                limits.allocation_units = limit;
                StopReason::AllocationLimit
            } else {
                limits.work = limit;
                StopReason::WorkLimit
            };
            let mut limited = Budget::new(limits);
            assert_eq!(
                table.suspend_calls(7, saved(), &batch, &mut limited),
                Err(LifetimeError::Stopped(reason)),
                "limit {limit}"
            );
            assert_eq!(limited.poll(), Err(reason));
            unchanged(&table);
        }
    }
}

#[test]
fn batch_growth_keeps_sorted_lookup_and_subquadratic_work() {
    fn measure(existing: usize, children: usize) -> u64 {
        let (mut table, parent, _) = fixture();
        let snapshot = saved().snapshot_digest;
        for index in 0..existing {
            assert_eq!(
                table.begin(
                    1000 + 2 * index as u64,
                    parent.operation.clone(),
                    snapshot,
                    &mut budget()
                ),
                Ok(())
            );
        }
        // Descending odd IDs interleave with unrelated even IDs after sorting.
        let calls: Vec<_> = (0..children)
            .rev()
            .map(|index| {
                let mut call = parent.clone();
                call.request_id = 1001 + 2 * index as u64;
                call.operation.name = "child".into();
                call
            })
            .collect();
        let batch: Vec<_> = calls.iter().map(|call| (call, snapshot)).collect();
        // This growth probe materializes hundreds of typed calls; its capacity
        // is separate from the small fixture's stop-boundary budgets above.
        let mut measured = Budget::new(Limits {
            work: 10_000_000,
            allocation_units: 10_000_000,
            ..budget().limits()
        });
        assert_eq!(
            table.suspend_calls(7, saved(), &batch, &mut measured),
            Ok(())
        );
        for index in 0..existing {
            assert_eq!(
                table.phase(1000 + 2 * index as u64, &mut budget()),
                Ok(RequestPhase::Running)
            );
        }
        for call in &calls {
            assert_eq!(
                table.phase(call.request_id, &mut budget()),
                Ok(RequestPhase::Running)
            );
        }
        assert_eq!(table.phase(7, &mut budget()), Ok(RequestPhase::Awaiting));
        let mut cancelled = vec![];
        assert_eq!(
            table.cancel_tree(7, &mut budget(), |id| cancelled.push(id)),
            Ok(())
        );
        let expected: Vec<_> = core::iter::once(7)
            .chain((0..children).map(|i| 1001 + 2 * i as u64))
            .collect();
        assert_eq!(cancelled, expected);
        measured.usage().work
    }
    for axis in [false, true] {
        let values: Vec<_> = [128, 256, 512]
            .into_iter()
            .map(|n| {
                if axis {
                    measure(n, 128)
                } else {
                    measure(128, n)
                }
            })
            .collect();
        // Doubling either input axis admits logarithmic index/sort overhead;
        // repeated quadratic insertion tends toward a factor of four.
        for pair in values.windows(2) {
            assert!(pair[1] * 2 < pair[0] * 5, "axis {axis}: {values:?}");
        }
    }
}
