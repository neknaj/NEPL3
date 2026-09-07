use nepl3_core::{budget::*, schema::*, source::*, value::*, value_codec::FoundationValueCodec};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1000,
        work: 10_000_000,
        depth: 1000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn symbolic_type_descriptors_preserve_all_variants_and_nested_named_data() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(error)?;
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(error)?;
    let variants = [
        (TypeDescriptor::Unit, "Unit"),
        (TypeDescriptor::Bool, "Bool"),
        (TypeDescriptor::U64, "U64"),
        (TypeDescriptor::Integer, "Integer"),
        (TypeDescriptor::Natural, "Natural"),
        (TypeDescriptor::Rational, "Rational"),
        (TypeDescriptor::Text, "Text"),
        (TypeDescriptor::Bytes, "Bytes"),
        (TypeDescriptor::Bytes32, "Bytes32"),
        (TypeDescriptor::NdfValue, "NdfValue"),
        (TypeDescriptor::NdfScalar, "NdfScalar"),
        (TypeDescriptor::TypedValue, "TypedValue"),
    ];
    for (ty, name) in variants {
        let value = codec
            .encode_type_descriptor(&ty, &mut budget())
            .map_err(error)?;
        assert_eq!(
            value,
            NdfValue::Variant(Variant {
                schema: schema.clone(),
                type_name: "TypeDescriptor".into(),
                variant: name.into(),
                fields: vec![]
            })
        );
    }
    // Unknown Named references remain symbolic data, useful when describing a
    // compiler failure. Encoding is not successful registry type resolution.
    let ty = TypeDescriptor::List(Box::new(TypeDescriptor::Option(Box::new(
        TypeDescriptor::Named(TypeRef {
            package: "unregistered".into(),
            revision: 9,
            name: "型".into(),
        }),
    ))));
    let value = codec
        .encode_type_descriptor(&ty, &mut budget())
        .map_err(error)?;
    let mut cursor = &value;
    for name in ["List", "Option", "Named"] {
        let NdfValue::Variant(v) = cursor else {
            return Err("variant".into());
        };
        assert_eq!(v.variant, name);
        assert_eq!(v.fields.len(), 1);
        cursor = &v.fields[0];
    }
    let NdfValue::Record(reference) = cursor else {
        return Err("TypeRef".into());
    };
    assert_eq!(reference.kind, "TypeRef");
    assert_eq!(
        reference.fields,
        vec![
            NdfValue::Text("unregistered".into()),
            NdfValue::U64(9),
            NdfValue::Text("型".into())
        ]
    );
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
    assert_eq!(
        nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?,
        value
    );
    for index in 0..4 {
        let mut limits = budget().limits();
        let reason = match index {
            0 => {
                limits.work = 0;
                StopReason::WorkLimit
            }
            1 => {
                limits.allocation_units = 0;
                StopReason::AllocationLimit
            }
            2 => {
                limits.depth = 0;
                StopReason::DepthLimit
            }
            _ => StopReason::Cancelled,
        };
        let mut stopped = Budget::new(limits);
        if index == 3 {
            stopped.cancel();
        }
        assert_eq!(
            codec.encode_type_descriptor(&ty, &mut stopped),
            Err(WireError::Stopped(reason))
        );
        assert_eq!(stopped.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn deep_symbolic_types_roundtrip_and_stop_without_recursive_cleanup() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(error)?;
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema, descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &empty, &mut admission).map_err(error)?;
    let mut ty = TypeDescriptor::Named(TypeRef {
        package: "unregistered".into(),
        revision: 9,
        name: "未登録型".into(),
    });
    for index in 0..2048 {
        ty = if index % 2 == 0 {
            TypeDescriptor::List(Box::new(ty))
        } else {
            TypeDescriptor::Option(Box::new(ty))
        };
    }
    let mut limits = budget().limits();
    limits.depth = 5000;
    let mut b = Budget::new(limits);
    let encoded = codec.encode_type_descriptor(&ty, &mut b).map_err(error)?;
    let bytes = nepl3_wire::encode(&encoded, &mut b).map_err(error)?;
    let decoded = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
    assert_eq!(encoded, decoded);
    // Inspect the constructor order independently of the encoder's algorithm.
    let mut cursor = &decoded;
    for index in (0..2048).rev() {
        let NdfValue::Variant(wrapper) = cursor else {
            return Err("wrapper".into());
        };
        assert_eq!(
            wrapper.variant,
            if index % 2 == 0 { "List" } else { "Option" }
        );
        assert_eq!(wrapper.fields.len(), 1);
        cursor = &wrapper.fields[0];
    }
    let NdfValue::Variant(named) = cursor else {
        return Err("named".into());
    };
    assert_eq!(named.variant, "Named");
    let NdfValue::Record(reference) = &named.fields[0] else {
        return Err("reference".into());
    };
    assert_eq!(reference.fields[2], NdfValue::Text("未登録型".into()));
    drop(decoded);
    drop(encoded);
    drop(ty);

    let mut too_deep = TypeDescriptor::Text;
    for _ in 0..100_000 {
        too_deep = TypeDescriptor::List(Box::new(too_deep));
    }
    let mut low = Budget::new(Limits {
        depth: 64,
        ..budget().limits()
    });
    assert_eq!(
        codec.encode_type_descriptor(&too_deep, &mut low),
        Err(WireError::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(low.poll(), Err(StopReason::DepthLimit));
    // Input and partially built encoder storage must also have bounded cleanup.
    drop(too_deep);
    Ok(())
}
