use super::*;
use alloc::vec;
use nepl3_core::budget::{Limits, StopReason};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 100_000,
        nodes: 100_000,
        depth: 100,
        ..Limits::default()
    })
}

#[test]
fn canonical_hash_reuses_frontier_storage_and_retains_stop_boundaries() -> Result<(), WireError> {
    let value = NdfValue::List(
        (0..64)
            .map(|_| NdfValue::List(vec![NdfValue::Unit; 64]))
            .collect(),
    );
    // NDF1 List: [7, children], Unit: [0]. Encode the independent expected
    // CBOR directly, including the two-byte length of each 64-element array.
    let mut bytes = vec![0x82, 7, 0x98, 64];
    for _ in 0..64 {
        bytes.extend_from_slice(&[0x82, 7, 0x98, 64]);
        for _ in 0..64 {
            bytes.extend_from_slice(&[0x81, 0]);
        }
    }
    let expected = Digest::domain(b"frontier", &bytes);
    assert_eq!(encode(&value, &mut budget())?, bytes);
    let mut measured = budget();
    assert_eq!(digest(b"frontier", &value, &mut measured)?, expected);
    let used = measured.usage();
    assert_eq!(used.nodes, 1 + 64 + 64 * 64);
    // At most 127 siblings coexist. Allocation follows the rounded frontier,
    // rather than the 4,161 visits; no encoded byte buffer is materialized.
    assert!(used.allocation_units <= 128 * core::mem::size_of::<(Option<&NdfValue>, u64)>() as u64);
    for (amount, reason) in [
        (used.work, StopReason::WorkLimit),
        (used.allocation_units, StopReason::AllocationLimit),
        (used.output_bytes, StopReason::OutputLimit),
        (used.nodes, StopReason::NodeLimit),
        (used.depth, StopReason::DepthLimit),
    ] {
        for below in [false, true] {
            let mut limits = budget().limits();
            let limit = amount - u64::from(below);
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::OutputLimit => limits.output_bytes = limit,
                StopReason::NodeLimit => limits.nodes = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("enumerated resource"),
            }
            let mut bounded = Budget::new(limits);
            let result = digest(b"frontier", &value, &mut bounded);
            if below {
                assert_eq!(result, Err(WireError::Stopped(reason)));
                assert_eq!(bounded.poll(), Err(reason));
                assert_eq!(
                    digest(b"frontier", &value, &mut bounded),
                    Err(WireError::Stopped(reason))
                );
            } else {
                assert_eq!(result?, expected);
            }
        }
    }
    assert_eq!(encode(&value, &mut budget())?, bytes);
    Ok(())
}

#[test]
fn wide_input_stops_before_growing_an_unfunded_frontier() {
    let value = NdfValue::List(vec![NdfValue::Unit; 10_000]);
    let mut limits = budget().limits();
    limits.work = 16;
    let mut b = Budget::new(limits);
    assert_eq!(
        digest(b"", &value, &mut b),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    assert!(
        b.usage().allocation_units <= 16 * core::mem::size_of::<(Option<&NdfValue>, u64)>() as u64
    );
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
}
