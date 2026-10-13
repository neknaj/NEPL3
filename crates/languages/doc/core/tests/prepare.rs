use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_doc_core::{
    model::*,
    portable,
    prepare::{self, DocRequirement, PreparationError},
};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        allocation_units: 100_000_000,
        depth: 10000,
        nodes: 1_000_000,
        output_bytes: 1_000_000,
        ..Limits::default()
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
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn document() -> DocumentSyntax {
    let asset = AssetRef {
        id: "図".into(),
        digest: Some(Digest::of(b"image input")),
    };
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text { text: "例".into() },
        DocKind::Body {
            blocks: vec![BlockRef(4), BlockRef(9)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(5)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6), InlineRef(7)],
        },
        DocKind::Link {
            target: LinkTarget::Page {
                page: "guide".into(),
                fragment: Some("introduction".into()),
            },
            label: InlineRef(2),
        },
        DocKind::InlineImage {
            asset: asset.clone(),
            alt: SentenceRef(8),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Image {
            asset,
            alt: SentenceRef(8),
            caption: None,
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
#[test]
fn namespace_plans_preserve_order_and_reserve_before_processing() -> Result<(), String> {
    use nepl3_doc_core::labels::namespace;
    let r = registry()?;
    let store = SourceStore::default();
    for count in [0, 1, 16, 64] {
        let documents: Vec<_> = (0..count)
            .map(|index| {
                let mut d = document();
                d.value.nodes[2].kind = DocKind::Text {
                    text: format!("title {index}"),
                };
                d
            })
            .collect();
        let mut admission = SourceAdmission::default();
        let members = documents
            .iter()
            .map(|d| namespace::inspect(d, &r, &mut b(), &mut admission).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let refs: Vec<_> = members.iter().collect();
        let namespace = namespace::resolve(&refs, &mut b()).map_err(err)?;
        let mut c = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        let mut measured = b();
        let plans =
            prepare::inspect_namespace(&namespace, &r, &mut c, &mut measured).map_err(err)?;
        let expected = documents
            .iter()
            .map(|d| prepare::inspect(d, &r, &mut c, &mut b()).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(plans, expected);
        // One shared namespace, distinct documents: digest order must follow
        // member order, including an empty namespace. No sorting of plans.
        assert!(
            plans
                .windows(2)
                .all(|pair| pair[0].document_digest != pair[1].document_digest)
        );
        if count == 0 {
            continue;
        }
        for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => limits.work = measured.usage().work,
                StopReason::AllocationLimit => {
                    limits.allocation_units = measured.usage().allocation_units
                }
                _ => return Err("fixed resource cases".into()),
            }
            assert_eq!(
                prepare::inspect_namespace(&namespace, &r, &mut c, &mut Budget::new(limits))
                    .map_err(err)?,
                plans
            );
            match reason {
                StopReason::WorkLimit => limits.work -= 1,
                StopReason::AllocationLimit => limits.allocation_units -= 1,
                _ => return Err("fixed resource cases".into()),
            }
            let mut stopped = Budget::new(limits);
            assert!(
                matches!(prepare::inspect_namespace(&namespace, &r, &mut c, &mut stopped),
                Err(PreparationError::Stopped(actual)) if actual == reason)
            );
            assert_eq!(stopped.poll(), Err(reason));
        }
        let bytes = (count * core::mem::size_of::<prepare::DocPreparationPlan>()) as u64;
        let mut limits = b().limits();
        limits.allocation_units = bytes - 1;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            prepare::inspect_namespace(&namespace, &r, &mut c, &mut stopped),
            Err(PreparationError::Stopped(StopReason::AllocationLimit))
        ));
        assert_eq!(
            stopped.usage().work,
            0,
            "reject reservation before processing members"
        );
        assert_eq!(stopped.poll(), Err(StopReason::AllocationLimit));
    }
    Ok(())
}

#[test]
fn preparation_discovers_distinct_placements_without_loading_assets() -> Result<(), String> {
    let r = registry()?;
    let d = document();
    let copy = d.clone();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let actual = prepare::inspect(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        actual.requirements,
        vec![
            DocRequirement::Link {
                node: 6,
                target: LinkTarget::Page {
                    page: "guide".into(),
                    fragment: Some("introduction".into())
                }
            },
            DocRequirement::Asset {
                node: 7,
                asset: AssetRef {
                    id: "図".into(),
                    digest: Some(Digest::of(b"image input"))
                }
            },
            DocRequirement::Asset {
                node: 9,
                asset: AssetRef {
                    id: "図".into(),
                    digest: Some(Digest::of(b"image input"))
                }
            }
        ]
    );
    // An independent direct domain-prefix + canonical CBOR concatenation:
    // this expects the full owned document, not just IDs or external names.
    let wire = nepl3_wire::encode(
        &portable::to_value(&d, &r, &mut c, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let mut bytes = b"NEPL3.Doc.Prepare.Document.v1\0".to_vec();
    bytes.extend(wire);
    assert_eq!(actual.document_digest, Digest::of(&bytes));
    assert_eq!(d, copy);
    Ok(())
}
#[test]
fn portable_plan_recomputes_requirements_and_rejects_stale_or_incomplete_data() -> Result<(), String>
{
    let r = registry()?;
    let d = document();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let plan = prepare::inspect(&d, &r, &mut c, &mut b()).map_err(err)?;
    let value = portable::prepare::plan_to_value(&plan, &d, &r, &mut c, &mut b()).map_err(err)?;
    let doc_bytes = nepl3_wire::encode(
        &portable::to_value(&d, &r, &mut c, &mut b()).map_err(err)?,
        &mut b(),
    )
    .map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let received = portable::from_value(
        &nepl3_wire::decode(&doc_bytes, &mut b()).map_err(err)?,
        &r,
        &mut c,
        &mut b(),
    )
    .map_err(err)?;
    let input = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    assert_eq!(
        portable::prepare::plan_from_value(&input, &received, &r, &mut c, &mut b()).map_err(err)?,
        plan
    );
    let mut stale = received.clone();
    if let DocKind::Text { text } = &mut stale.value.nodes[2].kind {
        *text = "更新".into();
    }
    assert!(portable::prepare::plan_from_value(&input, &stale, &r, &mut c, &mut b()).is_err());
    let mut incomplete = input;
    let NdfValue::Record(record) = &mut incomplete else {
        return Err("plan".into());
    };
    record.fields[1] = NdfValue::List(vec![]);
    assert!(
        portable::prepare::plan_from_value(&incomplete, &received, &r, &mut c, &mut b()).is_err()
    );
    Ok(())
}
#[test]
fn inspection_requires_article_labels_and_preserves_sticky_stops() -> Result<(), String> {
    let r = registry()?;
    let mut d = document();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    d.value.nodes[6].kind = DocKind::Reference {
        target: "missing".into(),
        label: InlineRef(2),
    };
    assert!(matches!(
        prepare::inspect(&d, &r, &mut c, &mut b()),
        Err(PreparationError::Label(_))
    ));
    for reason in [
        StopReason::WorkLimit,
        StopReason::DepthLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::DepthLimit => limits.depth = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            _ => {}
        }
        let mut b = Budget::new(limits);
        if reason == StopReason::Cancelled {
            b.cancel();
        }
        assert_eq!(
            prepare::inspect(&d, &r, &mut c, &mut b),
            Err(PreparationError::Stopped(reason))
        );
        assert_eq!(
            prepare::inspect(&d, &r, &mut c, &mut b),
            Err(PreparationError::Stopped(reason))
        );
    }
    Ok(())
}

fn plain_document(count: usize, root_kind: u8) -> DocumentSyntax {
    let (root, mut kinds) = match root_kind {
        0 => (
            DocRoot::Article(ArticleRef(0)),
            vec![
                DocKind::Article {
                    language: "ja".into(),
                    title: SentenceRef(1),
                    body: BodyRef(3),
                },
                DocKind::Sentence {
                    inlines: vec![InlineRef(2)],
                },
                DocKind::Text {
                    text: "title".into(),
                },
                DocKind::Body {
                    blocks: (0..count).map(|i| BlockRef((4 + 3 * i) as u64)).collect(),
                },
            ],
        ),
        1 => (
            DocRoot::Sentence(SentenceRef(0)),
            vec![DocKind::Sentence {
                inlines: (1..=count).map(|i| InlineRef(i as u64)).collect(),
            }],
        ),
        _ => (
            DocRoot::Inline(InlineRef(0)),
            vec![DocKind::Concat {
                inlines: (1..=count).map(|i| InlineRef(i as u64)).collect(),
            }],
        ),
    };
    for i in 0..count {
        if root_kind == 0 {
            let next = kinds.len() as u64;
            kinds.push(DocKind::Paragraph {
                items: vec![FlowRef(next + 1)],
            });
            kinds.push(DocKind::Sentence {
                inlines: vec![InlineRef(next + 2)],
            });
        }
        kinds.push(DocKind::Text {
            text: format!("paragraph {i}"),
        });
    }
    DocumentSyntax {
        value: DocValue {
            root,
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

#[test]
fn preparation_reuses_structure_for_all_roots_with_exact_budgets() -> Result<(), String> {
    use nepl3_core::value_codec::FoundationValueCodec;
    use nepl3_doc_core::labels;
    let r = registry()?;
    let store = SourceStore::default();
    for root in 0..3 {
        for count in [8, 32] {
            let d = plain_document(count, root);
            let inspect = match root {
                0 => prepare::inspect,
                1 => prepare::inspect_sentence,
                _ => prepare::inspect_inline,
            };
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
            let mut measured = b();
            let actual = inspect(&d, &r, &mut c, &mut measured).map_err(err)?;
            assert!(actual.requirements.is_empty());
            // Deliberately repeat the former public label + encoder validation.
            // No sources/guests means admission history cannot change its cost.
            let mut repeated = b();
            match root {
                0 => {
                    labels::check(&d, &r, &mut repeated, c.source_admission()).map_err(err)?;
                }
                1 => {
                    labels::check_sentence(&d, &r, &mut repeated, c.source_admission())
                        .map_err(err)?;
                }
                _ => {
                    labels::check_inline(&d, &r, &mut repeated, c.source_admission())
                        .map_err(err)?;
                }
            }
            let value = portable::to_value(&d, &r, &mut c, &mut repeated).map_err(err)?;
            let expected = c
                .canonical_value_digest(prepare::DOCUMENT_DOMAIN, &value, &mut repeated)
                .map_err(err)?;
            assert_eq!(actual.document_digest, expected);
            // These fixtures contain no external requirements. Mirror the
            // discovery scan and empty-group identity work of the old path.
            repeated
                .charge(
                    nepl3_core::budget::Resource::Work,
                    d.value.nodes.len() as u64,
                )
                .map_err(err)?;
            assert!(
                c.foreign_group_entry_digests(&d.value.embeds.group, &mut repeated)
                    .map_err(err)?
                    .is_empty()
            );
            assert!(measured.usage().work < repeated.usage().work);
            assert!(measured.usage().allocation_units < repeated.usage().allocation_units);
            for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
                let mut limits = b().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = measured.usage().work,
                    _ => limits.allocation_units = measured.usage().allocation_units,
                }
                assert_eq!(
                    inspect(&d, &r, &mut c, &mut Budget::new(limits)).map_err(err)?,
                    actual
                );
                match reason {
                    StopReason::WorkLimit => limits.work -= 1,
                    _ => limits.allocation_units -= 1,
                }
                let mut stopped = Budget::new(limits);
                assert_eq!(
                    inspect(&d, &r, &mut c, &mut stopped),
                    Err(PreparationError::Stopped(reason))
                );
                assert_eq!(stopped.poll(), Err(reason));
                assert_eq!(
                    inspect(&d, &r, &mut c, &mut stopped),
                    Err(PreparationError::Stopped(reason))
                );
            }
            let mut cancelled = b();
            cancelled.cancel();
            assert_eq!(
                inspect(&d, &r, &mut c, &mut cancelled),
                Err(PreparationError::Stopped(StopReason::Cancelled))
            );
            let mut limits = b().limits();
            limits.depth = 0;
            assert_eq!(
                inspect(&d, &r, &mut c, &mut Budget::new(limits)),
                Err(PreparationError::Stopped(StopReason::DepthLimit))
            );
            let mut malformed = d.clone();
            malformed.value.nodes.clear();
            assert!(matches!(
                inspect(&malformed, &r, &mut c, &mut b()),
                Err(PreparationError::Label(labels::LabelError::Structure(_)))
            ));
        }
    }
    Ok(())
}

#[test]
fn preparation_keeps_fresh_source_admission_and_local_label_errors() -> Result<(), String> {
    use nepl3_core::source::{SourceId, SourceSnapshot};
    use nepl3_doc_core::labels::LabelError;
    let r = registry()?;
    let store = SourceStore::default();
    for root in 0..3 {
        let inspect = match root {
            0 => prepare::inspect,
            1 => prepare::inspect_sentence,
            _ => prepare::inspect_inline,
        };
        let mut d = plain_document(8, root);
        d.sources.push(
            SourceSnapshot::new(
                SourceId("owned".into()),
                1,
                "memory:owned".into(),
                "原文".as_bytes().to_vec(),
                &mut b(),
            )
            .map_err(err)?,
        );
        let run = |input: &DocumentSyntax, budget: &mut Budget| {
            // Each receive uses a genuinely fresh ledger. Reusing the previous
            // admission would hide a missing or wrongly uncharged source check.
            let mut admission = SourceAdmission::default();
            let mut c = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
            let result = match root {
                0 => prepare::inspect(input, &r, &mut c, budget),
                1 => prepare::inspect_sentence(input, &r, &mut c, budget),
                _ => prepare::inspect_inline(input, &r, &mut c, budget),
            };
            Ok::<_, String>(result.map_err(err))
        };
        let mut measured = b();
        let expected = run(&d, &mut measured)??;
        assert_eq!(measured.usage().source_bytes, "原文".len() as u64);
        let mut limits = b().limits();
        limits.source_bytes = measured.usage().source_bytes;
        assert_eq!(run(&d, &mut Budget::new(limits))??, expected);
        limits.source_bytes -= 1;
        let mut stopped = Budget::new(limits);
        assert!(run(&d, &mut stopped)?.is_err());
        assert_eq!(stopped.poll(), Err(StopReason::SourceLimit));
        let mut admission = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &store, &mut admission).map_err(err)?;
        // Replace one leaf with a reference to a separately owned text leaf.
        let index = d.value.nodes.len() - 1;
        d.value.nodes.push(DocNode {
            kind: DocKind::Text {
                text: "label".into(),
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        d.value.nodes[index].kind = DocKind::Reference {
            target: "missing".into(),
            label: InlineRef((index + 1) as u64),
        };
        assert!(matches!(
            inspect(&d, &r, &mut c, &mut b()),
            Err(PreparationError::Label(LabelError::Unresolved { .. }))
        ));
    }
    Ok(())
}
