use super::*;
use nepl3_core::origin::{Mapping, MappingKind, OriginError};
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_reader::portable::{PortableError, read, transform};
use nepl3_wire::foundation::FoundationCodec;
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn mapped_view(schema: &SchemaRef, parent: Span, child: Span) -> ViewBundle {
    ViewBundle {
        elements: vec![
            ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span: parent,
                fields: vec![ViewField {
                    name: "mapped".into(),
                    children: vec![ViewRef(1)],
                }],
                roles: vec![],
                relations: vec![],
            },
            ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span: child,
                fields: vec![],
                roles: vec![],
                relations: vec![],
            },
        ],
        roots: vec![ViewRef(0)],
    }
}
#[test]
fn terminal_views_use_accepted_maps_and_preserve_reply_delta() -> Result<(), String> {
    for (kind, with_delta) in [
        ProviderKind::Read,
        ProviderKind::Dependent,
        ProviderKind::Transform,
    ]
    .into_iter()
    .flat_map(|kind| [false, true].map(move |flag| (kind, flag)))
    {
        let (registry, schema) = registry().map_err(error)?;
        let first = signature(&schema, ProviderKind::Read);
        let mut second = signature(&schema, kind);
        if kind == ProviderKind::Dependent {
            second.value_input = TypeDescriptor::Unit;
        }
        let expressions = match kind {
            ProviderKind::Read => vec![
                ReaderExpr::Call(first.operation.clone()),
                ReaderExpr::Call(first.operation.clone()),
                ReaderExpr::Seq(vec![ReaderId(0), ReaderId(1)]),
            ],
            ProviderKind::Dependent => vec![
                ReaderExpr::Call(first.operation.clone()),
                ReaderExpr::Literal("".into()),
                ReaderExpr::Then {
                    first: ReaderId(1),
                    provider: second.operation.clone(),
                },
                ReaderExpr::Seq(vec![ReaderId(0), ReaderId(2)]),
            ],
            ProviderKind::Transform => vec![
                ReaderExpr::Call(first.operation.clone()),
                ReaderExpr::Map {
                    body: ReaderId(0),
                    provider: second.operation.clone(),
                },
            ],
        };
        let root = expressions.len() as u64 - 1;
        let output = if kind == ProviderKind::Transform {
            TypeDescriptor::Text
        } else {
            TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue))
        };
        let mut p = plan(&schema, expressions, root, output);
        p.providers.push(first);
        if kind != ProviderKind::Read {
            p.providers.push(second);
        }
        let checked = p.check(&registry, &mut budget()).map_err(error)?;
        let mut b = budget();
        let mut session = ReaderSession::new("accepted-map".into(), &checked, &registry, &mut b)
            .map_err(error)?;
        let input = source("a").map_err(error)?;
        let mut store = SourceStore::default();
        store.insert(input.clone()).map_err(error)?;
        let mut admission = SourceAdmission::default();
        let raw = context(&schema, &registry).map_err(error)?;
        let context =
            check_context(&raw, &store, &registry, &mut b, &mut admission).map_err(error)?;
        let wait = session
            .read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 1,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut admission,
            )
            .map_err(error)?;
        let ReadReply::Await { continuation, .. } = wait else {
            return Err("first Await".into());
        };
        let generated = admission
            .create(
                SourceId("accepted-generated".into()),
                0,
                "memory:accepted-generated".into(),
                b"g".to_vec(),
                &mut b,
            )
            .map_err(error)?;
        let mapping = Mapping {
            source: input.span(0, 1).map_err(error)?,
            target: generated.span(0, 1).map_err(error)?,
            kind: MappingKind::Transformed,
        };
        let ProviderReply::Read(mut prefix) = terminal(
            "a",
            if kind == ProviderKind::Transform {
                1
            } else {
                0
            },
            &mut b,
        )
        .map_err(error)?
        else {
            return Err("Read prefix".into());
        };
        let ReadReply::Matched {
            sources,
            source_maps,
            ..
        } = prefix.as_mut()
        else {
            return Err("Matched prefix".into());
        };
        sources.push(generated.clone());
        source_maps.push(mapping.clone());
        let wait = session
            .resume(
                &continuation,
                ProviderReply::Read(prefix),
                &store,
                &mut b,
                &mut admission,
            )
            .map_err(error)?;
        let ReadReply::Await { continuation, .. } = wait else {
            return Err("second Await".into());
        };
        assert_eq!(continuation.current.source_maps, vec![mapping.clone()]);
        let mut reply_sources = vec![];
        let mut reply_maps = vec![];
        let mut all_maps = vec![mapping.clone()];
        let mut all_sources = vec![generated.clone()];
        let mut child = mapping.target.clone();
        if with_delta {
            let next = admission
                .create(
                    SourceId("reply-generated".into()),
                    0,
                    "memory:reply-generated".into(),
                    b"h".to_vec(),
                    &mut b,
                )
                .map_err(error)?;
            let next_map = Mapping {
                source: child,
                target: next.span(0, 1).map_err(error)?,
                kind: MappingKind::Transformed,
            };
            child = next_map.target.clone();
            reply_maps.push(next_map.clone());
            all_maps.push(next_map);
            reply_sources.push(next.clone());
            all_sources.push(next);
        }
        let view = mapped_view(&schema, mapping.source.clone(), child);
        // Construct the view's NDF independently of either terminal reply encoder.
        let mut declared = SourceStore::default();
        declared.insert(input.clone()).map_err(error)?;
        for source in &all_sources {
            declared.insert(source.clone()).map_err(error)?;
        }
        let mapped_ndf = {
            let mut independent =
                FoundationCodec::new(&registry, &declared, &mut admission).map_err(error)?;
            independent
                .scoped_with_mappings(&declared, &all_maps)
                .encode_views(&view, &mut b)
                .map_err(error)?
        };
        let empty = SourceStore::default();
        let reply = if kind == ProviderKind::Transform {
            let expected = TransformReply {
                outcome: TransformOutcome::Complete {
                    value: NdfValue::Text("B".into()),
                    view: view.clone(),
                    facts: vec![],
                },
                sources: reply_sources.clone(),
                source_maps: reply_maps.clone(),
                report: Report {
                    usage: b.usage(),
                    ..Report::default()
                },
            };
            let proof = session.pending_transform().map_err(error)?;
            let mut codec =
                FoundationCodec::new(&registry, &empty, &mut admission).map_err(error)?;
            let value =
                transform::reply_to_value(&expected, &proof, &mut codec, &mut b).map_err(error)?;
            let reverse = Mapping {
                source: mapping.target.clone(),
                target: mapping.source.clone(),
                kind: MappingKind::Transformed,
            };
            let mut invalid = expected.clone();
            invalid.source_maps = vec![reverse.clone()];
            assert!(matches!(
                transform::reply_to_value(&invalid, &proof, &mut codec, &mut b),
                Err(PortableError::Reader(ReaderError::Origin(
                    OriginError::Cycle
                )))
            ));
            let encoded_cycle = codec
                .scoped(&declared)
                .encode_mappings(&[reverse], &mut b)
                .map_err(error)?;
            let mut cycle = value.clone();
            let NdfValue::Record(record) = &mut cycle else {
                return Err("TransformReply cycle".into());
            };
            record.fields[2] = encoded_cycle;
            let bytes = nepl3_wire::encode(&cycle, &mut b).map_err(error)?;
            let cycle = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
            assert!(matches!(
                transform::reply_from_value(&cycle, &proof, &mut codec, &mut b),
                Err(PortableError::Boundary(nepl3_wire::WireError::Origin(
                    OriginError::Cycle
                )))
            ));
            assert_eq!(b.poll(), Ok(()));
            assert!(session.pending_transform().is_ok());
            let mut baseline = expected.clone();
            let TransformOutcome::Complete { view, .. } = &mut baseline.outcome else {
                return Err("Complete".into());
            };
            *view = ViewBundle {
                elements: vec![],
                roots: vec![],
            };
            let mut independent =
                transform::reply_to_value(&baseline, &proof, &mut codec, &mut b).map_err(error)?;
            let NdfValue::Record(record) = &mut independent else {
                return Err("TransformReply record".into());
            };
            let NdfValue::Variant(outcome) = &mut record.fields[0] else {
                return Err("outcome".into());
            };
            outcome.fields[1] = mapped_ndf;
            assert_eq!(independent, value);
            let bytes = nepl3_wire::encode(&independent, &mut b).map_err(error)?;
            let value = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
            let reply =
                transform::reply_from_value(&value, &proof, &mut codec, &mut b).map_err(error)?;
            assert_eq!(reply, expected);
            assert_eq!(reply.source_maps, reply_maps);
            ProviderReply::Transform(Box::new(reply))
        } else {
            let ProviderReply::Read(mut expected) = terminal("B", 1, &mut b).map_err(error)? else {
                return Err("Read terminal".into());
            };
            let ReadReply::Matched {
                view: reply_view,
                sources,
                source_maps,
                ..
            } = expected.as_mut()
            else {
                return Err("Matched terminal".into());
            };
            *reply_view = view;
            *sources = reply_sources.clone();
            *source_maps = reply_maps.clone();
            let proof = session.pending_read().map_err(error)?;
            let mut codec =
                FoundationCodec::new(&registry, &empty, &mut admission).map_err(error)?;
            let value =
                read::reply_to_value(&expected, &proof, &mut codec, &mut b).map_err(error)?;
            let reverse = Mapping {
                source: mapping.target.clone(),
                target: mapping.source.clone(),
                kind: MappingKind::Transformed,
            };
            let mut invalid = expected.clone();
            let ReadReply::Matched { source_maps, .. } = invalid.as_mut() else {
                return Err("Matched cycle".into());
            };
            *source_maps = vec![reverse.clone()];
            assert!(matches!(
                read::reply_to_value(&invalid, &proof, &mut codec, &mut b),
                Err(PortableError::Reader(ReaderError::Origin(
                    OriginError::Cycle
                )))
            ));
            let encoded_cycle = codec
                .scoped(&declared)
                .encode_mappings(&[reverse], &mut b)
                .map_err(error)?;
            let mut cycle = value.clone();
            let NdfValue::Variant(reply) = &mut cycle else {
                return Err("ReadReply cycle".into());
            };
            reply.fields[6] = encoded_cycle;
            let bytes = nepl3_wire::encode(&cycle, &mut b).map_err(error)?;
            let cycle = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
            assert!(matches!(
                read::reply_from_value(&cycle, &proof, &mut codec, &mut b),
                Err(PortableError::Boundary(nepl3_wire::WireError::Origin(
                    OriginError::Cycle
                )))
            ));
            assert_eq!(b.poll(), Ok(()));
            assert!(session.pending_read().is_ok());
            let mut baseline = expected.clone();
            let ReadReply::Matched { view, .. } = baseline.as_mut() else {
                return Err("Matched baseline".into());
            };
            *view = ViewBundle {
                elements: vec![],
                roots: vec![],
            };
            let mut independent =
                read::reply_to_value(&baseline, &proof, &mut codec, &mut b).map_err(error)?;
            let NdfValue::Variant(reply) = &mut independent else {
                return Err("ReadReply variant".into());
            };
            // Matched(value, end, newState, view, facts, sources, sourceMaps, report).
            reply.fields[3] = mapped_ndf;
            assert_eq!(independent, value);
            let bytes = nepl3_wire::encode(&independent, &mut b).map_err(error)?;
            let value = nepl3_wire::decode(&bytes, &mut b).map_err(error)?;
            let reply =
                read::reply_from_value(&value, &proof, &mut codec, &mut b).map_err(error)?;
            assert_eq!(&reply, expected.as_ref());
            let ReadReply::Matched { source_maps, .. } = &reply else {
                return Err("decoded Matched".into());
            };
            assert_eq!(*source_maps, reply_maps);
            ProviderReply::Read(Box::new(reply))
        };
        let final_reply = session
            .resume(&continuation, reply, &store, &mut b, &mut admission)
            .map_err(error)?;
        let ReadReply::Matched {
            view,
            sources,
            source_maps,
            ..
        } = final_reply
        else {
            return Err("final Matched".into());
        };
        assert_eq!(view.elements.len(), 2);
        assert_eq!(source_maps, all_maps);
        assert_eq!(sources, all_sources);
    }
    Ok(())
}
