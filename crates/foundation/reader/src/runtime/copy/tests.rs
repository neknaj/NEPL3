use super::*;
use alloc::{format, vec};
use nepl3_core::budget::Limits;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        depth: 100,
        nodes: 10000,
        allocation_units: 1_000_000,
        output_bytes: 1_000_000,
        diagnostics: 10,
        events: 10,
    })
}
fn sample(name: &str) -> Result<Span, String> {
    let source = SourceSnapshot::new(
        SourceId(name.into()),
        7,
        "memory:span".into(),
        vec![b'a'],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    source.span(0, 1).map_err(|e| format!("{e:?}"))
}
fn clone_bytes(name: &str) -> u64 {
    if cfg!(target_has_atomic = "ptr") {
        0
    } else {
        name.len() as u64
    }
}
#[test]
fn span_clone_and_nested_copies_use_actual_storage_but_compare_keeps_old_bound()
-> Result<(), String> {
    for name in ["s".into(), "界🙂".repeat(64)] {
        let value = sample(&name)?;
        let mut cloned = budget();
        assert_eq!(
            copy(&value, &mut cloned).map_err(|e| format!("{e:?}"))?,
            value
        );
        let bytes = clone_bytes(&name);
        assert_eq!(cloned.usage().work, bytes + 1);
        assert_eq!(
            cloned.usage().allocation_units,
            core::mem::size_of::<Span>() as u64 + bytes
        );
        let mut compared = budget();
        value.charge(&mut compared).map_err(|e| format!("{e:?}"))?;
        assert_eq!(compared.usage().work, name.len() as u64 + 2);
        assert_eq!(
            compared.usage().allocation_units,
            (core::mem::size_of::<Span>() + core::mem::size_of::<String>() + name.len()) as u64
        );
        // Reusing the very same identity never changes Compare into Clone.
        let nested = vec![value.clone(), value];
        let mut nested_clone = budget();
        let output = copy(&nested, &mut nested_clone).map_err(|e| format!("{e:?}"))?;
        assert_eq!(output, nested);
        assert_eq!(nested_clone.usage().work, 1 + 2 * (bytes + 1));
        assert_eq!(
            nested_clone.usage().allocation_units,
            core::mem::size_of::<Vec<Span>>() as u64
                + 2 * (core::mem::size_of::<Span>() as u64 + bytes)
        );
        let mut nested_compare = budget();
        nested
            .charge(&mut nested_compare)
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(nested_compare.usage().work, 1 + 2 * (name.len() as u64 + 2));
        assert_eq!(
            nested_compare.usage().allocation_units,
            core::mem::size_of::<Vec<Span>>() as u64 + 2 * compared.usage().allocation_units
        );
    }
    Ok(())
}
#[test]
fn span_clone_exact_limits_and_stops_remain_sticky() -> Result<(), String> {
    let name = "界🙂".repeat(64);
    let value = sample(&name)?;
    let mut measured = budget();
    let _ = copy(&value, &mut measured).map_err(|e| format!("{e:?}"))?;
    let limits = Limits {
        work: measured.usage().work,
        allocation_units: measured.usage().allocation_units,
        ..budget().limits()
    };
    assert_eq!(
        copy(&value, &mut Budget::new(limits)).map_err(|e| format!("{e:?}"))?,
        value
    );
    for (limits, reason) in [
        (
            Limits {
                work: limits.work - 1,
                ..limits
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: limits.allocation_units - 1,
                ..limits
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert_eq!(copy(&value, &mut b), Err(reason));
        assert_eq!(b.poll(), Err(reason));
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(copy(&value, &mut b), Err(StopReason::Cancelled));
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}
#[test]
fn span_compare_preserves_prior_charge_order_at_failures() -> Result<(), String> {
    let value = sample(&"界🙂".repeat(64))?;
    let size = core::mem::size_of::<Span>() as u64;
    for limits in [
        Limits {
            work: 0,
            ..budget().limits()
        },
        Limits {
            work: 1,
            ..budget().limits()
        },
        Limits {
            work: 2,
            ..budget().limits()
        },
        Limits {
            allocation_units: size - 1,
            ..budget().limits()
        },
        Limits {
            allocation_units: size + core::mem::size_of::<String>() as u64 - 1,
            ..budget().limits()
        },
        budget().limits(),
    ] {
        let mut actual = Budget::new(limits);
        let mut prior = Budget::new(limits);
        let result = value.charge(&mut actual);
        // Independent retained pre-change traversal; do not invoke Span's
        // new CopyCost branch while constructing the expected sequence.
        let expected = (|| {
            slot::<Span>(&mut prior)?;
            value
                .snapshot_ref()
                .source
                .0
                .charge_for(&mut prior, CopyPurpose::Compare)
        })();
        assert_eq!(result, expected);
        assert_eq!(actual.usage(), prior.usage());
        assert_eq!(actual.poll(), prior.poll());
    }
    Ok(())
}
