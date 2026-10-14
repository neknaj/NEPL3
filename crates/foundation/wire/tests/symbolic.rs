use nepl3_core::{
    budget::*,
    schema::*,
    source::*,
    value::*,
    value_codec::{FoundationCodecError, FoundationValueCodec},
    view::*,
};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 100_000,
        depth: 64,
        source_bytes: 0,
        output_bytes: 0,
        diagnostics: 0,
        events: 0,
    })
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut b = budget();
    let d = foundation::descriptor(&mut b).map_err(err)?;
    let mut r = SchemaRegistry::default();
    r.register(d.reference(&mut b).map_err(err)?, d, &mut b)
        .map_err(err)?;
    r.finalize(&mut b).map_err(err)?;
    Ok(r)
}
fn err(v: impl core::fmt::Debug) -> String {
    format!("{v:?}")
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Symbol {
    Kind(KindRef),
    Operation(OperationRef),
    Class(PresentationClass),
}
impl Symbol {
    fn name(&self) -> &'static str {
        match self {
            Self::Kind(_) => "KindRef",
            Self::Operation(_) => "OperationRef",
            Self::Class(_) => "PresentationClass",
        }
    }
    fn encode(
        &self,
        c: &mut impl FoundationValueCodec<Error = WireError>,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        match self {
            Self::Kind(v) => c.encode_kind_ref(v, b),
            Self::Operation(v) => c.encode_operation_ref(v, b),
            Self::Class(v) => c.encode_presentation_class(v, b),
        }
    }
    fn decode(
        &self,
        v: &NdfValue,
        c: &mut impl FoundationValueCodec<Error = WireError>,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        match self {
            Self::Kind(_) => c.decode_kind_ref(v, b).map(Self::Kind),
            Self::Operation(_) => c.decode_operation_ref(v, b).map(Self::Operation),
            Self::Class(_) => c.decode_presentation_class(v, b).map(Self::Class),
        }
    }
}
fn target() -> SchemaRef {
    SchemaRef {
        package: "unregistered.世界".into(),
        revision: u64::MAX,
        digest: Digest::of(b"symbolic target"),
    }
}
fn symbols() -> Vec<Symbol> {
    let mut values = vec![
        Symbol::Kind(KindRef {
            schema: target(),
            local_kind: u64::MAX,
        }),
        Symbol::Operation(OperationRef {
            schema: target(),
            name: String::new(),
        }),
    ];
    for fallback in [
        FallbackRole::Content,
        FallbackRole::Marker,
        FallbackRole::Delimiter,
        FallbackRole::Name,
        FallbackRole::Quantity,
        FallbackRole::Annotation,
    ] {
        values.push(Symbol::Class(PresentationClass {
            schema: target(),
            name: "unknown class".into(),
            fallback,
        }));
    }
    values
}
fn rec(s: &SchemaRef, name: &str, fields: Vec<NdfValue>) -> NdfValue {
    NdfValue::Record(Record {
        schema: s.clone(),
        kind: name.into(),
        fields,
    })
}
fn expected(v: &Symbol, s: &SchemaRef) -> NdfValue {
    let schema = target();
    let mut f = vec![rec(
        s,
        "SchemaRef",
        vec![
            NdfValue::Text(schema.package),
            NdfValue::U64(schema.revision),
            NdfValue::Bytes(schema.digest.0.to_vec()),
        ],
    )];
    match v {
        Symbol::Kind(k) => f.push(NdfValue::U64(k.local_kind)),
        Symbol::Operation(o) => f.push(NdfValue::Text(o.name.clone())),
        Symbol::Class(c) => {
            f.push(NdfValue::Text(c.name.clone()));
            f.push(NdfValue::Variant(Variant {
                schema: s.clone(),
                type_name: "FallbackRole".into(),
                variant: match c.fallback {
                    FallbackRole::Content => "Content",
                    FallbackRole::Marker => "Marker",
                    FallbackRole::Delimiter => "Delimiter",
                    FallbackRole::Name => "Name",
                    FallbackRole::Quantity => "Quantity",
                    FallbackRole::Annotation => "Annotation",
                }
                .into(),
                fields: Vec::new(),
            }));
        }
    }
    rec(s, v.name(), f)
}
#[test]
fn symbolic_values_preserve_layout_and_do_not_resolve_or_admit_sources() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    for v in symbols() {
        let mut b = budget();
        let value = v.encode(&mut c, &mut b).map_err(err)?;
        assert_eq!(value, expected(&v, c.foundation_schema()));
        let mut transport = Budget::new(Limits {
            output_bytes: 1_000_000,
            ..budget().limits()
        });
        let bytes = nepl3_wire::encode(&value, &mut transport).map_err(err)?;
        let cold = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        assert_eq!(v.decode(&cold, &mut c, &mut b).map_err(err)?, v);
        assert_eq!((b.usage().source_bytes, b.usage().output_bytes), (0, 0));
        assert!(r.selected("unregistered.世界", u64::MAX).is_none());
    }
    Ok(())
}
#[test]
fn symbolic_values_reject_nominal_and_nested_malformed_values() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    for v in symbols() {
        let original = v.encode(&mut c, &mut budget()).map_err(err)?;
        for mutation in 0..7 {
            let mut value = original.clone();
            let NdfValue::Record(root) = &mut value else {
                return Err("record".into());
            };
            match mutation {
                0 => root.schema.digest = Digest::of(b"forged foundation"),
                1 => root.kind = "Wrong".into(),
                2 => {
                    root.fields.pop();
                }
                3 => root.fields[1] = NdfValue::Unit,
                _ => {
                    let NdfValue::Record(schema) = &mut root.fields[0] else {
                        return Err("schema".into());
                    };
                    match mutation {
                        4 => schema.schema.digest = Digest::of(b"forged nested"),
                        5 => schema.fields[2] = NdfValue::Bytes(vec![0; 31]),
                        _ => schema.fields[1] = NdfValue::Text("wrong".into()),
                    }
                }
            }
            assert!(
                v.decode(&value, &mut c, &mut budget()).is_err(),
                "{mutation}"
            );
        }
        if matches!(v, Symbol::Class(_)) {
            for mutation in 0..3 {
                let mut value = original.clone();
                let NdfValue::Record(root) = &mut value else {
                    return Err("record".into());
                };
                let NdfValue::Variant(f) = &mut root.fields[2] else {
                    return Err("fallback".into());
                };
                match mutation {
                    0 => f.variant = "Unknown".into(),
                    1 => f.fields.push(NdfValue::Unit),
                    _ => f.schema.digest = Digest::of(b"forged fallback"),
                };
                assert!(v.decode(&value, &mut c, &mut budget()).is_err());
            }
        }
    }
    Ok(())
}
#[test]
fn symbolic_values_keep_sticky_limits_for_both_directions() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    for v in symbols() {
        let value = v.encode(&mut c, &mut budget()).map_err(err)?;
        for decode in [false, true] {
            let mut measured = budget();
            if decode {
                v.decode(&value, &mut c, &mut measured).map_err(err)?;
            } else {
                v.encode(&mut c, &mut measured).map_err(err)?;
            }
            let u = measured.usage();
            for (index, used) in [u.work, u.allocation_units, u.nodes, u.depth]
                .into_iter()
                .enumerate()
            {
                assert!(used > 0);
                for (limit, success) in [(used, true), (used - 1, false), (0, false)] {
                    let mut limits = budget().limits();
                    match index {
                        0 => limits.work = limit,
                        1 => limits.allocation_units = limit,
                        2 => limits.nodes = limit,
                        _ => limits.depth = limit,
                    };
                    let mut b = Budget::new(limits);
                    let result = if decode {
                        v.decode(&value, &mut c, &mut b).map(|_| ())
                    } else {
                        v.encode(&mut c, &mut b).map(|_| ())
                    };
                    if success {
                        result.map_err(err)?;
                    } else {
                        let e = result.err().ok_or("limit ignored")?;
                        assert!(e.stop_reason().is_some());
                        assert!(b.poll().is_err());
                    }
                }
            }
            let mut b = budget();
            b.cancel();
            let result = if decode {
                v.decode(&value, &mut c, &mut b).map(|_| ())
            } else {
                v.encode(&mut c, &mut b).map(|_| ())
            };
            assert_eq!(
                result.err().ok_or("cancel ignored")?.stop_reason(),
                Some(StopReason::Cancelled)
            );
        }
    }
    Ok(())
}

#[test]
fn symbolic_copy_work_scales_with_package_and_name_bytes() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
    for short in symbols() {
        let mut long = short.clone();
        let padding = "界".repeat(100);
        let added = match &mut long {
            Symbol::Kind(v) => {
                v.schema.package.push_str(&padding);
                padding.len()
            }
            Symbol::Operation(v) => {
                v.schema.package.push_str(&padding);
                v.name.push_str(&padding);
                2 * padding.len()
            }
            Symbol::Class(v) => {
                v.schema.package.push_str(&padding);
                v.name.push_str(&padding);
                2 * padding.len()
            }
        };
        let mut a = budget();
        let mut b = budget();
        let small = short.encode(&mut codec, &mut a).map_err(err)?;
        let large = long.encode(&mut codec, &mut b).map_err(err)?;
        assert!(b.usage().work >= a.usage().work + added as u64);
        let mut a = budget();
        let mut b = budget();
        short.decode(&small, &mut codec, &mut a).map_err(err)?;
        long.decode(&large, &mut codec, &mut b).map_err(err)?;
        assert!(b.usage().work >= a.usage().work + added as u64);
    }
    Ok(())
}
