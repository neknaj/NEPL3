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
        SourceId("input".into()),
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
    s.span(1, 3) == Err(SourceError::ScalarBoundary)
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
