use super::{budget, compile_document, load};
use nepl3_core::{budget::*, schema::*, source::*, value::*};
use nepl3_grammar_core::{
    model::{Document, ModelError},
    portable::{self, PortableError},
};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        nepl3_grammar_core::schema::descriptor(&mut budget()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
            .map_err(err)?;
    }
    r.finalize(&mut budget()).map_err(err)?;
    Ok(r)
}
fn encode(
    d: &Document,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<WireError>> {
    let sources = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &sources, &mut a).map_err(PortableError::Foundation)?;
    portable::to_value(d, r, &mut c, b)
}
fn decode(
    v: &NdfValue,
    r: &SchemaRegistry,
    sources: &SourceStore,
    b: &mut Budget,
) -> Result<Document, PortableError<WireError>> {
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, sources, &mut a).map_err(PortableError::Foundation)?;
    portable::from_value(v, r, &mut c, b)
}
fn literal<'a>(v: &'a mut NdfValue, name: &str) -> Option<&'a mut Record> {
    match v {
        NdfValue::Record(r) => {
            if r.kind == name {
                Some(r)
            } else {
                r.fields.iter_mut().find_map(|v| literal(v, name))
            }
        }
        NdfValue::Variant(r) => r.fields.iter_mut().find_map(|v| literal(v, name)),
        NdfValue::List(v) => v.iter_mut().find_map(|v| literal(v, name)),
        NdfValue::Some(v) => literal(v, name),
        _ => None,
    }
}
#[test]
fn cold_cbor_roundtrip_retains_nominal_literals_sources_and_compilation() -> Result<(), String> {
    let r = registry()?;
    for path in [
        "examples/grammar/binding.neplg",
        "examples/grammar/angle-tag.neplg",
        "languages/grammar/syntax.neplg",
    ] {
        let d = load(path)?;
        let mut value = encode(&d, &r, &mut budget()).map_err(err)?;
        for name in ["NameLiteral", "TextLiteral", "NatLiteral"] {
            assert!(literal(&mut value, name).is_some(), "{path}: {name}");
        }
        let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
        let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        let rebuilt = decode(&received, &r, &SourceStore::default(), &mut budget()).map_err(err)?;
        assert_eq!(d, rebuilt);
        let before = compile_document(&d)?;
        let after = compile_document(&rebuilt)?;
        match (before, after) {
            (Ok(a), Ok(b)) => assert_eq!(a.package, b.package),
            (Err(a), Err(b)) => assert_eq!(a, b),
            _ => return Err("roundtrip changed compilation".into()),
        }
    }
    Ok(())
}
#[test]
fn nominal_swap_forged_schema_and_negative_nat_are_rejected() -> Result<(), String> {
    let r = registry()?;
    let d = load("examples/grammar/binding.neplg")?;
    for (from, to) in [
        ("NameLiteral", "TextLiteral"),
        ("TextLiteral", "NameLiteral"),
    ] {
        let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
        literal(&mut v, from).ok_or("literal")?.kind = to.into();
        assert!(matches!(
            decode(&v, &r, &SourceStore::default(), &mut budget()),
            Err(PortableError::Schema(_))
        ));
    }
    let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
    literal(&mut v, "NameLiteral")
        .ok_or("literal")?
        .schema
        .revision += 1;
    assert!(decode(&v, &r, &SourceStore::default(), &mut budget()).is_err());
    let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
    literal(&mut v, "NatLiteral").ok_or("literal")?.fields[0] =
        NdfValue::Integer(Integer::from(-1i64));
    assert!(matches!(
        decode(&v, &r, &SourceStore::default(), &mut budget()),
        Err(PortableError::Model(ModelError::Natural))
    ));
    Ok(())
}
#[test]
fn declared_closure_cannot_borrow_ambient_sources_and_invalid_root_is_rejected()
-> Result<(), String> {
    let r = registry()?;
    let mut d = load("examples/grammar/binding.neplg")?;
    let mut ambient = SourceStore::default();
    for s in &d.sources {
        ambient
            .insert_with_budget(
                s.clone_with_budget(&mut budget()).map_err(err)?,
                &mut budget(),
            )
            .map_err(err)?;
    }
    let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
    let NdfValue::Record(root) = &mut v else {
        return Err("document".into());
    };
    root.fields[0] = NdfValue::List(vec![]);
    assert!(decode(&v, &r, &ambient, &mut budget()).is_err());
    d.root.0 = u64::MAX;
    assert!(matches!(
        encode(&d, &r, &mut budget()),
        Err(PortableError::Model(ModelError::MissingNode))
    ));
    Ok(())
}
#[test]
fn exact_limits_and_cancellation_preserve_stop_reason() -> Result<(), String> {
    let r = registry()?;
    let d = load("examples/grammar/binding.neplg")?;
    let mut measured = budget();
    let v = encode(&d, &r, &mut measured).map_err(err)?;
    let encoded = measured.usage();
    let mut measured = budget();
    let _ = decode(&v, &r, &SourceStore::default(), &mut measured).map_err(err)?;
    let decoded = measured.usage();
    for (decoding, usage) in [(false, encoded), (true, decoded)] {
        for (resource, amount, stop) in [
            (0, usage.work, StopReason::WorkLimit),
            (1, usage.allocation_units, StopReason::AllocationLimit),
            (2, usage.source_bytes, StopReason::SourceLimit),
            (3, usage.nodes, StopReason::NodeLimit),
            (4, usage.depth, StopReason::DepthLimit),
        ] {
            for short in [false, true] {
                let mut limits = Limits {
                    source_bytes: 10_000_000,
                    work: 100_000_000,
                    depth: 1000,
                    nodes: 1_000_000,
                    allocation_units: 500_000_000,
                    output_bytes: 10_000_000,
                    diagnostics: 1000,
                    events: 1000,
                };
                let ceiling = amount - u64::from(short);
                match resource {
                    0 => limits.work = ceiling,
                    1 => limits.allocation_units = ceiling,
                    2 => limits.source_bytes = ceiling,
                    3 => limits.nodes = ceiling,
                    _ => limits.depth = ceiling,
                }
                let mut b = Budget::new(limits);
                let result = if decoding {
                    decode(&v, &r, &SourceStore::default(), &mut b).map(|_| ())
                } else {
                    encode(&d, &r, &mut b).map(|_| ())
                };
                if short {
                    assert_eq!(result, Err(PortableError::Stopped(stop)));
                } else {
                    result.map_err(err)?;
                }
            }
        }
    }
    let mut b = budget();
    b.cancel();
    assert!(matches!(
        encode(&d, &r, &mut b),
        Err(PortableError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn shared_admission_counts_sources_once_and_multisource_order_is_canonical() -> Result<(), String> {
    let r = registry()?;
    let mut d = load("examples/grammar/binding.neplg")?;
    d.sources.push(
        SourceSnapshot::new(
            SourceId("A-unused".into()),
            0,
            "memory:unused".into(),
            b"extra".to_vec(),
            &mut budget(),
        )
        .map_err(err)?,
    );
    let expected_bytes = d.sources.iter().map(|s| s.text().len() as u64).sum::<u64>();
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let mut b = budget();
    let v = portable::to_value(&d, &r, &mut c, &mut b).map_err(err)?;
    let mut rebuilt = portable::from_value(&v, &r, &mut c, &mut b).map_err(err)?;
    assert_eq!(b.usage().source_bytes, expected_bytes);
    assert_eq!(rebuilt.sources[0].identity().source.0, "A-unused");
    d.sources
        .sort_by(|a, b| a.identity().source.0.cmp(&b.identity().source.0));
    rebuilt
        .sources
        .sort_by(|a, b| a.identity().source.0.cmp(&b.identity().source.0));
    assert_eq!(d, rebuilt);
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        decode(&v, &r, &empty, &mut cancelled),
        Err(PortableError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
#[test]
fn receiver_rejects_forged_source_span_reference_category_and_unreachable_node()
-> Result<(), String> {
    let r = registry()?;
    let d = load("examples/grammar/binding.neplg")?;
    for mutation in 0..6 {
        let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
        match mutation {
            0 => {
                let content = literal(&mut v, "SourceContent").ok_or("source")?;
                content.fields[2] = NdfValue::Text("forged source bytes".into());
            }
            1 => {
                literal(&mut v, "Span").ok_or("span")?.fields[2] = NdfValue::U64(u64::MAX);
            }
            2 => {
                let NdfValue::Record(doc) = &mut v else {
                    return Err("document".into());
                };
                let NdfValue::Record(root) = &mut doc.fields[2] else {
                    return Err("root".into());
                };
                root.fields[0] = NdfValue::U64(u64::MAX);
            }
            3 => {
                let NdfValue::Record(doc) = &mut v else {
                    return Err("document".into());
                };
                let NdfValue::List(nodes) = &mut doc.fields[1] else {
                    return Err("nodes".into());
                };
                let NdfValue::Record(root_node) = &mut nodes[d.root.0 as usize] else {
                    return Err("root node".into());
                };
                let NdfValue::Variant(language) = &mut root_node.fields[0] else {
                    return Err("language".into());
                };
                assert_eq!(language.variant, "Language");
                let NdfValue::Record(list) = &mut language.fields[3] else {
                    return Err("declarations".into());
                };
                let NdfValue::List(items) = &mut list.fields[0] else {
                    return Err("items".into());
                };
                let NdfValue::Record(first) = items.first_mut().ok_or("first")? else {
                    return Err("nodeid".into());
                };
                let nepl3_grammar_core::model::NodeKind::Language { declarations, .. } =
                    &d.node(d.root).map_err(err)?.kind
                else {
                    return Err("language".into());
                };
                let wrong = d
                    .nodes
                    .iter()
                    .enumerate()
                    .find(|(_, node)| {
                        declarations.span.contains(&node.span)
                            && node.kind.category()
                                != nepl3_grammar_core::model::Category::Declaration
                    })
                    .ok_or("contained wrong category")?
                    .0;
                first.fields[0] = NdfValue::U64(wrong as u64);
            }
            4 => {
                let NdfValue::Record(doc) = &mut v else {
                    return Err("document".into());
                };
                let NdfValue::List(nodes) = &mut doc.fields[1] else {
                    return Err("nodes".into());
                };
                nodes.push(
                    nodes
                        .first()
                        .ok_or("node")?
                        .clone_with_budget(&mut budget())
                        .map_err(err)?,
                );
            }
            _ => {
                let NdfValue::Record(doc) = &mut v else {
                    return Err("document".into());
                };
                let NdfValue::List(sources) = &mut doc.fields[0] else {
                    return Err("sources".into());
                };
                sources.push(
                    sources
                        .first()
                        .ok_or("source")?
                        .clone_with_budget(&mut budget())
                        .map_err(err)?,
                );
            }
        }
        let result = decode(&v, &r, &SourceStore::default(), &mut budget());
        match mutation {
            2 => assert!(matches!(
                result,
                Err(PortableError::Model(ModelError::MissingNode))
            )),
            3 => assert!(
                matches!(result, Err(PortableError::Model(ModelError::Category))),
                "{result:?}"
            ),
            4 => assert!(matches!(
                result,
                Err(PortableError::Model(ModelError::Unreachable))
            )),
            _ => assert!(result.is_err(), "mutation {mutation}"),
        }
    }
    Ok(())
}
#[test]
fn receiver_rejects_same_category_cycle() -> Result<(), String> {
    let r = registry()?;
    let d = load("examples/grammar/binding.neplg")?;
    let mut v = encode(&d, &r, &mut budget()).map_err(err)?;
    let NdfValue::Record(doc) = &mut v else {
        return Err("document".into());
    };
    let NdfValue::List(nodes) = &mut doc.fields[1] else {
        return Err("nodes".into());
    };
    let mut found = false;
    for (index, node) in nodes.iter_mut().enumerate() {
        let NdfValue::Record(node) = node else {
            continue;
        };
        let NdfValue::Variant(kind) = &mut node.fields[0] else {
            continue;
        };
        if matches!(
            kind.variant.as_str(),
            "Many" | "Some" | "Optional" | "Look" | "Not" | "Commit" | "Discard"
        ) {
            let NdfValue::Record(body) = &mut kind.fields[0] else {
                return Err("body".into());
            };
            body.fields[0] = NdfValue::U64(index as u64);
            found = true;
            break;
        }
    }
    assert!(found, "fixture needs recursive reader expression");
    assert!(matches!(
        decode(&v, &r, &SourceStore::default(), &mut budget()),
        Err(PortableError::Model(ModelError::Cycle))
    ));
    Ok(())
}
