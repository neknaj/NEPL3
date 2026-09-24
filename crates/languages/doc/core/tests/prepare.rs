use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_core::{
    origin::{Origin, OriginId},
    source::{SourceId, SourceSnapshot},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    model::*,
    portable,
    prepare::{self, DocRequirement, PreparationError},
};
use nepl3_wire::foundation::FoundationCodec;
#[path = "support/closure.rs"]
mod support;
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
fn document(r: &SchemaRegistry) -> Result<DocumentSyntax, String> {
    let asset = AssetRef {
        id: "図".into(),
        digest: Some(Digest::of(b"image input")),
    };
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(2),
        },
        DocKind::Sentence {
            syntax: EmbedRef(0),
        },
        DocKind::Body {
            blocks: vec![BlockRef(3), BlockRef(4)],
        },
        DocKind::Image {
            asset: asset.clone(),
            alt: SentenceRef(1),
            caption: None,
        },
        DocKind::Image {
            asset,
            alt: SentenceRef(1),
            caption: None,
        },
    ];
    let mut guest = support::closure(r)?;
    guest.syntax.category = "Sentence".into();
    Ok(DocumentSyntax {
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
            embeds: vec![DocEmbed {
                kind: EmbedKind::Sentence,
                content: DocContent::Syntax {
                    closure: Box::new(guest),
                },
            }],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    })
}
#[test]
fn namespace_plans_preserve_order_and_reserve_before_processing() -> Result<(), String> {
    use nepl3_doc_core::labels::namespace;
    let r = registry()?;
    let store = SourceStore::default();
    for count in [0, 1, 16, 64] {
        let documents: Vec<_> = (0..count)
            .map(|index| {
                let mut d = document(&r)?;
                if let DocKind::Article { language, .. } = &mut d.value.nodes[0].kind {
                    *language = format!("x-{index}");
                }
                Ok(d)
            })
            .collect::<Result<Vec<_>, String>>()?;
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
    let d = document(&r)?;
    let copy = d.clone();
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let actual = prepare::inspect(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        actual.requirements[..2],
        vec![
            DocRequirement::Asset {
                node: 3,
                asset: AssetRef {
                    id: "図".into(),
                    digest: Some(Digest::of(b"image input"))
                }
            },
            DocRequirement::Asset {
                node: 4,
                asset: AssetRef {
                    id: "図".into(),
                    digest: Some(Digest::of(b"image input"))
                }
            }
        ]
    );
    assert_eq!(actual.requirements.len(), 3);
    assert!(matches!(
        actual.requirements[2],
        DocRequirement::Foreign {
            embed: EmbedRef(0),
            kind: EmbedKind::Sentence,
            ..
        }
    ));
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
    let d = document(&r)?;
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
    if let DocKind::Article { language, .. } = &mut stale.value.nodes[0].kind {
        *language = "en".into();
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
fn inline_inspection_preserves_link_requirements_labels_and_sticky_stops() -> Result<(), String> {
    let r = registry()?;
    let mut d = document(&r)?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    d.value.root = DocRoot::Inline(InlineRef(0));
    d.value.nodes.truncate(1);
    d.value.embeds[0].kind = EmbedKind::SentenceInline;
    let DocContent::Syntax { closure } = &mut d.value.embeds[0].content else {
        return Err("closure".into());
    };
    closure.syntax.category = "Inline".into();
    let target = LinkTarget::Page {
        page: "guide".into(),
        fragment: Some("introduction".into()),
    };
    d.value.nodes[0].kind = DocKind::Link {
        target: target.clone(),
        label: EmbedRef(0),
    };
    let plan = prepare::inspect_inline(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        plan.requirements[0],
        DocRequirement::Link { node: 0, target }
    );
    assert_eq!(plan.requirements.len(), 2);
    // InlineImage and block Image own the same asset contract independently.
    let asset = AssetRef {
        id: "inline-image".into(),
        digest: Some(Digest::of(b"inline image")),
    };
    d.value.nodes[0].kind = DocKind::InlineImage {
        asset: asset.clone(),
        alt: SentenceRef(1),
    };
    d.value.nodes.push(DocNode {
        kind: DocKind::Sentence {
            syntax: EmbedRef(0),
        },
        locations: vec![],
        origin: None,
        span: None,
    });
    d.value.embeds[0].kind = EmbedKind::Sentence;
    if let DocContent::Syntax { closure } = &mut d.value.embeds[0].content {
        closure.syntax.category = "Sentence".into();
    }
    let image = prepare::inspect_inline(&d, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(
        image.requirements[0],
        DocRequirement::Asset { node: 0, asset }
    );
    assert_eq!(image.requirements.len(), 2);
    d.value.nodes.truncate(1);
    d.value.embeds[0].kind = EmbedKind::SentenceInline;
    if let DocContent::Syntax { closure } = &mut d.value.embeds[0].content {
        closure.syntax.category = "Inline".into();
    }
    d.value.nodes[0].kind = DocKind::Reference {
        target: "missing".into(),
        label: EmbedRef(0),
    };
    assert!(matches!(
        prepare::inspect_inline(&d, &r, &mut c, &mut b()),
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
            prepare::inspect_inline(&d, &r, &mut c, &mut b),
            Err(PreparationError::Stopped(reason))
        );
        assert_eq!(
            prepare::inspect_inline(&d, &r, &mut c, &mut b),
            Err(PreparationError::Stopped(reason))
        );
    }
    Ok(())
}
