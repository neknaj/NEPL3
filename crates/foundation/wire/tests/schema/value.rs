use super::{budget, error, registry, sample};
use nepl3_core::{
    budget::*,
    schema::*,
    source::{Digest, SourceAdmission, SourceStore},
    value::*,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn encode(d: &SchemaDescriptor, r: &SchemaRegistry, b: &mut Budget) -> Result<NdfValue, WireError> {
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    FoundationCodec::new(r, &sources, &mut admission)?.encode_schema_descriptor(d, b)
}
fn decode(
    v: &NdfValue,
    id: &SchemaRef,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<SchemaDescriptor, WireError> {
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    FoundationCodec::new(r, &sources, &mut admission)?.decode_schema_descriptor(v, id, b)
}
#[test]
fn trait_values_preserve_descriptors_and_share_canonical_byte_boundary() -> Result<(), String> {
    let r = registry()?;
    for d in [
        sample(),
        foundation::descriptor(&mut budget()).map_err(error)?,
    ] {
        let id = d.reference(&mut budget()).map_err(error)?;
        let mut b = Budget::new(Limits {
            output_bytes: 0,
            ..budget().limits()
        });
        let v = encode(&d, &r, &mut b).map_err(error)?;
        let bytes = nepl3_wire::encode(&v, &mut budget()).map_err(error)?;
        assert_eq!(
            bytes,
            nepl3_wire::schema::encode(&d, &r, &mut budget()).map_err(error)?
        );
        let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
        let rebuilt = decode(&received, &id, &r, &mut b).map_err(error)?;
        assert_eq!(rebuilt, d);
        assert_eq!(b.usage().source_bytes, 0);
        assert_eq!(b.usage().output_bytes, 0);
        if d.package != "nepl3.foundation" {
            assert!(r.selected(&d.package, d.revision).is_none());
        }
    }
    // Symbolic dependencies remain data even in scoped foundation codecs.
    let mut d = sample();
    d.operations[0].input = TypeDescriptor::Named(TypeRef {
        package: "missing.dependency".into(),
        revision: 99,
        name: "Unknown".into(),
    });
    let id = d.reference(&mut budget()).map_err(error)?;
    let source = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &source, &mut a).map_err(error)?;
    let mut scoped = c.scoped(&source);
    let v = scoped
        .encode_schema_descriptor(&d, &mut budget())
        .map_err(error)?;
    assert_eq!(
        scoped
            .decode_schema_descriptor(&v, &id, &mut budget())
            .map_err(error)?,
        d
    );
    assert!(r.selected("missing.dependency", 99).is_none());
    Ok(())
}
#[test]
fn trait_rejects_full_identity_spoofing_and_duplicate_semantics() -> Result<(), String> {
    let r = registry()?;
    let d = sample();
    let id = d.reference(&mut budget()).map_err(error)?;
    let v = encode(&d, &r, &mut budget()).map_err(error)?;
    for bad in [
        SchemaRef {
            package: "wrong".into(),
            ..id.clone()
        },
        SchemaRef {
            revision: 8,
            ..id.clone()
        },
        SchemaRef {
            digest: Digest::of(b"wrong"),
            ..id.clone()
        },
    ] {
        assert_eq!(
            decode(&v, &bad, &r, &mut budget()),
            Err(WireError::Schema(SchemaError::IdentityMismatch))
        );
    }
    let mut forged = v.clone();
    let NdfValue::Record(root) = &mut forged else {
        return Err("root".into());
    };
    root.schema.digest = Digest::of(b"forged foundation");
    assert!(decode(&forged, &id, &r, &mut budget()).is_err());
    let mut duplicate = d.clone();
    duplicate.types.push(duplicate.types[0].clone());
    assert!(matches!(
        encode(&duplicate, &r, &mut budget()),
        Err(WireError::Schema(SchemaError::DuplicateName))
    ));
    let mut duplicate = v.clone();
    let NdfValue::Record(root) = &mut duplicate else {
        return Err("root".into());
    };
    let NdfValue::List(types) = &mut root.fields[2] else {
        return Err("types".into());
    };
    types.push(types[0].clone());
    assert_eq!(
        decode(&duplicate, &id, &r, &mut budget()),
        Err(WireError::Schema(SchemaError::DuplicateName))
    );
    assert!(encode(&d, &SchemaRegistry::default(), &mut budget()).is_err());
    let mut empty = d;
    empty.package.clear();
    assert!(encode(&empty, &r, &mut budget()).is_err());
    Ok(())
}
#[test]
fn trait_descriptor_depth_remains_128_without_transport_roundtrip() -> Result<(), String> {
    let r = registry()?;
    let mut d = sample();
    let mut ty = TypeDescriptor::U64;
    for _ in 0..128 {
        ty = TypeDescriptor::List(Box::new(ty));
    }
    d.operations[0].input = ty;
    let large = || {
        Budget::new(Limits {
            depth: 1024,
            ..budget().limits()
        })
    };
    let id = d.reference(&mut large()).map_err(error)?;
    let mut v = encode(&d, &r, &mut large()).map_err(error)?;
    assert_eq!(decode(&v, &id, &r, &mut large()).map_err(error)?, d);
    let NdfValue::Record(root) = &mut v else {
        return Err("root".into());
    };
    let NdfValue::List(operations) = &mut root.fields[3] else {
        return Err("operations".into());
    };
    let NdfValue::Record(operation) = &mut operations[0] else {
        return Err("operation".into());
    };
    let inner = core::mem::replace(&mut operation.fields[1], NdfValue::Unit);
    operation.fields[1] = NdfValue::Variant(Variant {
        schema: operation.schema.clone(),
        type_name: "TypeDescriptor".into(),
        variant: "List".into(),
        fields: vec![inner],
    });
    assert_eq!(
        decode(&v, &id, &r, &mut large()),
        Err(WireError::Schema(SchemaError::DescriptorDepth))
    );
    d.operations[0].input = TypeDescriptor::List(Box::new(d.operations[0].input.clone()));
    assert!(matches!(
        encode(&d, &r, &mut large()),
        Err(WireError::Schema(SchemaError::DescriptorDepth))
    ));
    Ok(())
}
#[test]
fn trait_value_limits_are_exact_and_stops_are_preserved() -> Result<(), String> {
    let r = registry()?;
    let d = sample();
    let id = d.reference(&mut budget()).map_err(error)?;
    let mut b = budget();
    let v = encode(&d, &r, &mut b).map_err(error)?;
    let encoded = b.usage();
    let mut b = budget();
    let _ = decode(&v, &id, &r, &mut b).map_err(error)?;
    let decoded = b.usage();
    for (decoding, usage) in [(false, encoded), (true, decoded)] {
        for (resource, amount, stop) in [
            (0, usage.work, StopReason::WorkLimit),
            (1, usage.allocation_units, StopReason::AllocationLimit),
            (2, usage.depth, StopReason::DepthLimit),
            (3, usage.nodes, StopReason::NodeLimit),
        ] {
            for short in [false, true] {
                let mut limits = budget().limits();
                let ceiling = amount - u64::from(short);
                match resource {
                    0 => limits.work = ceiling,
                    1 => limits.allocation_units = ceiling,
                    2 => limits.depth = ceiling,
                    _ => limits.nodes = ceiling,
                }
                let mut b = Budget::new(limits);
                let result = if decoding {
                    decode(&v, &id, &r, &mut b).map(|_| ())
                } else {
                    encode(&d, &r, &mut b).map(|_| ())
                };
                if short {
                    assert_eq!(result.err().and_then(|e| e.stop_reason()), Some(stop));
                    assert_eq!(b.poll(), Err(stop));
                } else {
                    result.map_err(error)?;
                }
            }
        }
        let mut b = budget();
        b.cancel();
        let result = if decoding {
            decode(&v, &id, &r, &mut b).map(|_| ())
        } else {
            encode(&d, &r, &mut b).map(|_| ())
        };
        assert_eq!(result, Err(WireError::Stopped(StopReason::Cancelled)));
    }
    Ok(())
}
