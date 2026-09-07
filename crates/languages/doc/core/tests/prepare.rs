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
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
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
