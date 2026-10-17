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

#[test]
fn pages_archive_resolver_identity_and_plan_routes_are_explicit() -> Result<(), String> {
    use nepl3_doc_core::pages::{self, PageIdentityProfile};
    let r = setup()?;
    let set = page_set(vec![document(&r)?]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let legacy = pages::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(legacy.identity_profile(), PageIdentityProfile::LegacyV1);
    let checked = pages::archive::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(checked.identity_profile(), PageIdentityProfile::ArchiveV1);
    assert_ne!(checked.plan().identity, legacy.plan().identity);
    assert_ne!(checked.document_digest(0), legacy.document_digest(0));
    assert_eq!(checked.plan().links, legacy.plan().links);
    let value = codec::pages::to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    // Independent direct digest over the exact archive envelope, not resolver internals.
    assert_eq!(
        checked.plan().identity,
        c.canonical_value_digest(pages::archive::SET_DOMAIN, &value, &mut b())
            .map_err(err)?
    );
    let standalone = codec::to_value(&set.pages[0].document, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        checked.document_digest(0),
        Some(
            c.canonical_value_digest(pages::archive::DOCUMENT_DOMAIN, &standalone.value, &mut b())
                .map_err(err)?
        )
    );
    let component = standalone.guest_components[0];
    let expected_guest = c
        .canonical_value_digest(
            pages::archive::GUEST_DOMAIN,
            &NdfValue::Bytes(component.0.to_vec()),
            &mut b(),
        )
        .map_err(err)?;
    assert!(checked.plan().remaining.iter().any(|item| matches!(&item.requirement,
        nepl3_doc_core::prepare::DocRequirement::Foreign{guest_digest,..} if *guest_digest==expected_guest)));
    let plan = checked.into_plan();
    let portable_plan =
        codec::pages::plan_to_value(&plan, &set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        codec::pages::plan_from_value(&portable_plan, &set, &r, &mut c, &mut b()).map_err(err)?,
        plan
    );
    assert!(portable::pages::plan_from_value(&portable_plan, &set, &r, &mut c, &mut b()).is_err());
    let old =
        portable::pages::plan_to_value(legacy.plan(), &set, &r, &mut c, &mut b()).map_err(err)?;
    assert!(codec::pages::plan_from_value(&old, &set, &r, &mut c, &mut b()).is_err());
    assert!(codec::pages::plan_to_value(legacy.plan(), &set, &r, &mut c, &mut b()).is_err());
    Ok(())
}

#[test]
fn pages_archive_resolver_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = setup()?;
    let doc = page_set(vec![document(&r)?]);
    for alternate in [false, true] {
        let run = |budget:&mut Budget| -> Result<(),nepl3_doc_core::pages::PageError<'_,nepl3_wire::WireError>> {
            let empty=SourceStore::default();let mut admission=SourceAdmission::default();
            let mut c=FoundationCodec::new(&r,&empty,&mut admission).map_err(PortableError::Foundation)?;
            if alternate {
                let mut c=super::default::DefaultCodec::new(c);
                nepl3_doc_core::pages::archive::resolve(&doc,&r,&mut c,budget).map(|_|())
            } else { nepl3_doc_core::pages::archive::resolve(&doc,&r,&mut c,budget).map(|_|()) }
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
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            let retained = budget.usage();
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(nepl3_doc_core::pages::PageError::Stopped(
                StopReason::Cancelled
            ))
        );
    }
    Ok(())
}

use nepl3_doc_core::pages::{PageDocument, PageRegistration, PageSet};
fn link_document(target: LinkTarget) -> DocumentSyntax {
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "文書".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(5)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6), InlineRef(7)],
        },
        DocKind::Anchor {
            id: "導入".into(),
            label: InlineRef(2),
        },
        DocKind::Link {
            target,
            label: InlineRef(2),
        },
    ];
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(0)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: Default::default(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
fn linked_set() -> PageSet {
    PageSet {
        files: vec![],
        pages: vec![
            PageDocument {
                registration: PageRegistration {
                    id: "first".into(),
                    source: "doc/first.nepld".into(),
                    route: "docs/first/index.html".into(),
                },
                document: link_document(LinkTarget::Page {
                    page: "second".into(),
                    fragment: Some("導入".into()),
                }),
            },
            PageDocument {
                registration: PageRegistration {
                    id: "second".into(),
                    source: "doc/next/second.nepld".into(),
                    route: "docs/second/index.html".into(),
                },
                document: link_document(LinkTarget::Relative {
                    path: ".././first.nepld".into(),
                    fragment: Some("導入".into()),
                }),
            },
        ],
    }
}

#[test]
fn pages_archive_links_helpers_and_tampered_plan_are_checked() -> Result<(), String> {
    use nepl3_doc_core::pages::{self, FileBytes, PageFile, PageProjectionError};
    let r = setup()?;
    let mut set = linked_set();
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut budget = b();
    let checked = pages::archive::resolve(&set, &r, &mut c, &mut budget).map_err(err)?;
    let legacy = pages::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(checked.plan().links.len(), 2);
    assert_eq!(checked.plan().links, legacy.plan().links);
    checked.document_structure(0, &mut budget).map_err(err)?;
    let text = checked
        .project_unresolved_text(
            0,
            SentenceRef(1),
            nepl3_doc_core::text::AnnotationPolicy::BaseOnly,
            &mut budget,
        )
        .map_err(err)?;
    assert_eq!(
        text.outcome,
        nepl3_doc_core::text::PlainTextOutcome::Complete {
            text: "文書".into()
        }
    );
    let mut wrong = Budget::new(Limits {
        work: 1,
        ..budget.limits()
    });
    assert!(matches!(
        checked.document_structure(0, &mut wrong),
        Err(PageProjectionError::Stopped(_))
    ));
    let mut wrong = Budget::new(Limits {
        work: budget.limits().work + 1,
        ..budget.limits()
    });
    assert!(matches!(
        checked.document_structure(0, &mut wrong),
        Err(PageProjectionError::ScopeMismatch)
    ));
    assert!(matches!(
        budget.with_depth(|b| checked.document_structure(0, b).map(|_| ())),
        Err(PageProjectionError::ScopeMismatch)
    ));
    let mut plan = checked.into_plan();
    plan.links.swap(0, 1);
    assert!(codec::pages::plan_to_value(&plan, &set, &r, &mut c, &mut b()).is_err());
    let mut outer = b();
    outer
        .with_depth_at_least(4, |b| {
            let checked = pages::archive::resolve(&set, &r, &mut c, b)?;
            checked
                .document_structure(0, b)
                .map_err(|_| pages::PageError::Boundary(PortableError::Shape))?;
            Ok::<_, pages::PageError<'_, nepl3_wire::WireError>>(())
        })
        .map_err(err)?;
    set.files.push(PageFile {
        registration: PageRegistration {
            id: "file".into(),
            source: "doc/pic.svg".into(),
            route: "img/pic.svg".into(),
        },
        content: FileBytes(vec![1, 2]),
    });
    set.pages[0].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::Relative {
            path: "pic.svg".into(),
            fragment: None,
        },
        label: InlineRef(2),
    };
    let checked = pages::archive::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        checked.plan().links[0].target,
        pages::PageDestination::File { index: 0 }
    );
    set.pages[0].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::Page {
            page: "second".into(),
            fragment: Some("missing".into()),
        },
        label: InlineRef(2),
    };
    assert!(matches!(
        pages::archive::resolve(&set, &r, &mut c, &mut b()),
        Err(pages::PageError::MissingFragment { .. })
    ));
    Ok(())
}

#[test]
fn pages_archive_plan_adapter_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = setup()?;
    let doc = page_set(vec![document(&r)?]);
    for alternate in [false, true] {
        let run = |budget:&mut Budget| -> Result<(),nepl3_doc_core::pages::PageError<'_,nepl3_wire::WireError>> {
            let empty=SourceStore::default();let mut admission=SourceAdmission::default();
            let c=FoundationCodec::new(&r,&empty,&mut admission).map_err(PortableError::Foundation)?;
            let mut c=super::default::DefaultCodec::new(c);
            let plan=nepl3_doc_core::pages::archive::resolve(&doc,&r,&mut c,budget)?.into_plan();
            let value=codec::pages::plan_to_value(&plan,&doc,&r,&mut c,budget)?;
            if alternate {codec::pages::plan_from_value(&value,&doc,&r,&mut c,budget)?;}
            Ok(())
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
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            let retained = budget.usage();
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(nepl3_doc_core::pages::PageError::Stopped(
                StopReason::Cancelled
            ))
        );
    }
    Ok(())
}

#[test]
fn legacy_resolver_retains_pre_refactor_identity_and_usage() -> Result<(), String> {
    let r = setup()?;
    let set = page_set(vec![document(&r)?]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut budget = b();
    let plan = nepl3_doc_core::pages::resolve(&set, &r, &mut c, &mut budget)
        .map_err(err)?
        .into_plan();
    // Measured independently on pre-refactor bd5220de with the identical fixture
    // and fresh codec/admission. These constants protect compatibility, not new
    // algorithm correctness (covered by archive digest oracles above).
    let u = budget.usage();
    assert_eq!(
        (
            u.source_bytes,
            u.depth,
            u.nodes,
            u.output_bytes,
            u.diagnostics,
            u.events
        ),
        (21, 17, 453, 256, 0, 0)
    );
    #[cfg(target_has_atomic = "ptr")]
    assert_eq!(u.work, 35591);
    #[cfg(all(target_pointer_width = "64", target_has_atomic = "ptr"))]
    assert_eq!(u.allocation_units, 27254);
    assert_eq!(
        plan.identity.0,
        [
            217, 34, 86, 93, 27, 210, 96, 153, 180, 123, 231, 35, 144, 210, 135, 132, 151, 132, 80,
            75, 131, 111, 136, 148, 206, 221, 225, 142, 160, 176, 54, 230
        ]
    );
    assert_eq!(plan.links.len(), 0);
    assert_eq!(plan.remaining.len(), 1);
    assert!(
        matches!(&plan.remaining[0].requirement,nepl3_doc_core::prepare::DocRequirement::Foreign{guest_digest,..}
        if guest_digest.0==[225,252,72,62,147,114,222,11,155,47,35,217,162,40,83,53,131,190,85,252,181,114,74,203,181,90,26,165,155,84,58,158])
    );
    Ok(())
}

impl<C: nepl3_core::value_codec::commitment::FoundationGroupCommitmentCodec>
    nepl3_core::value_codec::commitment::FoundationGroupCommitmentCodec
    for super::default::DefaultCodec<C>
{
    fn compact_group_commitments_at_depths(
        &mut self,
        g: &nepl3_core::syntax::group::ForeignClosureGroup,
        d: &[u64],
        r: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<
        nepl3_core::value_codec::commitment::GroupCommitments,
        GroupEncodingError<Self::Error>,
    > {
        let mut value = self.0.compact_group_commitments_at_depths(g, d, r, b)?;
        match self.1 {
            super::default::ArchiveComponents::Exact => {}
            super::default::ArchiveComponents::Short => {
                value.entry_digests.pop();
            }
            super::default::ArchiveComponents::Long => value
                .entry_digests
                .push(nepl3_core::source::Digest([0; 32])),
        }
        Ok(value)
    }
}
#[test]
fn commitment_pages_bind_complete_private_preimages_without_archive_contracts() -> Result<(), String>
{
    use nepl3_core::value_codec::commitment::FoundationGroupCommitmentCodec;
    use nepl3_doc_core::pages::{self, PageIdentityProfile};
    let r = registry()?;
    assert!(r.selected("nepl3.doc-archive", 1).is_none());
    let set = page_set(vec![document(&r)?]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let checked = pages::compact::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(checked.identity_profile(), PageIdentityProfile::CompactV1);
    // Start with an independently encoded ordinary Doc value and replace only
    // the typed group's location. The private compact preimage is not transport.
    let mut skeleton =
        portable::to_value(&set.pages[0].document, &r, &mut c, &mut b()).map_err(err)?;
    let group = c
        .encode_foreign_group(
            &nepl3_core::syntax::group::ForeignClosureGroup {
                provenances: vec![],
                environments: vec![],
                entries: vec![],
            },
            &mut b(),
        )
        .map_err(err)?;
    fields(&mut fields(&mut fields(&mut skeleton)?[0])?[2])?[1] = group;
    let commits = c
        .compact_group_commitments_at_depths(
            &set.pages[0].document.value.embeds.group,
            &[0],
            &r,
            &mut b(),
        )
        .map_err(err)?;
    let doc_value = NdfValue::List(vec![
        skeleton,
        NdfValue::Bytes(commits.group_digest.0.to_vec()),
    ]);
    assert_eq!(
        checked.document_digest(0),
        Some(
            c.canonical_value_digest(pages::compact::DOCUMENT_DOMAIN, &doc_value, &mut b())
                .map_err(err)?
        )
    );
    let mut legacy = portable::pages::set_to_value(&set, &r, &mut c, &mut b()).map_err(err)?;
    let all = fields(&mut legacy)?;
    let files = all[1].clone();
    let NdfValue::List(pages) = &mut all[0] else {
        return Err("pages".into());
    };
    let registration = fields(&mut pages[0])?[0].clone();
    let preimage = NdfValue::List(vec![
        NdfValue::List(vec![NdfValue::List(vec![registration, doc_value])]),
        files,
    ]);
    assert_eq!(
        checked.plan().identity,
        c.canonical_value_digest(pages::compact::SET_DOMAIN, &preimage, &mut b())
            .map_err(err)?
    );
    let guest = c
        .canonical_value_digest(
            pages::compact::GUEST_DOMAIN,
            &NdfValue::Bytes(commits.entry_digests[0].0.to_vec()),
            &mut b(),
        )
        .map_err(err)?;
    assert!(checked.plan().remaining.iter().any(|p|matches!(&p.requirement,nepl3_doc_core::prepare::DocRequirement::Foreign{guest_digest,..} if *guest_digest==guest)));
    Ok(())
}
#[test]
fn commitment_pages_reject_archive_and_legacy_plans_and_bad_provider_counts() -> Result<(), String>
{
    use nepl3_doc_core::pages;
    let r = setup()?;
    let set = page_set(vec![document(&r)?]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let plan = pages::compact::resolve(&set, &r, &mut c, &mut b())
        .map_err(err)?
        .into_plan();
    let value =
        portable::commitment::plan_to_value(&plan, &set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        portable::commitment::plan_from_value(&value, &set, &r, &mut c, &mut b()).map_err(err)?,
        plan
    );
    assert!(portable::pages::plan_from_value(&value, &set, &r, &mut c, &mut b()).is_err());
    assert!(codec::pages::plan_from_value(&value, &set, &r, &mut c, &mut b()).is_err());
    for other in [
        pages::resolve(&set, &r, &mut c, &mut b())
            .map_err(err)?
            .into_plan(),
        pages::archive::resolve(&set, &r, &mut c, &mut b())
            .map_err(err)?
            .into_plan(),
    ] {
        assert!(portable::commitment::plan_to_value(&other, &set, &r, &mut c, &mut b()).is_err());
    }
    for mode in [
        super::default::ArchiveComponents::Short,
        super::default::ArchiveComponents::Long,
    ] {
        let mut admission = SourceAdmission::default();
        let inner = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let mut c = super::default::DefaultCodec::new(inner);
        c.1 = mode;
        assert!(pages::compact::resolve(&set, &r, &mut c, &mut b()).is_err());
    }
    Ok(())
}

#[test]
fn commitment_pages_resolver_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = registry()?;
    let doc = page_set(vec![document(&r)?]);
    for alternate in [false, true] {
        let run = |budget:&mut Budget| -> Result<(),nepl3_doc_core::pages::PageError<'_,nepl3_wire::WireError>> {
            let empty=SourceStore::default();let mut admission=SourceAdmission::default();
            let mut c=FoundationCodec::new(&r,&empty,&mut admission).map_err(PortableError::Foundation)?;
            if alternate {
                let mut c=super::default::DefaultCodec::new(c);
                nepl3_doc_core::pages::compact::resolve(&doc,&r,&mut c,budget).map(|_|())
            } else { nepl3_doc_core::pages::compact::resolve(&doc,&r,&mut c,budget).map(|_|()) }
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
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            let retained = budget.usage();
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(nepl3_doc_core::pages::PageError::Stopped(
                StopReason::Cancelled
            ))
        );
    }
    Ok(())
}
#[test]
fn commitment_pages_plan_adapter_budgets_are_exact_and_sticky() -> Result<(), String> {
    let r = registry()?;
    let doc = page_set(vec![document(&r)?]);
    for alternate in [false, true] {
        let run = |budget:&mut Budget| -> Result<(),nepl3_doc_core::pages::PageError<'_,nepl3_wire::WireError>> {
            let empty=SourceStore::default();let mut admission=SourceAdmission::default();
            let c=FoundationCodec::new(&r,&empty,&mut admission).map_err(PortableError::Foundation)?;
            let mut c=super::default::DefaultCodec::new(c);
            let plan=nepl3_doc_core::pages::compact::resolve(&doc,&r,&mut c,budget)?.into_plan();
            let value=portable::commitment::plan_to_value(&plan,&doc,&r,&mut c,budget)?;
            if alternate {portable::commitment::plan_from_value(&value,&doc,&r,&mut c,budget)?;}
            Ok(())
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
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            let retained = budget.usage();
            assert_eq!(
                run(&mut budget),
                Err(nepl3_doc_core::pages::PageError::Stopped(reason))
            );
            assert_eq!(budget.usage(), retained);
        }
        let mut cancelled = b();
        cancelled.cancel();
        assert_eq!(
            run(&mut cancelled),
            Err(nepl3_doc_core::pages::PageError::Stopped(
                StopReason::Cancelled
            ))
        );
    }
    Ok(())
}
#[test]
fn commitment_pages_links_helpers_and_tampered_plan_are_checked() -> Result<(), String> {
    use nepl3_doc_core::pages::{self, FileBytes, PageFile, PageProjectionError};
    let r = registry()?;
    let mut set = linked_set();
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut budget = b();
    let checked = pages::compact::resolve(&set, &r, &mut c, &mut budget).map_err(err)?;
    let legacy = pages::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(checked.plan().links.len(), 2);
    assert_eq!(checked.plan().links, legacy.plan().links);
    checked.document_structure(0, &mut budget).map_err(err)?;
    let text = checked
        .project_unresolved_text(
            0,
            SentenceRef(1),
            nepl3_doc_core::text::AnnotationPolicy::BaseOnly,
            &mut budget,
        )
        .map_err(err)?;
    assert_eq!(
        text.outcome,
        nepl3_doc_core::text::PlainTextOutcome::Complete {
            text: "文書".into()
        }
    );
    let mut wrong = Budget::new(Limits {
        work: 1,
        ..budget.limits()
    });
    assert!(matches!(
        checked.document_structure(0, &mut wrong),
        Err(PageProjectionError::Stopped(_))
    ));
    let mut wrong = Budget::new(Limits {
        work: budget.limits().work + 1,
        ..budget.limits()
    });
    assert!(matches!(
        checked.document_structure(0, &mut wrong),
        Err(PageProjectionError::ScopeMismatch)
    ));
    assert!(matches!(
        budget.with_depth(|b| checked.document_structure(0, b).map(|_| ())),
        Err(PageProjectionError::ScopeMismatch)
    ));
    let mut plan = checked.into_plan();
    plan.links.swap(0, 1);
    assert!(portable::commitment::plan_to_value(&plan, &set, &r, &mut c, &mut b()).is_err());
    let mut outer = b();
    outer
        .with_depth_at_least(4, |b| {
            let checked = pages::compact::resolve(&set, &r, &mut c, b)?;
            checked
                .document_structure(0, b)
                .map_err(|_| pages::PageError::Boundary(PortableError::Shape))?;
            Ok::<_, pages::PageError<'_, nepl3_wire::WireError>>(())
        })
        .map_err(err)?;
    set.files.push(PageFile {
        registration: PageRegistration {
            id: "file".into(),
            source: "doc/pic.svg".into(),
            route: "img/pic.svg".into(),
        },
        content: FileBytes(vec![1, 2]),
    });
    set.pages[0].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::Relative {
            path: "pic.svg".into(),
            fragment: None,
        },
        label: InlineRef(2),
    };
    let checked = pages::compact::resolve(&set, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        checked.plan().links[0].target,
        pages::PageDestination::File { index: 0 }
    );
    set.pages[0].document.value.nodes[7].kind = DocKind::Link {
        target: LinkTarget::Page {
            page: "second".into(),
            fragment: Some("missing".into()),
        },
        label: InlineRef(2),
    };
    assert!(matches!(
        pages::compact::resolve(&set, &r, &mut c, &mut b()),
        Err(pages::PageError::MissingFragment { .. })
    ));
    Ok(())
}

#[test]
fn commitment_pages_keep_ordered_document_and_guest_associations() -> Result<(), String> {
    let r = registry()?;
    let first = document(&r)?;
    let mut second = document(&r)?;
    second.value.nodes[2].kind = DocKind::Text {
        text: "second".into(),
    };
    second.value.embeds.group.entries[0]
        .syntax
        .bundle
        .sources
        .push(
            SourceSnapshot::new(
                SourceId("extra".into()),
                1,
                "memory:extra".into(),
                b"unused local".to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
    let run = |set: &PageSet| -> Result<(_, Vec<_>, Vec<_>), String> {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let checked =
            nepl3_doc_core::pages::compact::resolve(set, &r, &mut c, &mut b()).map_err(err)?;
        let docs = (0..set.pages.len())
            .map(|i| checked.document_digest(i as u64).ok_or("document"))
            .collect::<Result<Vec<_>, _>>()
            .map_err(str::to_owned)?;
        let guests = checked
            .plan()
            .remaining
            .iter()
            .filter_map(|p| match &p.requirement {
                nepl3_doc_core::prepare::DocRequirement::Foreign { guest_digest, .. } => {
                    Some((p.page, *guest_digest))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        Ok((checked.plan().identity, docs, guests))
    };
    let a = run(&page_set(vec![first.clone()]))?;
    let b = run(&page_set(vec![second.clone()]))?;
    let together = run(&page_set(vec![first.clone(), second.clone()]))?;
    assert_eq!(together.1, vec![a.1[0], b.1[0]]);
    assert_eq!(together.2, vec![(0, a.2[0].1), (1, b.2[0].1)]);
    let reversed = run(&page_set(vec![second, first]))?;
    assert_ne!(together.0, reversed.0);
    assert_eq!(reversed.1, vec![b.1[0], a.1[0]]);
    Ok(())
}
#[test]
fn commitment_pages_reject_cross_document_source_conflict() -> Result<(), String> {
    let r = registry()?;
    let mut first = document(&r)?;
    let mut second = document(&r)?;
    for (doc, text) in [
        (&mut first, b"first".as_slice()),
        (&mut second, b"second".as_slice()),
    ] {
        doc.sources.push(
            SourceSnapshot::new(
                SourceId("shared-doc".into()),
                1,
                "memory:shared".into(),
                text.to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
    }
    let run = |set: &PageSet| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        nepl3_doc_core::pages::compact::resolve(set, &r, &mut c, &mut b())
            .map(|_| ())
            .map_err(err)
    };
    run(&page_set(vec![first.clone()]))?;
    run(&page_set(vec![second.clone()]))?;
    assert!(run(&page_set(vec![first, second])).is_err());
    Ok(())
}

#[test]
fn commitment_pages_bind_files_sources_and_match_alternate_provider() -> Result<(), String> {
    use nepl3_doc_core::pages::{self, FileBytes, PageFile};
    let r = registry()?;
    let mut set = page_set(vec![document(&r)?]);
    set.files.push(PageFile {
        registration: PageRegistration {
            id: "file".into(),
            source: "asset.bin".into(),
            route: "asset.bin".into(),
        },
        content: FileBytes(vec![1, 2, 3]),
    });
    let run = |set: &PageSet, alternate: bool| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let inner = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let checked = if alternate {
            let mut c = super::default::DefaultCodec::new(inner);
            pages::compact::resolve(set, &r, &mut c, &mut b()).map_err(err)?
        } else {
            let mut c = inner;
            pages::compact::resolve(set, &r, &mut c, &mut b()).map_err(err)?
        };
        Ok::<_, String>((checked.plan().clone(), checked.document_digest(0)))
    };
    let original = run(&set, false)?;
    assert_eq!(original, run(&set, true)?);
    let mut changed = set.clone();
    changed.files[0].content.0[0] ^= 1;
    let bytes = run(&changed, false)?;
    assert_ne!(bytes.0.identity, original.0.identity);
    assert_eq!(bytes.1, original.1);
    let mut changed = set.clone();
    changed.files[0].registration.route = "other.bin".into();
    assert_ne!(run(&changed, false)?.0.identity, original.0.identity);
    let mut changed = set.clone();
    changed.pages[0].document.sources.push(
        SourceSnapshot::new(
            SourceId("unused-doc".into()),
            1,
            "memory:unused-doc".into(),
            b"declared".to_vec(),
            &mut b(),
        )
        .map_err(err)?,
    );
    assert_ne!(run(&changed, false)?.1, original.1);
    Ok(())
}
#[test]
fn commitment_pages_check_deepest_shared_guest_occurrence() -> Result<(), String> {
    use nepl3_core::value_codec::commitment::FoundationGroupCommitmentCodec;
    let r = registry()?;
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
    for kind in [
        DocKind::Sentence {
            inlines: vec![InlineRef(0), InlineRef(19)],
        },
        DocKind::Body { blocks: vec![] },
        DocKind::Article {
            language: "en".into(),
            title: SentenceRef(20),
            body: BodyRef(21),
        },
    ] {
        nodes.push(DocNode {
            kind,
            locations: vec![],
            origin: None,
            span: None,
        });
    }
    let doc = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(22)),
            nodes,
            embeds: embeds(EmbedKind::InlineMath, closure(&r)?),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let mut group_budget = b();
    c.compact_group_commitments_at_depths(&doc.value.embeds.group, &[0], &r, &mut group_budget)
        .map_err(err)?;
    let set = page_set(vec![doc]);
    let run = |budget: &mut Budget| {
        let mut admission = SourceAdmission::default();
        let mut c =
            FoundationCodec::new(&r, &empty, &mut admission).map_err(PortableError::Foundation)?;
        nepl3_doc_core::pages::compact::resolve(&set, &r, &mut c, budget).map(|_| ())
    };
    let mut measured = b();
    measured.with_depth_at_least(4, run).map_err(err)?;
    assert!(measured.usage().depth > group_budget.usage().depth);
    let mut short = Budget::new(Limits {
        depth: measured.usage().depth - 1,
        ..b().limits()
    });
    assert!(matches!(
        short.with_depth_at_least(4, run),
        Err(nepl3_doc_core::pages::PageError::Stopped(
            StopReason::DepthLimit
        ))
    ));
    assert_eq!(short.current_depth(), 0);
    Ok(())
}
