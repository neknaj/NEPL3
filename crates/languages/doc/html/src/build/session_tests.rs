// Synthetic prepared-engine fixtures. Front-door source/closure validation is
// covered separately by public integration tests; these test pause transitions.
use super::*;
use alloc::{format, vec};
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

fn fixture() -> Result<DocumentSyntax, String> {
    let r = registry()?;
    let closure = closure(&r)?;
    let kinds = vec![
        DocKind::Article {
            language: "en".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "text".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4), BlockRef(5), BlockRef(6), BlockRef(5)],
        },
        DocKind::Code {
            syntax: EmbedRef(0),
        },
        DocKind::DisplayMath {
            syntax: EmbedRef(1),
        },
        DocKind::Paragraph {
            items: vec![FlowRef(7)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(8), InlineRef(9)],
        },
        DocKind::InlineMath {
            syntax: EmbedRef(2),
        },
        DocKind::Anno {
            base: InlineRef(8),
            notes: vec![InlineRef(2)],
        },
    ];
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
            embeds: [
                EmbedKind::Code,
                EmbedKind::DisplayMath,
                EmbedKind::InlineMath,
            ]
            .into_iter()
            .map(|kind| DocEmbed {
                kind,
                closure: closure.clone(),
            })
            .collect(),
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    })
}
fn preparation<'a>(
    document: &'a DocumentSyntax,
    options: &'a RenderOptions,
) -> crate::prepare::PreparedRendering<'a> {
    crate::prepare::PreparedRendering {
        document,
        options,
        selections: vec![None; document.value.nodes.len()],
        identity: nepl3_core::source::Digest([7; 32]),
        images: vec![],
    }
}
fn markup(index: u64) -> HtmlRequest {
    HtmlRequest {
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Text {
                text: format!("guest-{index}"),
            }],
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
    }
}
#[test]
fn session_retains_real_order_depth_and_one_shot_completion() -> Result<(), String> {
    let document = fixture()?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = preparation(&document, &options);
    let mut budget = b();
    budget
        .with_depth_at_least::<_, RenderError>(11, |budget| {
            let mut session = Session::new(&prepared, &[], budget, true)?;
            let mut nodes = Vec::new();
            let mut depths = Vec::new();
            let rendered = loop {
                match session.advance()? {
                    Step::Guest(context) => {
                        assert!(core::ptr::eq(context.document(), &document));
                        assert!(core::ptr::eq(context.options(), &options));
                        assert_eq!(context.ordinal(), nodes.len() as u64);
                        let index = context.ordinal();
                        nodes.push(context.node());
                        assert!(matches!(session.advance()?, Step::Waiting));
                        let request = session
                            .with_pending(|budget| {
                                depths.push(budget.current_depth());
                                Ok::<_, RenderError>(markup(index))
                            })
                            .map_err(|e| match e {
                                ForeignRenderError::Render(e) | ForeignRenderError::Foreign(e) => e,
                            })?;
                        assert!(matches!(session.advance()?, Step::Waiting));
                        session.resume(request)?;
                    }
                    Step::Finished(rendered) => break rendered,
                    Step::Waiting => return Err(RenderError::InternalShape),
                }
            };
            assert_eq!(nodes, vec![4, 5, 8, 8, 5]);
            assert_eq!(depths, vec![15, 13, 15, 17, 13]);
            assert_eq!(rendered.foreign.len(), 5);
            assert!(session.advance().is_err());
            assert!(session.resume(markup(99)).is_err());
            Ok(())
        })
        .map_err(err)?;
    assert_eq!(budget.current_depth(), 0);
    Ok(())
}
#[test]
fn failed_or_duplicate_resume_cannot_finish_a_partial_document() -> Result<(), String> {
    let document = fixture()?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = preparation(&document, &options);
    for mode in 0..4 {
        let mut budget = b();
        let mut session = Session::new(&prepared, &[], &mut budget, true).map_err(err)?;
        let Step::Guest(_) = session.advance().map_err(err)? else {
            return Err("guest".into());
        };
        match mode {
            0 => {
                session.resume(markup(0)).map_err(err)?;
                assert!(session.resume(markup(0)).is_err());
            }
            1 => {
                assert!(matches!(
                    session.with_pending(|_| Err::<HtmlRequest, _>("callback")),
                    Err(ForeignRenderError::Foreign("callback"))
                ));
            }
            2 => {
                let mut bad = markup(0);
                bad.fragment.root = 99;
                assert!(session.resume(bad).is_err());
            }
            _ => {
                assert!(matches!(
                    session.with_pending(|b| {
                        b.cancel();
                        Err::<HtmlRequest, _>("callback")
                    }),
                    Err(ForeignRenderError::Render(RenderError::Stopped(
                        StopReason::Cancelled
                    )))
                ));
            }
        }
        assert!(session.advance().is_err());
    }
    Ok(())
}
