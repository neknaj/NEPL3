use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, TypeDescriptor, TypeRef},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    value::NdfValue,
};
use nepl3_doc_core::{
    model::*,
    portable::{self, PortableError},
};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 2_000_000,
        allocation_units: 500_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        let s = d.reference(&mut b()).map_err(err)?;
        r.register(s, d, &mut b()).map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn literal(r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    let source = SourceSnapshot::new(
        SourceId("doc".into()),
        3,
        "memory:doc".into(),
        "\"前{[文/ぶん]/note}後\"".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    document_fixture(source, r)
}

fn document_fixture(source: SourceSnapshot, r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    use nepl3_core::{
        origin::{Origin, OriginId},
        value::KindRef,
        view::{ViewBundle, ViewElement, ViewField, ViewRef},
    };
    // Hand-authored semantic arena for "前{[文/ぶん]/note}後". Byte ranges
    // refer to the original UTF-8 source, independently of any Sentence parser.
    let entries = [
        (
            DocKind::Text { text: "前".into() },
            1,
            4,
            "View:TextRun",
            vec![],
        ),
        (
            DocKind::Text { text: "文".into() },
            6,
            9,
            "View:TextRun",
            vec![],
        ),
        (
            DocKind::Text {
                text: "ぶん".into(),
            },
            10,
            16,
            "View:TextRun",
            vec![],
        ),
        (
            DocKind::Ruby {
                base: InlineRef(1),
                reading: InlineRef(2),
            },
            5,
            17,
            "View:Ruby",
            vec![ViewRef(1), ViewRef(2)],
        ),
        (
            DocKind::Text {
                text: "note".into(),
            },
            18,
            22,
            "View:TextRun",
            vec![],
        ),
        (
            DocKind::Anno {
                base: InlineRef(3),
                notes: vec![InlineRef(4)],
            },
            4,
            23,
            "View:Anno",
            vec![ViewRef(3), ViewRef(4)],
        ),
        (
            DocKind::Text { text: "後".into() },
            23,
            26,
            "View:TextRun",
            vec![],
        ),
        (
            DocKind::Sentence {
                inlines: vec![InlineRef(0), InlineRef(5), InlineRef(6)],
            },
            0,
            27,
            "View:Sentence",
            vec![ViewRef(0), ViewRef(5), ViewRef(6)],
        ),
    ];
    let schema = r.selected("nepl3.doc", 1).ok_or("Doc schema")?;
    let mut nodes = Vec::new();
    let mut origins = Vec::new();
    let mut elements = Vec::new();
    for (kind, start, end, view_kind, children) in entries {
        let span = source.span(start, end).map_err(err)?;
        let origin = OriginId(origins.len() as u64);
        origins.push(Origin::Direct(span.clone()));
        nodes.push(DocNode {
            kind,
            origin: Some(origin),
            span: Some(span.clone()),
            locations: vec![],
        });
        elements.push(ViewElement {
            kind: KindRef {
                schema: schema.clone(),
                local_kind: r.kind_id(schema, view_kind).map_err(err)?,
            },
            span,
            fields: if children.is_empty() {
                vec![]
            } else {
                vec![ViewField {
                    name: "items".into(),
                    children,
                }]
            },
            roles: vec![],
            relations: vec![],
        });
    }
    let document = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(7)),
            nodes,
            embeds: vec![],
        },
        views: vec![DocView {
            head: source.span(0, 27).map_err(err)?,
            view: ViewBundle {
                roots: vec![ViewRef(7)],
                elements,
            },
        }],
        sources: vec![source],
        origins,
        source_maps: vec![],
    };
    document
        .validate_structure(r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    Ok(document)
}

#[test]
fn source_uniqueness_uses_logical_revision_and_preserves_source_order() -> Result<(), String> {
    use nepl3_doc_core::check::StructureError;
    let r = registry()?;
    let mut document = literal(&r)?;
    let first = document.sources[0].clone();
    let revision = SourceSnapshot::new(
        SourceId("doc".into()),
        4,
        "memory:doc".into(),
        b"later".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let other = SourceSnapshot::new(
        SourceId("別".into()),
        3,
        "memory:other".into(),
        b"other".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let sources = [first, revision, other];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        document.sources = order.iter().map(|i| sources[*i].clone()).collect();
        let original = document.sources.clone();
        document
            .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(document.sources, original);
        for duplicate in &sources {
            document.sources.push(duplicate.clone());
            assert!(matches!(
                document.validate_structure(&r, &mut b(), &mut SourceAdmission::default()),
                Err(StructureError::DuplicateSource)
            ));
            document.sources.pop();
        }
    }
    Ok(())
}

#[test]
fn many_source_revisions_fit_a_bounded_index_lookup_allowance() -> Result<(), String> {
    let r = registry()?;
    let mut document = literal(&r)?;
    for index in 0..256 {
        document.sources.push(
            SourceSnapshot::new(
                SourceId(format!("fixture/{index:04}")),
                0,
                "memory:fixture".into(),
                Vec::new(),
                &mut b(),
            )
            .map_err(err)?,
        );
    }
    // Enough for charged index comparisons and unchanged source/view checks;
    // not enough for comparing every source with every previous declaration.
    let mut limited = Budget::new(Limits {
        work: 500_000,
        ..b().limits()
    });
    document
        .validate_structure(&r, &mut limited, &mut SourceAdmission::default())
        .map_err(err)?;
    let mut cancelled = b();
    cancelled.cancel();
    assert!(matches!(
        document.validate_structure(&r, &mut cancelled, &mut SourceAdmission::default()),
        Err(nepl3_doc_core::check::StructureError::Stopped(
            StopReason::Cancelled
        ))
    ));
    Ok(())
}

#[test]
fn sentence_payload_uses_only_the_explicit_owner_after_cbor() -> Result<(), String> {
    let r = registry()?;
    let source = SourceSnapshot::new(
        SourceId("large".into()),
        4,
        "memory:large".into(),
        format!("\"前{{[文/ぶん]/note}}後\"{}", " ".repeat(65_536)).into_bytes(),
        &mut b(),
    )
    .map_err(err)?;
    let doc = document_fixture(source.clone(), &r)?;
    let mut ambient = SourceStore::default();
    ambient.insert(source.clone()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &ambient, &mut admission).map_err(err)?;
    let payload = portable::sentence::to_value(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&payload, &mut b()).map_err(err)?;
    // The owner has 64 KiB of unrelated source. The wire payload contains just
    // this short sentence's semantic/view data and snapshot references.
    assert!(bytes.len() < 16_384, "payload length {}", bytes.len());
    let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let actual = portable::sentence::from_value(&received, &source, &r, &mut codec, &mut b())
        .map_err(err)?;
    assert_eq!(actual, doc);
    let wrong = SourceSnapshot::new(
        SourceId("large".into()),
        5,
        "memory:large".into(),
        source.text().as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    // Even though the correct source exists in ambient, only the explicit
    // owner may satisfy references at this first receiving boundary.
    assert!(portable::sentence::from_value(&received, &wrong, &r, &mut codec, &mut b()).is_err());
    let altered = SourceSnapshot::new(
        SourceId("large".into()),
        4,
        "memory:large".into(),
        b"different bytes".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    assert!(portable::sentence::from_value(&received, &altered, &r, &mut codec, &mut b()).is_err());
    for stop in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match stop {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => (),
        }
        let mut limited = Budget::new(limits);
        if stop == StopReason::Cancelled {
            limited.cancel();
        }
        assert!(
            matches!(portable::sentence::from_value(&received, &source, &r, &mut codec, &mut limited),
            Err(PortableError::Stopped(s)) if s==stop)
        );
        assert_eq!(limited.poll(), Err(stop));
    }
    let mut limits = b().limits();
    limits.source_bytes = 0;
    let mut fresh_budget = Budget::new(limits);
    let mut fresh_admission = SourceAdmission::default();
    let mut fresh_codec = FoundationCodec::new(&r, &ambient, &mut fresh_admission).map_err(err)?;
    assert!(matches!(
        portable::sentence::from_value(&received, &source, &r, &mut fresh_codec, &mut fresh_budget),
        Err(PortableError::Stopped(StopReason::SourceLimit))
    ));
    assert_eq!(fresh_budget.poll(), Err(StopReason::SourceLimit));
    assert_eq!(doc.sources[0], source);
    Ok(())
}
#[test]
fn document_ndf_and_cbor_first_receiver_preserve_source_view_and_meaning() -> Result<(), String> {
    let r = registry()?;
    let doc = literal(&r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut budget = b();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = portable::to_value(&doc, &r, &mut codec, &mut budget).map_err(err)?;
    assert_eq!(
        budget.usage().source_bytes,
        doc.sources[0].text().len() as u64
    );
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let mut receiver = b();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let actual = portable::from_value(&value, &r, &mut codec, &mut receiver).map_err(err)?;
    assert_eq!(actual, doc);
    assert_eq!(
        receiver.usage().source_bytes,
        doc.sources[0].text().len() as u64
    );
    assert_eq!(
        portable::to_value(&actual, &r, &mut codec, &mut receiver).map_err(err)?,
        value
    );
    assert_eq!(
        receiver.usage().source_bytes,
        doc.sources[0].text().len() as u64
    );
    Ok(())
}
#[test]
fn schema_valid_wrong_category_and_resource_stops_are_typed() -> Result<(), String> {
    let r = registry()?;
    let doc = literal(&r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = portable::to_value(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    let mut invalid = value.clone();
    let NdfValue::Record(document) = &mut invalid else {
        return Err("document".into());
    };
    let NdfValue::Record(arena) = &mut document.fields[0] else {
        return Err("arena".into());
    };
    let NdfValue::Variant(root) = &mut arena.fields[0] else {
        return Err("root".into());
    };
    let NdfValue::Record(index) = &mut root.fields[0] else {
        return Err("index".into());
    };
    index.fields[0] = NdfValue::U64(0); // Node zero is Text, never a Sentence.
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.doc".into(),
            revision: 1,
            name: "DocumentSyntax".into(),
        }),
        &invalid,
        &mut b(),
    )
    .map_err(err)?;
    assert!(matches!(
        portable::from_value(&invalid, &r, &mut codec, &mut b()),
        Err(PortableError::Structure(_))
    ));
    for (resource, reason) in [
        (0, StopReason::SourceLimit),
        (1, StopReason::WorkLimit),
        (2, StopReason::AllocationLimit),
        (3, StopReason::DepthLimit),
        (4, StopReason::Cancelled),
    ] {
        let mut limits = b().limits();
        match resource {
            0 => limits.source_bytes = 0,
            1 => limits.work = 0,
            2 => limits.allocation_units = 0,
            3 => limits.depth = 0,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if resource == 4 {
            budget.cancel();
        }
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        assert!(
            matches!(portable::from_value(&value,&r,&mut codec,&mut budget),Err(PortableError::Stopped(actual)) if actual==reason)
        );
    }
    Ok(())
}

#[test]
fn valid_document_decode_keeps_work_stop_causes_at_inner_boundaries() -> Result<(), String> {
    let r = registry()?;
    let doc = literal(&r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = portable::to_value(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    let mut full = b();
    portable::from_value(&value, &r, &mut codec, &mut full).map_err(err)?;
    let step = (full.usage().work / 120).max(1);
    let mut stops = 0;
    for cap in (0..full.usage().work).step_by(step as usize) {
        let mut limits = b().limits();
        limits.work = cap;
        let mut budget = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        match portable::from_value(&value, &r, &mut codec, &mut budget) {
            Err(PortableError::Stopped(StopReason::WorkLimit)) => {
                stops += 1;
            }
            Ok(actual) => {
                assert_eq!(actual, doc);
                assert!(budget.usage().work <= cap);
            }
            Err(other) => return Err(format!("Work cap {cap}: {other:?}")),
        }
    }
    assert!(stops > 0);
    Ok(())
}

#[test]
fn mapped_view_parent_child_roundtrip_requires_explicit_mapping_closure() -> Result<(), String> {
    use nepl3_core::{
        origin::{Mapping, MappingKind},
        value::KindRef,
        view::{ViewBundle, ViewElement, ViewField, ViewRef},
    };
    let r = registry()?;
    let mut original = literal(&r)?;
    let source = original.sources[0].clone();
    let head = source.span(0, source.text().len() as u64).map_err(err)?;
    let decoded = SourceSnapshot::new(
        SourceId("transformed".into()),
        1,
        "memory:transformed".into(),
        source.text().as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let mapped = decoded.span(0, decoded.text().len() as u64).map_err(err)?;
    let schema = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: r.kind_id(schema, "Token").map_err(err)?,
    };
    // Identical byte ranges in different snapshots require an explicit Exact map;
    // neither the ambient store nor matching text is a containment proof.
    original.views = vec![DocView {
        head: head.clone(),
        view: ViewBundle {
            roots: vec![ViewRef(0)],
            elements: vec![
                ViewElement {
                    kind: kind.clone(),
                    span: head.clone(),
                    fields: vec![ViewField {
                        name: "decoded".into(),
                        children: vec![ViewRef(1)],
                    }],
                    roles: vec![],
                    relations: vec![],
                },
                ViewElement {
                    kind,
                    span: mapped.clone(),
                    fields: vec![],
                    roles: vec![],
                    relations: vec![],
                },
            ],
        },
    }];
    original.sources.push(decoded);
    original.source_maps = vec![Mapping {
        source: head,
        target: mapped,
        kind: MappingKind::Exact,
    }];
    original
        .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let value = portable::to_value(&original, &r, &mut c, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let raw = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let actual = portable::from_value(&raw, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(actual, original);
    let mut invalid = raw;
    let NdfValue::Record(missing_map) = &mut invalid else {
        return Err("record".into());
    };
    missing_map.fields[4] = NdfValue::List(vec![]);
    assert!(portable::from_value(&invalid, &r, &mut c, &mut b()).is_err());
    original.source_maps.clear();
    assert!(portable::to_value(&original, &r, &mut c, &mut b()).is_err());
    Ok(())
}

#[test]
fn additional_views_reuse_the_same_source_closure() -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let r = registry()?;
    let mut document = literal(&r)?;
    let source = document.sources[0].clone();
    let decoded = SourceSnapshot::new(
        SourceId("decoded".into()),
        1,
        "memory:decoded".into(),
        source.text().as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let mapping = Mapping {
        source: source.span(0, source.text().len() as u64).map_err(err)?,
        target: decoded.span(0, decoded.text().len() as u64).map_err(err)?,
        kind: MappingKind::Exact,
    };
    document.sources.push(decoded);
    let view = document.views[0].clone();
    let mut increments = Vec::new();
    for count in [8, 16, 32] {
        document.source_maps = vec![mapping.clone(); count];
        let mut costs = Vec::new();
        for views in [1, 17] {
            document.views = vec![view.clone(); views];
            let mut budget = b();
            document
                .validate_structure(&r, &mut budget, &mut SourceAdmission::default())
                .map_err(err)?;
            costs.push(budget.usage().work);
        }
        // These views reference the original snapshot directly. Extra valid
        // mappings change initial closure validation, not the work of adding
        // identical views after that immutable store has been validated.
        increments.push(costs[1] - costs[0]);
    }
    assert!(increments[0] > 0);
    assert_eq!(increments, vec![increments[0]; 3]);
    document.sources.pop();
    assert!(
        document
            .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}
