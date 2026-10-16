use super::*;
use nepl3_core::{
    value::NdfValue,
    value_codec::{
        GroupEncodingError,
        archive::{FoundationGroupArchiveCodec, GroupArchiveEncoding},
    },
};
use nepl3_doc_core::portable::{PortableError, archive as codec};
fn setup() -> Result<SchemaRegistry, String> {
    let mut r = registry()?;
    for d in [
        nepl3_core::schema::archive::descriptor(&mut b()),
        nepl3_core::schema::archive::group::descriptor(&mut b()),
        nepl3_doc_core::schema::archive::descriptor(&mut b()),
        nepl3_doc_core::schema::archive::pages::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        let reference = d.reference(&mut b()).map_err(err)?;
        r.register(reference, d, &mut b()).map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn document(r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    Ok(mapped_article(closure(r)?, vec![], vec![]))
}
fn encode(
    doc: &DocumentSyntax,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<codec::DocumentArchiveEncoding, PortableError<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c =
        FoundationCodec::new(r, &empty, &mut admission).map_err(PortableError::Foundation)?;
    codec::to_value(doc, r, &mut c, budget)
}
fn decode(
    value: &NdfValue,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<DocumentSyntax, PortableError<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c =
        FoundationCodec::new(r, &empty, &mut admission).map_err(PortableError::Foundation)?;
    codec::from_value(value, r, &mut c, budget)
}
fn fields(v: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match v {
        NdfValue::Record(r) => Ok(&mut r.fields),
        _ => Err("record".into()),
    }
}
#[test]
fn document_archive_roundtrip_preserves_v1_boundary_and_rejects_cross_route() -> Result<(), String>
{
    let r = setup()?;
    let doc = document(&r)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let legacy = portable::to_value(&doc, &r, &mut c, &mut b()).map_err(err)?;
    let encoded = encode(&doc, &r, &mut b()).map_err(err)?;
    assert_eq!(encoded.guest_components.len(), 1);
    let wire = nepl3_wire::encode(&encoded.value, &mut b()).map_err(err)?;
    let received = nepl3_wire::decode(&wire, &mut b()).map_err(err)?;
    let actual = decode(&received, &r, &mut b()).map_err(err)?;
    assert_eq!(actual, doc);
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    assert_eq!(
        portable::to_value(&actual, &r, &mut c, &mut b()).map_err(err)?,
        legacy
    );
    assert!(portable::from_value(&received, &r, &mut c, &mut b()).is_err());
    assert!(decode(&legacy, &r, &mut b()).is_err());
    let NdfValue::Record(root) = &received else {
        return Err("root".into());
    };
    assert!(portable::from_value(&root.fields[0], &r, &mut c, &mut b()).is_err());
    Ok(())
}
#[test]
fn document_archive_rejects_nonempty_placeholder_and_missing_group_membership() -> Result<(), String>
{
    let r = setup()?;
    let doc = document(&r)?;
    let encoded = encode(&doc, &r, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let group = c
        .encode_foreign_group(&doc.value.embeds.group, &mut b())
        .map_err(err)?;
    let mut bad = encoded.value.clone();
    let skeleton = &mut fields(&mut bad)?[0];
    let value = &mut fields(skeleton)?[0];
    let embeds = &mut fields(value)?[2];
    fields(embeds)?[1] = group;
    assert!(decode(&bad, &r, &mut b()).is_err());
    let mut bad = encoded.value;
    let NdfValue::Bytes(bytes) = &fields(&mut bad)?[1] else {
        return Err("bytes".into());
    };
    let mut group = nepl3_wire::decode(bytes, &mut b()).map_err(err)?;
    let sources = &mut fields(&mut group)?[0];
    let NdfValue::List(members) = &mut fields(sources)?[1] else {
        return Err("members".into());
    };
    members.pop();
    fields(&mut bad)?[1] = NdfValue::Bytes(nepl3_wire::encode(&group, &mut b()).map_err(err)?);
    assert!(decode(&bad, &r, &mut b()).is_err());
    Ok(())
}
#[test]
fn document_archive_rejects_document_guest_source_conflict() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    let mut encoded = encode(&doc, &r, &mut b()).map_err(err)?.value;
    let original = &doc.value.embeds.group.entries[0].syntax.bundle.sources[0];
    let conflicting = SourceSnapshot::new(
        original.identity().source.clone(),
        original.identity().revision,
        "memory:conflict".into(),
        b"different content".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let sources = c.encode_sources(&[conflicting], &mut b()).map_err(err)?;
    fields(&mut fields(&mut encoded)?[0])?[1] = sources;
    assert!(decode(&encoded, &r, &mut b()).is_err());
    Ok(())
}
#[test]
fn document_archive_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    for decoding in [false, true] {
        let input = encode(&doc, &r, &mut b()).map_err(err)?.value;
        let run = |budget: &mut Budget| -> Result<(), PortableError<nepl3_wire::WireError>> {
            if decoding {
                decode(&input, &r, budget).map(|_| ())
            } else {
                encode(&doc, &r, budget).map(|_| ())
            }
        };
        let mut measured = b();
        run(&mut measured).map_err(err)?;
        let u = measured.usage();
        let limits = Limits {
            source_bytes: u.source_bytes,
            work: u.work,
            depth: u.depth,
            nodes: u.nodes,
            allocation_units: u.allocation_units,
            output_bytes: u.output_bytes,
            diagnostics: u.diagnostics,
            events: u.events,
        };
        run(&mut Budget::new(limits)).map_err(err)?;
        for resource in 0..6 {
            let mut short = limits;
            let reason = match resource {
                0 => {
                    short.source_bytes -= 1;
                    StopReason::SourceLimit
                }
                1 => {
                    short.work -= 1;
                    StopReason::WorkLimit
                }
                2 => {
                    short.depth -= 1;
                    StopReason::DepthLimit
                }
                3 => {
                    short.nodes -= 1;
                    StopReason::NodeLimit
                }
                4 => {
                    short.allocation_units -= 1;
                    StopReason::AllocationLimit
                }
                _ => {
                    if short.output_bytes == 0 {
                        continue;
                    }
                    short.output_bytes -= 1;
                    StopReason::OutputLimit
                }
            };
            let mut budget = Budget::new(short);
            assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
            let retained = budget.usage();
            assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(PortableError::Stopped(StopReason::Cancelled))
        );
    }
    Ok(())
}
// Existing alternate provider deliberately keeps v1 unfused defaults. Archive
// support is explicit delegation, never silently supplied by the ordinary trait.
impl<C: FoundationGroupArchiveCodec> FoundationGroupArchiveCodec
    for super::default::DefaultCodec<C>
{
    fn encode_compact_group_at_depths(
        &mut self,
        g: &nepl3_core::syntax::group::ForeignClosureGroup,
        d: &[u64],
        r: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<GroupArchiveEncoding, GroupEncodingError<Self::Error>> {
        let mut encoded = self.0.encode_compact_group_at_depths(g, d, r, b)?;
        match self.1 {
            super::default::ArchiveComponents::Exact => {}
            super::default::ArchiveComponents::Short => {
                encoded.entry_digests.pop();
            }
            super::default::ArchiveComponents::Long => {
                encoded
                    .entry_digests
                    .push(nepl3_core::source::Digest([0; 32]));
            }
        }
        Ok(encoded)
    }
    fn decode_group_archive(
        &mut self,
        bytes: &[u8],
        b: &mut Budget,
    ) -> Result<nepl3_core::syntax::group::ForeignClosureGroup, Self::Error> {
        self.0.decode_group_archive(bytes, b)
    }
}
#[test]
fn document_archive_alternate_codec_matches() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    let expected = encode(&doc, &r, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut c = super::default::DefaultCodec::new(c);
    let actual = codec::to_value(&doc, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(actual.value, expected.value);
    assert_eq!(actual.guest_components, expected.guest_components);
    let received = codec::from_value(&actual.value, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(received, doc);
    Ok(())
}

#[test]
fn document_archive_rejects_provider_component_count_mismatch() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    for mode in [
        super::default::ArchiveComponents::Short,
        super::default::ArchiveComponents::Long,
    ] {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let inner = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let mut c = super::default::DefaultCodec::new(inner);
        c.1 = mode;
        assert!(matches!(
            codec::to_value(&doc, &r, &mut c, &mut b()),
            Err(PortableError::Shape)
        ));
    }
    Ok(())
}
#[test]
fn document_archive_empty_group_and_distinct_registry() -> Result<(), String> {
    let r = setup()?;
    let equivalent = setup()?;
    let mut doc = document(&r)?;
    doc.value.nodes[4].kind = DocKind::Paragraph { items: vec![] };
    doc.value.embeds = DocEmbeds {
        kinds: vec![],
        group: nepl3_core::syntax::group::ForeignClosureGroup {
            provenances: vec![],
            environments: vec![],
            entries: vec![],
        },
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&equivalent, &empty, &mut admission).map_err(err)?;
    let encoded = codec::to_value(&doc, &r, &mut c, &mut b()).map_err(err)?;
    assert!(encoded.guest_components.is_empty());
    assert_eq!(
        codec::from_value(&encoded.value, &r, &mut c, &mut b()).map_err(err)?,
        doc
    );
    let missing = registry()?;
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&missing, &empty, &mut admission).map_err(err)?;
    assert!(codec::to_value(&doc, &r, &mut c, &mut b()).is_err());
    assert!(codec::from_value(&encoded.value, &r, &mut c, &mut b()).is_err());
    Ok(())
}
#[test]
fn document_archive_receiver_checks_deepest_shared_occurrence() -> Result<(), String> {
    let r = setup()?;
    let mut nodes = vec![DocNode {
        kind: DocKind::InlineMath {
            syntax: EmbedRef(0),
        },
        locations: vec![],
        origin: None,
        span: None,
    }];
    for i in 1..20 {
        nodes.push(DocNode {
            kind: DocKind::Strong {
                inline: InlineRef(i - 1),
            },
            locations: vec![],
            origin: None,
            span: None,
        });
    }
    nodes.push(DocNode {
        kind: DocKind::Sentence {
            inlines: vec![InlineRef(0), InlineRef(19)],
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    let doc = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(20)),
            nodes,
            embeds: embeds(EmbedKind::InlineMath, closure(&r)?),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let encoded = encode(&doc, &r, &mut b()).map_err(err)?;
    let NdfValue::Record(root) = &encoded.value else {
        return Err("archive".into());
    };
    let NdfValue::Bytes(bytes) = &root.fields[1] else {
        return Err("bytes".into());
    };
    let low = Limits {
        depth: 21,
        ..b().limits()
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    c.decode_group_archive(bytes, &mut Budget::new(low))
        .map_err(err)?;
    assert!(matches!(
        decode(&encoded.value, &r, &mut Budget::new(low)),
        Err(PortableError::Stopped(StopReason::DepthLimit))
    ));
    let mut measured = b();
    measured
        .with_depth_at_least(4, |b| decode(&encoded.value, &r, b))
        .map_err(err)?;
    let limit = Limits {
        depth: measured.usage().depth,
        ..b().limits()
    };
    let mut exact = Budget::new(limit);
    exact
        .with_depth_at_least(4, |b| decode(&encoded.value, &r, b))
        .map_err(err)?;
    assert_eq!(exact.current_depth(), 0);
    let mut short = Budget::new(Limits {
        depth: limit.depth - 1,
        ..limit
    });
    assert!(matches!(
        short.with_depth_at_least(4, |b| decode(&encoded.value, &r, b)),
        Err(PortableError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(short.current_depth(), 0);
    Ok(())
}

#[test]
fn document_archive_composite_boundary_uses_one_budget() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    let run =
        |budget: &mut Budget| -> Result<DocumentSyntax, PortableError<nepl3_wire::WireError>> {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &empty, &mut admission)
                .map_err(PortableError::Foundation)?;
            let archive = codec::to_value(&doc, &r, &mut c, budget)?;
            let bytes = nepl3_wire::encode(&archive.value, budget).map_err(|e| {
                match nepl3_core::value_codec::FoundationCodecError::stop_reason(&e) {
                    Some(r) => PortableError::Stopped(r),
                    None => PortableError::Foundation(e),
                }
            })?;
            let value = nepl3_wire::decode(&bytes, budget).map_err(|e| {
                match nepl3_core::value_codec::FoundationCodecError::stop_reason(&e) {
                    Some(r) => PortableError::Stopped(r),
                    None => PortableError::Foundation(e),
                }
            })?;
            codec::from_value(&value, &r, &mut c, budget)
        };
    let mut measured = b();
    assert_eq!(run(&mut measured).map_err(err)?, doc);
    let u = measured.usage();
    let limits = Limits {
        source_bytes: u.source_bytes,
        work: u.work,
        depth: u.depth,
        nodes: u.nodes,
        allocation_units: u.allocation_units,
        output_bytes: u.output_bytes,
        diagnostics: u.diagnostics,
        events: u.events,
    };
    assert_eq!(run(&mut Budget::new(limits)).map_err(err)?, doc);
    for resource in 0..6 {
        let mut short = limits;
        let reason = match resource {
            0 => {
                short.source_bytes -= 1;
                StopReason::SourceLimit
            }
            1 => {
                short.work -= 1;
                StopReason::WorkLimit
            }
            2 => {
                short.depth -= 1;
                StopReason::DepthLimit
            }
            3 => {
                short.nodes -= 1;
                StopReason::NodeLimit
            }
            4 => {
                short.allocation_units -= 1;
                StopReason::AllocationLimit
            }
            _ => {
                short.output_bytes -= 1;
                StopReason::OutputLimit
            }
        };
        let mut budget = Budget::new(short);
        assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
        let retained = budget.usage();
        assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
        assert_eq!(budget.usage(), retained);
    }
    Ok(())
}

#[test]
fn pages_archive_preserves_documents_files_and_rejects_legacy_route() -> Result<(), String> {
    use nepl3_doc_core::pages::{FileBytes, PageDocument, PageFile, PageRegistration, PageSet};
    let r = setup()?;
    let registration = |name: &str| PageRegistration {
        id: name.into(),
        source: format!("{name}.nepld"),
        route: format!("{name}.md"),
    };
    let set = PageSet {
        pages: vec![
            PageDocument {
                registration: registration("one"),
                document: document(&r)?,
            },
            PageDocument {
                registration: registration("two"),
                document: document(&r)?,
            },
        ],
        files: vec![PageFile {
            registration: registration("asset"),
            content: FileBytes(vec![0, 1, 255]),
        }],
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = codec::pages::to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    let wire = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let value = nepl3_wire::decode(&wire, &mut b()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    assert_eq!(
        codec::pages::from_value(&value, &r, &mut c, &mut b()).map_err(err)?,
        set
    );
    assert!(portable::pages::set_from_value(&value, &r, &mut c, &mut b()).is_err());
    let legacy = portable::pages::set_to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert!(codec::pages::from_value(&legacy, &r, &mut c, &mut b()).is_err());
    Ok(())
}

#[test]
fn pages_archive_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = setup()?;
    let doc = nepl3_doc_core::pages::PageSet {
        pages: vec![nepl3_doc_core::pages::PageDocument {
            registration: nepl3_doc_core::pages::PageRegistration {
                id: "p".into(),
                source: "p.nepld".into(),
                route: "p.md".into(),
            },
            document: document(&r)?,
        }],
        files: vec![],
    };
    for decoding in [false, true] {
        let encode = |budget: &mut Budget| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &empty, &mut admission)
                .map_err(PortableError::Foundation)?;
            codec::pages::to_value(&doc, &r, &mut c, budget)
        };
        let input = encode(&mut b()).map_err(err)?;
        let run = |budget: &mut Budget| -> Result<(), PortableError<nepl3_wire::WireError>> {
            if decoding {
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(&r, &empty, &mut admission)
                    .map_err(PortableError::Foundation)?;
                codec::pages::from_value(&input, &r, &mut c, budget).map(|_| ())
            } else {
                encode(budget).map(|_| ())
            }
        };
        let mut measured = b();
        run(&mut measured).map_err(err)?;
        let u = measured.usage();
        let limits = Limits {
            source_bytes: u.source_bytes,
            work: u.work,
            depth: u.depth,
            nodes: u.nodes,
            allocation_units: u.allocation_units,
            output_bytes: u.output_bytes,
            diagnostics: u.diagnostics,
            events: u.events,
        };
        run(&mut Budget::new(limits)).map_err(err)?;
        for resource in 0..6 {
            let mut short = limits;
            let reason = match resource {
                0 => {
                    short.source_bytes -= 1;
                    StopReason::SourceLimit
                }
                1 => {
                    short.work -= 1;
                    StopReason::WorkLimit
                }
                2 => {
                    short.depth -= 1;
                    StopReason::DepthLimit
                }
                3 => {
                    short.nodes -= 1;
                    StopReason::NodeLimit
                }
                4 => {
                    short.allocation_units -= 1;
                    StopReason::AllocationLimit
                }
                _ => {
                    if short.output_bytes == 0 {
                        continue;
                    }
                    short.output_bytes -= 1;
                    StopReason::OutputLimit
                }
            };
            let mut budget = Budget::new(short);
            assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
            let retained = budget.usage();
            assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(PortableError::Stopped(StopReason::Cancelled))
        );
    }
    Ok(())
}

fn page_set(documents: Vec<DocumentSyntax>) -> nepl3_doc_core::pages::PageSet {
    use nepl3_doc_core::pages::{PageDocument, PageRegistration, PageSet};
    PageSet {
        pages: documents
            .into_iter()
            .enumerate()
            .map(|(i, document)| PageDocument {
                registration: PageRegistration {
                    id: format!("p{i}"),
                    source: format!("p{i}.nepld"),
                    route: format!("p{i}.md"),
                },
                document,
            })
            .collect(),
        files: vec![],
    }
}
fn encode_pages(
    set: &nepl3_doc_core::pages::PageSet,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c =
        FoundationCodec::new(r, &empty, &mut admission).map_err(PortableError::Foundation)?;
    codec::pages::to_value(set, r, &mut c, budget)
}
fn decode_pages(
    value: &NdfValue,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<nepl3_doc_core::pages::PageSet, PortableError<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c =
        FoundationCodec::new(r, &empty, &mut admission).map_err(PortableError::Foundation)?;
    codec::pages::from_value(value, r, &mut c, budget)
}
fn archived_document(value: &mut NdfValue, page: usize) -> Result<&mut NdfValue, String> {
    let NdfValue::List(pages) = &mut fields(value)?[0] else {
        return Err("pages".into());
    };
    Ok(&mut fields(&mut pages[page])?[1])
}
#[test]
fn pages_archive_local_membership_and_cross_page_conflicts_reject() -> Result<(), String> {
    let r = setup()?;
    let doc = document(&r)?;
    let original = &doc.value.embeds.group.entries[0].syntax.bundle.sources[0];
    let conflict = SourceSnapshot::new(
        original.identity().source.clone(),
        original.identity().revision,
        "memory:other".into(),
        b"other".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let set = page_set(vec![doc.clone(), doc]);
    let value = encode_pages(&set, &r, &mut b()).map_err(err)?;
    let mut bad = value.clone();
    // A later page cannot borrow a missing declaration from the first page's admission.
    let group_bytes = &fields(archived_document(&mut bad, 1)?)?[1];
    let NdfValue::Bytes(bytes) = group_bytes else {
        return Err("group bytes".into());
    };
    let mut group = nepl3_wire::decode(bytes, &mut b()).map_err(err)?;
    // Use the same group-envelope membership mutation as the document-level test.
    let sources = &mut fields(&mut group)?[0];
    let NdfValue::List(members) = &mut fields(sources)?[1] else {
        return Err("members".into());
    };
    members.clear();
    fields(archived_document(&mut bad, 1)?)?[1] =
        NdfValue::Bytes(nepl3_wire::encode(&group, &mut b()).map_err(err)?);
    assert!(decode_pages(&bad, &r, &mut b()).is_err());
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let sources = c.encode_sources(&[conflict], &mut b()).map_err(err)?;
    let mut bad = value;
    fields(&mut fields(archived_document(&mut bad, 1)?)?[0])?[1] = sources;
    assert!(decode_pages(&bad, &r, &mut b()).is_err());
    Ok(())
}
#[test]
fn pages_archive_empty_files_only_and_missing_contracts() -> Result<(), String> {
    use nepl3_doc_core::pages::{FileBytes, PageFile, PageRegistration};
    let r = setup()?;
    let mut set = page_set(vec![]);
    for files_only in [false, true] {
        if files_only {
            set.files.push(PageFile {
                registration: PageRegistration {
                    id: "f".into(),
                    source: "f.svg".into(),
                    route: "f.svg".into(),
                },
                content: FileBytes(vec![0, 255]),
            });
        }
        let value = encode_pages(&set, &r, &mut b()).map_err(err)?;
        assert_eq!(decode_pages(&value, &r, &mut b()).map_err(err)?, set);
        assert!(encode_pages(&set, &registry()?, &mut b()).is_err());
        assert!(decode_pages(&value, &registry()?, &mut b()).is_err());
        let mut malformed = value;
        fields(&mut malformed)?[0] = NdfValue::Bytes(vec![]);
        assert!(decode_pages(&malformed, &r, &mut b()).is_err());
    }
    Ok(())
}

#[test]
fn pages_archive_composite_boundary_uses_one_budget() -> Result<(), String> {
    let r = setup()?;
    let doc = page_set(vec![document(&r)?, document(&r)?]);
    let run =
        |budget: &mut Budget| -> Result<nepl3_doc_core::pages::PageSet, PortableError<nepl3_wire::WireError>> {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &empty, &mut admission)
                .map_err(PortableError::Foundation)?;
            let archive = codec::pages::to_value(&doc, &r, &mut c, budget)?;
            let bytes = nepl3_wire::encode(&archive, budget).map_err(|e| {
                match nepl3_core::value_codec::FoundationCodecError::stop_reason(&e) {
                    Some(r) => PortableError::Stopped(r),
                    None => PortableError::Foundation(e),
                }
            })?;
            let value = nepl3_wire::decode(&bytes, budget).map_err(|e| {
                match nepl3_core::value_codec::FoundationCodecError::stop_reason(&e) {
                    Some(r) => PortableError::Stopped(r),
                    None => PortableError::Foundation(e),
                }
            })?;
            codec::pages::from_value(&value, &r, &mut c, budget)
        };
    let mut measured = b();
    assert_eq!(run(&mut measured).map_err(err)?, doc);
    let u = measured.usage();
    let limits = Limits {
        source_bytes: u.source_bytes,
        work: u.work,
        depth: u.depth,
        nodes: u.nodes,
        allocation_units: u.allocation_units,
        output_bytes: u.output_bytes,
        diagnostics: u.diagnostics,
        events: u.events,
    };
    assert_eq!(run(&mut Budget::new(limits)).map_err(err)?, doc);
    for resource in 0..6 {
        let mut short = limits;
        let reason = match resource {
            0 => {
                short.source_bytes -= 1;
                StopReason::SourceLimit
            }
            1 => {
                short.work -= 1;
                StopReason::WorkLimit
            }
            2 => {
                short.depth -= 1;
                StopReason::DepthLimit
            }
            3 => {
                short.nodes -= 1;
                StopReason::NodeLimit
            }
            4 => {
                short.allocation_units -= 1;
                StopReason::AllocationLimit
            }
            _ => {
                short.output_bytes -= 1;
                StopReason::OutputLimit
            }
        };
        let mut budget = Budget::new(short);
        assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
        let retained = budget.usage();
        assert_eq!(run(&mut budget), Err(PortableError::Stopped(reason)));
        assert_eq!(budget.usage(), retained);
    }
    Ok(())
}

#[test]
fn pages_archive_distinct_documents_alternate_provider_and_legacy_stability() -> Result<(), String>
{
    let r = setup()?;
    let first = document(&r)?;
    let mut second = document(&r)?;
    second.value.nodes[2].kind = DocKind::Text {
        text: "Different page".into(),
    };
    second.value.embeds.group.entries[0]
        .syntax
        .bundle
        .sources
        .push(
            SourceSnapshot::new(
                SourceId("second-only".into()),
                1,
                "memory:second".into(),
                b"unused local declaration".to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
    let set = page_set(vec![first, second]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let legacy = portable::pages::set_to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    let legacy_bytes = nepl3_wire::encode(&legacy, &mut b()).map_err(err)?;
    let plan = nepl3_doc_core::pages::resolve(&set, &r, &mut c, &mut b())
        .map_err(err)?
        .into_plan();
    let value = encode_pages(&set, &r, &mut b()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut c = super::default::DefaultCodec::new(c);
    assert_eq!(
        codec::pages::to_value(&set, &r, &mut c, &mut b()).map_err(err)?,
        value
    );
    let received = codec::pages::from_value(&value, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(received, set);
    let after = portable::pages::set_to_value(&received, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        nepl3_wire::encode(&after, &mut b()).map_err(err)?,
        legacy_bytes
    );
    assert_eq!(
        nepl3_doc_core::pages::resolve(&received, &r, &mut c, &mut b())
            .map_err(err)?
            .into_plan(),
        plan
    );
    Ok(())
}

#[test]
fn pages_archive_conflicting_individually_valid_pages_reject() -> Result<(), String> {
    let r = setup()?;
    let mut first = document(&r)?;
    let mut second = document(&r)?;
    for (doc, text) in [
        (&mut first, b"first".as_slice()),
        (&mut second, b"second".as_slice()),
    ] {
        doc.sources.push(
            SourceSnapshot::new(
                SourceId("page-owned".into()),
                1,
                "memory:page-owned".into(),
                text.to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
    }
    // Each page is independently valid, including its own guest and local sources.
    let first_value = encode_pages(&page_set(vec![first.clone()]), &r, &mut b()).map_err(err)?;
    let second_value = encode_pages(&page_set(vec![second.clone()]), &r, &mut b()).map_err(err)?;
    decode_pages(&first_value, &r, &mut b()).map_err(err)?;
    decode_pages(&second_value, &r, &mut b()).map_err(err)?;
    assert!(encode_pages(&page_set(vec![first, second]), &r, &mut b()).is_err());
    // Assemble independently encoded envelopes. Only shared cross-page admission
    // can detect this conflict; neither document conflicts with itself.
    let mut joined = first_value;
    let mut second_value = second_value;
    let NdfValue::List(other) = &mut fields(&mut second_value)?[0] else {
        return Err("pages".into());
    };
    let page = other.pop().ok_or("second page")?;
    let NdfValue::List(pages) = &mut fields(&mut joined)?[0] else {
        return Err("pages".into());
    };
    pages.push(page);
    assert!(decode_pages(&joined, &r, &mut b()).is_err());
    Ok(())
}
