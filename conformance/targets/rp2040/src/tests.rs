//! Production API checks. Expectations are literal protocol/Unicode facts, not
//! snapshots of this implementation's output. More target cases remain to add.
use alloc::vec;
use nepl3_core::{budget::*, source::*, value::NdfValue};
pub const CASES: &[(&str, fn() -> bool)] = &[
    ("core.source", source),
    ("core.budget", stopped),
    ("wire.cbor", cbor),
    ("wire.rejection", rejection),
];
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 4096,
        work: 1_000_000,
        depth: 64,
        nodes: 4096,
        allocation_units: 65536,
        output_bytes: 4096,
        diagnostics: 8,
        events: 8,
    })
}
fn source() -> bool {
    let Ok(s) = SourceSnapshot::new(
        SourceId("入力🙂".into()),
        0,
        "memory:input".into(),
        "日🙂\r\nX".as_bytes().to_vec(),
        &mut budget(),
    ) else {
        return false;
    };
    let Ok(index) = LineIndex::new(&s, &mut budget()) else {
        return false;
    };
    snapshot_lookup_cache(&s)
        && span_copy_cost(&s)
        && s.span(1, 3) == Err(SourceError::ScalarBoundary)
        && index.line_count() == 2
        && index.offset(
            &s,
            Position {
                line: 0,
                character: 3,
            },
            PositionEncoding::Utf16,
        ) == Ok(7)
        && index.offset(
            &s,
            Position {
                line: 1,
                character: 0,
            },
            PositionEncoding::Utf16,
        ) == Ok(9)
}
// The derived cache has the same fixed-width encoding on this non-atomic
// target. Equal hashes never grant structural equality or source admission.
fn snapshot_lookup_cache(source: &SourceSnapshot) -> bool {
    if source.identity_lookup_hash() != 0xc6e5_526e_de5d_d2b8 {
        return false;
    }
    let Ok(independent) = SourceSnapshot::new(
        source.identity().source.clone(),
        source.identity().revision,
        "memory:other".into(),
        source.text().as_bytes().to_vec(),
        &mut budget(),
    ) else {
        return false;
    };
    if independent.identity() != source.identity()
        || independent.identity_lookup_hash() != source.identity_lookup_hash()
        || &independent == source
    {
        return false;
    }
    let bytes = if cfg!(target_has_atomic = "ptr") {
        0
    } else {
        (source.identity().source.0.len() + source.uri().len() + source.text().len()) as u64
    };
    let limits = Limits {
        work: bytes + 1,
        allocation_units: core::mem::size_of::<SourceSnapshot>() as u64 + bytes,
        ..budget().limits()
    };
    let mut b = Budget::new(limits);
    let Ok(cloned) = source.clone_with_budget(&mut b) else {
        return false;
    };
    cloned == *source
        && cloned.identity_lookup_hash() == source.identity_lookup_hash()
        && b.usage().work == limits.work
        && b.usage().allocation_units == limits.allocation_units
}
// The RP2040 target has no pointer atomics: this executes the owned-name
// branch, not merely a host build of the shared-identity branch.
fn span_copy_cost(source: &SourceSnapshot) -> bool {
    let Ok(span) = source.span(0, 3) else {
        return false;
    };
    let name_bytes = source.identity().source.0.len() as u64;
    let expected_work = name_bytes + 1;
    let expected_allocation = core::mem::size_of::<Span>() as u64 + name_bytes;
    let mut measured = budget();
    let Ok(copied) = span.clone_with_budget(&mut measured) else {
        return false;
    };
    if measured.usage().work != expected_work
        || measured.usage().allocation_units != expected_allocation
        || source.slice(&copied) != Ok("日")
    {
        return false;
    }
    let mut charged = budget();
    if span.charge_shared_clone(&mut charged).is_err() || charged.usage() != measured.usage() {
        return false;
    }
    let exact = Limits {
        work: expected_work,
        allocation_units: expected_allocation,
        ..budget().limits()
    };
    if span.clone_with_budget(&mut Budget::new(exact)) != Ok(span.clone()) {
        return false;
    }
    for (limits, reason) in [
        (
            Limits {
                work: expected_work - 1,
                ..exact
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: expected_allocation - 1,
                ..exact
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        if span.clone_with_budget(&mut b) != Err(reason) || b.poll() != Err(reason) {
            return false;
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    span.clone_with_budget(&mut cancelled) == Err(StopReason::Cancelled)
        && cancelled.poll() == Err(StopReason::Cancelled)
}
fn stopped() -> bool {
    let mut limits = budget().limits();
    limits.work = 2;
    let mut b = Budget::new(limits);
    b.charge(Resource::Work, 2).is_ok()
        && b.charge(Resource::Work, 1) == Err(StopReason::WorkLimit)
        && b.charge(Resource::Work, 0) == Err(StopReason::WorkLimit)
}
fn cbor() -> bool {
    let value = NdfValue::List(vec![NdfValue::Bool(true), NdfValue::Text("日".into())]);
    let Ok(bytes) = nepl3_wire::encode(&value, &mut budget()) else {
        return false;
    };
    nepl3_wire::decode(&bytes, &mut budget()) == Ok(value)
}
fn rejection() -> bool {
    nepl3_wire::decode(&[], &mut budget()) == Err(nepl3_wire::WireError::UnexpectedEnd)
}
