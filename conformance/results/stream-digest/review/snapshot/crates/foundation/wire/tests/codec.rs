use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{
        FieldDescriptor, NamedType, SchemaDescriptor, SchemaRegistry, TypeDescriptor, TypeRef,
        TypeShape,
    },
    source::{Digest, SourceAdmission, SourceStore},
    value::{Integer, NdfValue, Rational, Record, SchemaRef, Variant},
    value_codec::FoundationValueCodec,
};
use nepl3_wire::{WireError, decode, decode_checked, encode, encode_checked};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000_000,
        work: 100_000_000,
        depth: 1_000_000,
        nodes: 1_000_000,
        allocation_units: 256_000_000,
        output_bytes: 100_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn hex(text: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !text.len().is_multiple_of(2) {
        return Err("odd hex fixture".into());
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).map_err(Into::into))
        .collect()
}
fn schema() -> SchemaRef {
    SchemaRef {
        package: "p".into(),
        revision: 1,
        digest: Digest([0; 32]),
    }
}

#[test]
fn all_twelve_tags_match_specified_cbor_bytes() -> TestResult {
    // Expected bytes are written from RFC8949 major types + spec09's NDF tag table,
    // including bare SchemaRef tuple/field arrays rather than a second NDF wrapper.
    let header = format!("836170015820{}", "00".repeat(32));
    let vectors = vec![
        (NdfValue::Unit, "8100".to_owned()),
        (NdfValue::Bool(true), "8201f5".into()),
        (NdfValue::U64(24), "82021818".into()),
        (
            NdfValue::Integer(Integer::from(-256_i64)),
            "8303f5420100".into(),
        ),
        (
            NdfValue::Rational(
                Rational::from_canonical(Integer::from(-1_i64), &[2])
                    .map_err(|e| format!("{e:?}"))?,
            ),
            "83048303f541014102".into(),
        ),
        (NdfValue::Text("日".into()), "820563e697a5".into()),
        (NdfValue::Bytes(vec![0, 255]), "82064200ff".into()),
        (
            NdfValue::List(vec![NdfValue::Unit, NdfValue::None]),
            "82078281008108".into(),
        ),
        (NdfValue::None, "8108".into()),
        (NdfValue::Some(Box::new(NdfValue::Unit)), "82098100".into()),
        (
            NdfValue::Record(Record {
                schema: schema(),
                kind: "R".into(),
                fields: vec![NdfValue::U64(7)],
            }),
            format!("840a{header}615281820207"),
        ),
        (
            NdfValue::Variant(Variant {
                schema: schema(),
                type_name: "T".into(),
                variant: "V".into(),
                fields: vec![],
            }),
            format!("850b{header}6154615680"),
        ),
    ];
    let mut registry = SchemaRegistry::default();
    let descriptor =
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    registry
        .register(
            descriptor
                .reference(&mut budget())
                .map_err(|e| format!("{e:?}"))?,
            descriptor,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    for (value, expected) in vectors {
        let bytes = hex(&expected)?;
        assert_eq!(encode(&value, &mut budget()), Ok(bytes.clone()));
        assert_eq!(
            codec.canonical_value_digest(b"all-tags\0", &value, &mut budget()),
            Ok(Digest::domain(b"all-tags\0", &bytes))
        );
        assert_eq!(decode(&bytes, &mut budget()), Ok(value));
    }
    Ok(())
}

#[test]
fn integer_width_boundaries_use_shortest_unsigned_encoding() -> TestResult {
    for (value, suffix) in [
        (23, "17"),
        (24, "1818"),
        (255, "18ff"),
        (256, "190100"),
        (65_535, "19ffff"),
        (65_536, "1a00010000"),
        (u32::MAX.into(), "1affffffff"),
        (u32::MAX as u64 + 1, "1b0000000100000000"),
        (u64::MAX, "1bffffffffffffffff"),
    ] {
        let expected = hex(&format!("8202{suffix}"))?;
        assert_eq!(
            encode(&NdfValue::U64(value), &mut budget()),
            Ok(expected.clone())
        );
        assert_eq!(decode(&expected, &mut budget()), Ok(NdfValue::U64(value)));
    }
    Ok(())
}

#[test]
fn malformed_noncanonical_and_forbidden_cbor_values_are_rejected() -> TestResult {
    for invalid in [
        "f6",
        "a0",
        "c08100",
        "9f8100ff",
        "8201f6",
        "8202f90000", // null/map/tag/indefinite/float
        "981800",
        "810c",
        "82021800",
        "8202190018",
        "82055f40ff", // nonminimal lengths/int, unknown tag, indefinite text
        "8303f540",
        "8303f44100",
        "83048303f4410140",
        "83048303f441024104", // negativezero/leadingzero/denom0/nonreduced
        "83048303f4404102",
        "820561ff",
        "81008100",
        "82079bffffffffffffffff", // zero rational, UTF8, trailing, huge count
    ] {
        assert!(decode(&hex(invalid)?, &mut budget()).is_err(), "{invalid}");
    }
    Ok(())
}

#[test]
fn every_truncated_prefix_fails_and_complete_input_preserves_bytes() -> TestResult {
    let value = NdfValue::List(vec![
        NdfValue::Text("a\r\n𠮷\0".into()),
        NdfValue::Integer(Integer::from(u64::MAX)),
    ]);
    let bytes = encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    for end in 0..bytes.len() {
        assert!(
            decode(&bytes[..end], &mut budget()).is_err(),
            "prefix {end}"
        );
    }
    assert_eq!(decode(&bytes, &mut budget()), Ok(value));
    Ok(())
}

#[test]
fn limits_stop_with_typed_reasons_without_a_successful_partial_value() {
    let mut limits = budget().limits();
    limits.depth = 1;
    assert_eq!(
        decode(&[0x82, 9, 0x81, 0], &mut Budget::new(limits)),
        Err(WireError::Stopped(StopReason::DepthLimit))
    );
    limits = budget().limits();
    limits.output_bytes = 1;
    assert_eq!(
        encode(&NdfValue::Unit, &mut Budget::new(limits)),
        Err(WireError::Stopped(StopReason::OutputLimit))
    );
    limits = budget().limits();
    limits.work = 0;
    assert_eq!(
        decode(&[0x81, 0], &mut Budget::new(limits)),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    limits = budget().limits();
    limits.allocation_units = 0;
    assert_eq!(
        decode(&[0x82, 5, 0x61, b'a'], &mut Budget::new(limits)),
        Err(WireError::Stopped(StopReason::AllocationLimit))
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        decode(&[0x81, 0], &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
}

#[test]
fn checked_boundary_uses_real_registry_and_rejects_wrong_field_type() -> TestResult {
    let descriptor = SchemaDescriptor {
        package: "example".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Box".into(),
            constraints: vec![],
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::Text,
                }],
            },
        }],
        operations: vec![],
    };
    let reference = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let value = NdfValue::Record(Record {
        schema: reference.clone(),
        kind: "Box".into(),
        fields: vec![NdfValue::Text("contents".into())],
    });
    let expected = TypeDescriptor::Named(TypeRef {
        package: "example".into(),
        revision: 1,
        name: "Box".into(),
    });
    let mut registry = SchemaRegistry::default();
    registry
        .register(reference, descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let bytes = encode_checked(&value, &expected, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let checked = decode_checked(&bytes, &expected, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(checked.value(), &value);
    let mut bad = value;
    if let NdfValue::Record(record) = &mut bad {
        record.fields[0] = NdfValue::U64(1);
    }
    let bytes = encode(&bad, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(decode_checked(&bytes, &expected, &registry, &mut budget()).is_err());
    Ok(())
}

#[test]
fn deeply_nested_values_do_not_use_the_native_call_stack() -> TestResult {
    let mut bytes = Vec::new();
    for _ in 0..32_000 {
        bytes.extend_from_slice(&[0x82, 9]);
    }
    bytes.extend_from_slice(&[0x81, 0]);
    let value = decode(&bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))?,
        bytes
    );
    // Dropping an accepted deep value and error cleanup are also boundary work.
    drop(value);
    bytes.pop();
    assert!(decode(&bytes, &mut budget()).is_err());
    Ok(())
}
