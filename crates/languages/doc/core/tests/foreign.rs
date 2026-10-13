use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{check::StructureError, model::*, portable};
use nepl3_wire::foundation::FoundationCodec;
fn err(v: impl core::fmt::Debug) -> String {
    format!("{v:?}")
}
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 100_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
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
fn closure(r: &SchemaRegistry) -> Result<ForeignClosure, String> {
    let source = SourceSnapshot::new(
        SourceId("guest".into()),
        1,
        "memory:guest".into(),
        b"opaque guest spelling".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let span = source.span(0, source.text().len() as u64).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = codec
        .environment_digest(&environment, &mut b())
        .map_err(err)?;
    let schema = r.selected("nepl3.doc", 1).ok_or("schema")?.clone();
    // This is deliberately only a common syntax graph, not a checked Article.
    // Code display cannot acquire a requirement to semantically lower this guest.
    let node = SyntaxNode {
        schema: schema.clone(),
        kind: "View:TextRun".into(),
        fields: vec![],
        head: Some(span.clone()),
        cover: Some(span.clone()),
        origin: OriginId(0),
        token: None,
    };
    Ok(ForeignClosure {
        syntax: ForeignSyntax {
            schema,
            category: "Article".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![source],
                nodes: vec![node],
                origins: vec![Origin::Direct(span)],
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 17, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 17,
            digest,
            value: environment,
        },
        owner_origins: vec![],
        owner_sources: vec![],
        owner_source_maps: vec![],
    })
}
fn embeds(kind: EmbedKind, closure: ForeignClosure) -> DocEmbeds {
    use nepl3_core::syntax::group::*;
    DocEmbeds {
        kinds: vec![kind],
        group: ForeignClosureGroup {
            provenances: vec![ForeignProvenance {
                origins: closure.owner_origins,
                sources: closure.owner_sources,
                source_maps: closure.owner_source_maps,
            }],
            environments: vec![ForeignOwnerEnvironment {
                provenance: nepl3_core::syntax::group::ProvenanceRef(0),
                environment: closure.owner_environment,
            }],
            entries: vec![ForeignGroupEntry {
                owner: nepl3_core::syntax::group::OwnerEnvironmentRef(0),
                syntax: closure.syntax,
            }],
        },
    }
}
#[test]
fn code_guest_stays_syntax_through_doc_structure_and_cbor() -> Result<(), String> {
    let r = registry()?;
    let doc = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Block(BlockRef(0)),
            nodes: vec![DocNode {
                locations: Vec::new(),
                kind: DocKind::Code {
                    syntax: EmbedRef(0),
                },
                origin: None,
                span: None,
            }],
            embeds: embeds(EmbedKind::Code, closure(&r)?),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    doc.validate_structure(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let value = portable::to_value(&doc, &r, &mut codec, &mut b()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let actual = portable::from_value(&value, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(actual, doc);
    assert_eq!(
        actual.value.embeds.group.entries[0].syntax.bundle.nodes[0].kind,
        "View:TextRun"
    );
    Ok(())
}
#[test]
fn shared_embed_validation_composes_its_deepest_doc_owner() -> Result<(), String> {
    let r = registry()?;
    let mut nodes = vec![DocNode {
        locations: Vec::new(),
        kind: DocKind::InlineMath {
            syntax: EmbedRef(0),
        },
        origin: None,
        span: None,
    }];
    for i in 1..20 {
        nodes.push(DocNode {
            locations: Vec::new(),
            kind: DocKind::Strong {
                inline: InlineRef(i - 1),
            },
            origin: None,
            span: None,
        });
    }
    nodes.push(DocNode {
        locations: Vec::new(),
        kind: DocKind::Sentence {
            inlines: vec![InlineRef(0), InlineRef(19)],
        },
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
    let mut full = b();
    doc.validate_structure(&r, &mut full, &mut SourceAdmission::default())
        .map_err(err)?;
    assert!(full.usage().depth > 21);
    // Preparation's retained proof must still validate the deepest semantic
    // occurrence before encoding, including standalone Inline/Sentence roots.
    for inline in [false, true] {
        let mut input = doc.clone();
        if inline {
            input.value.nodes.pop();
            input.value.root = DocRoot::Inline(InlineRef(19));
        }
        let inspect = if inline {
            nepl3_doc_core::prepare::inspect_inline
        } else {
            nepl3_doc_core::prepare::inspect_sentence
        };
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        inspect(&input, &r, &mut codec, &mut b()).map_err(err)?;
        let mut limits = b().limits();
        limits.depth = if inline { 20 } else { 21 };
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            inspect(&input, &r, &mut codec, &mut stopped),
            Err(nepl3_doc_core::prepare::PreparationError::Stopped(
                StopReason::DepthLimit
            ))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::DepthLimit));
    }
    let mut limits = b().limits();
    limits.depth = 21;
    assert!(matches!(
        doc.validate_structure(
            &r,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        ),
        Err(StructureError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}

fn mapping_pair(name: &str) -> Result<(Vec<SourceSnapshot>, nepl3_core::origin::Mapping), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let sources = ["a", "b"]
        .into_iter()
        .map(|suffix| {
            SourceSnapshot::new(
                SourceId(format!("{name}-{suffix}")),
                1,
                format!("memory:{name}-{suffix}"),
                b"mapped bytes".to_vec(),
                &mut b(),
            )
            .map_err(err)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mapping = Mapping {
        source: sources[0].span(0, 12).map_err(err)?,
        target: sources[1].span(0, 12).map_err(err)?,
        kind: MappingKind::Exact,
    };
    Ok((sources, mapping))
}

fn mapped_article(
    guest: ForeignClosure,
    sources: Vec<SourceSnapshot>,
    maps: Vec<nepl3_core::origin::Mapping>,
) -> DocumentSyntax {
    let kinds = [
        DocKind::Article {
            language: "en".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "Mapped guest".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4)],
        },
        DocKind::Code {
            syntax: EmbedRef(0),
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
            embeds: embeds(EmbedKind::Code, guest),
        },
        sources,
        source_maps: maps,
        origins: vec![],
        views: vec![],
    }
}

#[test]
fn page_guest_digest_uses_owned_maps_not_parent_scope() -> Result<(), String> {
    use nepl3_core::{origin::MappingKind, source::Digest, value::NdfValue};
    use nepl3_doc_core::{pages::*, prepare};
    let r = registry()?;
    let (owner_sources, owner_map) = mapping_pair("owner")?;
    let (document_sources, document_map) = mapping_pair("document")?;
    let (external_sources, external_map) = mapping_pair("external")?;
    let mut guest = closure(&r)?;
    guest.owner_sources = owner_sources.clone();
    guest.owner_source_maps = vec![owner_map];
    let mut declared_sources = document_sources;
    // These ambient declarations must not repair missing closure-owned data.
    declared_sources.extend(owner_sources);
    let first = mapped_article(guest, declared_sources, vec![document_map]);
    let mut second = first.clone();
    second.source_maps[0].kind = MappingKind::Transformed;
    let set = PageSet {
        pages: [first, second]
            .into_iter()
            .enumerate()
            .map(|(i, document)| PageDocument {
                registration: PageRegistration {
                    id: format!("page-{i}"),
                    source: format!("page-{i}.nepld"),
                    route: format!("page-{i}.md"),
                },
                document,
            })
            .collect(),
        files: vec![],
    };
    let mut store = SourceStore::default();
    for source in external_sources
        .into_iter()
        .chain(set.pages[0].document.sources.iter().cloned())
    {
        store.insert_with_budget(source, &mut b()).map_err(err)?;
    }
    let field = |v: &NdfValue, index: usize| -> Result<NdfValue, String> {
        let NdfValue::Record(record) = v else {
            return Err("expected record".into());
        };
        record
            .fields
            .get(index)
            .cloned()
            .ok_or_else(|| "missing field".into())
    };
    let run = |input: &PageSet, kind| -> Result<(_, _, _), String> {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut map = external_map.clone();
        map.kind = kind;
        let maps = [map];
        let mut scoped = codec.scoped_with_mappings(&store, &maps);
        let encoded =
            portable::pages::set_to_value(input, &r, &mut scoped, &mut b()).map_err(err)?;
        let NdfValue::List(ref encoded_pages) = field(&encoded, 0)? else {
            return Err("page list".into());
        };
        let mut standalone = Vec::new();
        let mut guest_values = Vec::new();
        for (index, page) in input.pages.iter().enumerate() {
            let plan = prepare::inspect(&page.document, &r, &mut scoped, &mut b()).map_err(err)?;
            let group = scoped
                .encode_foreign_group(&page.document.value.embeds.group, &mut b())
                .map_err(err)?;
            // Independent component/framing oracle from the encoded group, not
            // foreign_group_entry_digests or a consuming identity helper.
            let NdfValue::List(ref provenances) = field(&group, 0)? else {
                return Err("provenances".into());
            };
            let NdfValue::List(ref environments) = field(&group, 1)? else {
                return Err("environments".into());
            };
            let NdfValue::List(ref entries) = field(&group, 2)? else {
                return Err("entries".into());
            };
            let mut parts = Vec::new();
            for (domain, component) in [
                (
                    b"NEPL3.Foreign.Provenance.v1\0".as_slice(),
                    provenances[0].clone(),
                ),
                (
                    b"NEPL3.Foreign.Environment.v1\0".as_slice(),
                    field(&environments[0], 1)?,
                ),
                (
                    b"NEPL3.Foreign.Syntax.v1\0".as_slice(),
                    field(&entries[0], 1)?,
                ),
            ] {
                let mut bytes = domain.to_vec();
                bytes.extend(nepl3_wire::encode(&component, &mut b()).map_err(err)?);
                parts.push(NdfValue::Bytes(Digest::of(&bytes).0.to_vec()));
            }
            let mut bytes = b"NEPL3.Foreign.Entry.v1\0".to_vec();
            bytes.extend(nepl3_wire::encode(&NdfValue::List(parts), &mut b()).map_err(err)?);
            let value = NdfValue::Bytes(Digest::of(&bytes).0.to_vec());
            let mut bytes = prepare::GUEST_DOMAIN.to_vec();
            bytes.extend(nepl3_wire::encode(&value, &mut b()).map_err(err)?);
            let digest = Digest::of(&bytes);
            assert_eq!(
                plan.requirements,
                vec![prepare::DocRequirement::Foreign {
                    embed: EmbedRef(0),
                    kind: EmbedKind::Code,
                    guest_digest: digest
                }]
            );
            let document = field(&encoded_pages[index], 1)?;
            let doc_value = field(&document, 0)?;
            let encoded_embeds = field(&doc_value, 2)?;
            assert_eq!(field(&encoded_embeds, 1)?, group);
            standalone.push(plan);
            guest_values.push(value);
        }
        let checked = resolve(input, &r, &mut scoped, &mut b()).map_err(err)?;
        for (index, plan) in standalone.iter().enumerate() {
            assert_eq!(
                checked.document_digest(index as u64),
                Some(plan.document_digest)
            );
            assert_eq!(
                checked.plan().remaining[index],
                PageRequirement {
                    page: index as u64,
                    requirement: plan.requirements[0].clone()
                }
            );
        }
        assert_eq!(checked.plan().remaining.len(), standalone.len());
        let plan = checked.into_plan();
        // Portable reconstruction starts with an empty ambient store and ledger.
        let empty = SourceStore::default();
        let mut fresh_admission = SourceAdmission::default();
        let mut fresh = FoundationCodec::new(&r, &empty, &mut fresh_admission).map_err(err)?;
        let received =
            portable::pages::set_from_value(&encoded, &r, &mut fresh, &mut b()).map_err(err)?;
        assert_eq!(&received, input);
        assert_eq!(
            resolve(&received, &r, &mut fresh, &mut b())
                .map_err(err)?
                .into_plan(),
            plan
        );
        Ok((plan, standalone, guest_values))
    };
    let baseline = run(&set, MappingKind::Exact)?;
    assert_eq!(baseline, run(&set, MappingKind::Transformed)?);
    assert_eq!(baseline.1[0].requirements, baseline.1[1].requirements);
    assert_eq!(baseline.2[0], baseline.2[1]);
    assert_ne!(baseline.1[0].document_digest, baseline.1[1].document_digest);
    let mut changed = set.clone();
    changed.pages[0].document.source_maps[0].kind = MappingKind::Transformed;
    let parent_change = run(&changed, MappingKind::Exact)?;
    assert_eq!(parent_change.2, baseline.2);
    assert_ne!(parent_change.0.identity, baseline.0.identity);
    changed = set.clone();
    changed.pages[0].document.value.embeds.group.provenances[0].source_maps[0].kind =
        MappingKind::Transformed;
    let owner_change = run(&changed, MappingKind::Exact)?;
    assert_ne!(owner_change.2[0], baseline.2[0]);
    assert_ne!(owner_change.1[0].requirements, baseline.1[0].requirements);
    assert_ne!(
        owner_change.1[0].document_digest,
        baseline.1[0].document_digest
    );
    assert_ne!(owner_change.0.identity, baseline.0.identity);
    // The missing endpoint still exists in both the document and codec stores.
    // Rejection must come from the closure's own authority, not lack of ambient data.
    changed.pages[0].document.value.embeds.group.provenances[0]
        .sources
        .remove(1);
    for batched in [false, true] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let maps = [external_map.clone()];
        let mut scoped = codec.scoped_with_mappings(&store, &maps);
        let mut budget = b();
        if batched {
            assert!(resolve(&changed, &r, &mut scoped, &mut budget).is_err());
        } else {
            assert!(
                prepare::inspect(&changed.pages[0].document, &r, &mut scoped, &mut budget).is_err()
            );
        }
        assert_eq!(budget.poll(), Ok(()));
    }
    Ok(())
}

#[test]
fn doc_group_roles_hidden_owners_and_shared_fragment_are_checked() -> Result<(), String> {
    use nepl3_core::syntax::group::*;
    let r = registry()?;
    let mut guest = closure(&r)?;
    guest.owner_origins.push(Origin::Synthetic {
        reason: "unreferenced owner origin retained".into(),
        anchor: None,
    });
    let mut slots = embeds(EmbedKind::InlineMath, guest);
    let mut environment = slots.group.environments[0].clone();
    environment.environment.id = 29;
    slots.group.environments.push(environment);
    let mut entry = slots.group.entries[0].clone();
    entry.owner = OwnerEnvironmentRef(1);
    entry.syntax.environment.id = 29;
    slots.group.entries.push(entry);
    slots.group.entries.push(slots.group.entries[0].clone());
    slots
        .kinds
        .extend([EmbedKind::InlineMath, EmbedKind::InlineMath]);
    let kinds = [
        DocKind::InlineMath {
            syntax: EmbedRef(0),
        },
        DocKind::InlineMath {
            syntax: EmbedRef(1),
        },
        DocKind::InlineMath {
            syntax: EmbedRef(2),
        },
        DocKind::Concat {
            inlines: vec![InlineRef(1), InlineRef(0), InlineRef(1)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(3), InlineRef(2)],
        },
    ];
    let document = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(4)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: slots,
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    document
        .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let mut wrong_count = document.clone();
    wrong_count.value.embeds.kinds.pop();
    assert_eq!(
        wrong_count.value.validate_shape(&mut b()).err(),
        Some(nepl3_doc_core::check::ShapeError::EmbedCount)
    );
    let mut wrong_role = document.clone();
    wrong_role.value.embeds.kinds[0] = EmbedKind::Code;
    assert!(
        wrong_role
            .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
            .is_err()
    );
    let fragment = document
        .fragment(
            DocRoot::Inline(InlineRef(3)),
            &r,
            &mut b(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
    assert_eq!(fragment.value.embeds.len(), 2);
    assert_eq!(
        fragment.value.embeds.group.provenances,
        document.value.embeds.group.provenances
    );
    assert_eq!(fragment.value.embeds.group.environments.len(), 2);
    for (new, old) in [1usize, 0].into_iter().enumerate() {
        assert_eq!(
            fragment.value.embeds.get(new),
            document.value.embeds.get(old)
        );
    }
    let DocRoot::Inline(root) = fragment.value.root else {
        return Err("inline".into());
    };
    let DocKind::Concat { ref inlines } = fragment.value.nodes[root.0 as usize].kind else {
        return Err("concat".into());
    };
    assert_eq!(inlines[0], inlines[2]);
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let raw = portable::to_value(&fragment, &r, &mut codec, &mut b()).map_err(err)?;
    let mut fresh_admission = SourceAdmission::default();
    let mut fresh = FoundationCodec::new(&r, &empty, &mut fresh_admission).map_err(err)?;
    assert_eq!(
        portable::from_value(&raw, &r, &mut fresh, &mut b()).map_err(err)?,
        fragment
    );
    let mut hidden = document.clone();
    hidden.value.root = DocRoot::Inline(InlineRef(0));
    hidden.value.nodes.truncate(1);
    hidden.value.nodes[0].kind = DocKind::Text {
        text: "empty foreign closure".into(),
    };
    hidden.value.embeds.kinds.clear();
    hidden.value.embeds.group.entries.clear();
    assert!(!hidden.value.embeds.is_empty());
    assert!(
        hidden
            .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
            .is_err()
    );
    hidden.value.embeds.group.environments.clear();
    assert!(!hidden.value.embeds.is_empty());
    assert!(
        hidden
            .validate_structure(&r, &mut b(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}
