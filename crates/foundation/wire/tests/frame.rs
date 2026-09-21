use nepl3_core::{budget::*, schema::*, value::NdfValue};
use nepl3_wire::{WireError, frame};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1024,
        work: 100_000,
        depth: 100,
        nodes: 1000,
        allocation_units: 100_000,
        output_bytes: 1024,
        diagnostics: 10,
        events: 10,
    })
}
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    registry.finalize(&mut budget()).map_err(error)?;
    Ok(registry)
}

#[test]
fn frames_preserve_boundaries_and_reject_incomplete_final_input() -> Result<(), String> {
    let registry = registry()?;
    let ty = TypeDescriptor::U64;
    let first =
        frame::encode_checked(&NdfValue::U64(7), &ty, &registry, &mut budget()).map_err(error)?;
    // The wire profile fixes its own value encoding. The transport length is
    // independently checked against that exact byte sequence, including endianness.
    let payload = nepl3_wire::encode(&NdfValue::U64(7), &mut budget()).map_err(error)?;
    assert_eq!(&first[..8], &(payload.len() as u64).to_be_bytes());
    assert_eq!(&first[8..], payload);
    for length in 0..first.len() {
        assert!(
            frame::decode_checked(&first[..length], false, &ty, &registry, &mut budget())
                .map_err(error)?
                .is_none()
        );
        assert_eq!(
            frame::decode_checked(&first[..length], true, &ty, &registry, &mut budget()),
            Err(WireError::UnexpectedEnd)
        );
    }
    let mut pair = first.clone();
    pair.extend_from_slice(&first);
    let mut b = budget();
    let (value, rest) = frame::decode_checked(&pair, true, &ty, &registry, &mut b)
        .map_err(error)?
        .ok_or("frame")?;
    assert_eq!(value.value(), &NdfValue::U64(7));
    assert_eq!(rest, first);
    assert_eq!(b.usage().source_bytes, first.len() as u64);
    assert!(
        frame::decode_checked(
            &first,
            true,
            &TypeDescriptor::Bool,
            &registry,
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn frame_limits_are_checked_before_payload_arrives() -> Result<(), String> {
    let mut b = budget();
    assert_eq!(
        frame::payload_length(&1017u64.to_be_bytes(), &mut b),
        Err(WireError::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::SourceLimit));
    assert_eq!(
        frame::payload_length(&u64::MAX.to_be_bytes(), &mut budget()),
        Err(WireError::InvalidLength)
    );
    let mut b = budget();
    b.cancel();
    assert_eq!(
        frame::payload_length(&1u64.to_be_bytes(), &mut b),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    let registry = registry()?;
    assert_eq!(
        frame::decode_checked(
            &[0; 8],
            true,
            &TypeDescriptor::U64,
            &registry,
            &mut budget()
        ),
        Err(WireError::UnexpectedEnd)
    );
    Ok(())
}

#[test]
fn frame_byte_limits_include_headers_and_accumulate_across_messages() -> Result<(), String> {
    let registry = registry()?;
    let ty = TypeDescriptor::U64;
    let value = NdfValue::U64(7);
    let encoded = frame::encode_checked(&value, &ty, &registry, &mut budget()).map_err(error)?;
    let length = encoded.len() as u64;
    let mut limits = budget().limits();
    limits.source_bytes = length * 2;
    let mut receiver = Budget::new(limits);
    for count in 1..=2 {
        let (decoded, remaining) =
            frame::decode_checked(&encoded, true, &ty, &registry, &mut receiver)
                .map_err(error)?
                .ok_or("complete frame")?;
        assert_eq!(decoded.value(), &value);
        assert!(remaining.is_empty());
        assert_eq!(receiver.usage().source_bytes, count * length);
    }
    assert_eq!(
        frame::decode_checked(&encoded[..8], false, &ty, &registry, &mut receiver),
        Err(WireError::Stopped(StopReason::SourceLimit))
    );
    assert_eq!(receiver.poll(), Err(StopReason::SourceLimit));
    for available in [length - 1, length] {
        limits.output_bytes = available;
        let mut sender = Budget::new(limits);
        let output = frame::encode_checked(&value, &ty, &registry, &mut sender);
        if available == length {
            assert_eq!(output.map_err(error)?, encoded);
            assert_eq!(sender.usage().output_bytes, length);
        } else {
            assert_eq!(output, Err(WireError::Stopped(StopReason::OutputLimit)));
            assert_eq!(sender.poll(), Err(StopReason::OutputLimit));
        }
    }
    // A second NDF value inside the declared payload is not a second frame.
    let mut malformed = encoded;
    malformed[..8].copy_from_slice(&(length - 8 + 1).to_be_bytes());
    malformed.push(0);
    assert_eq!(
        frame::decode_checked(&malformed, true, &ty, &registry, &mut budget()),
        Err(WireError::TrailingData)
    );
    Ok(())
}
