//! Backend-only opaque syntax fixtures. Actual Sentence semantics are tested
//! by tools/doc_prepare with the standard reader and selected Sentence adapter.
use nepl3_core::{
    budget::{Budget, Limits},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        OwnerProvenance, SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::*;
use nepl3_doc_html::*;
use nepl3_markup::html::*;
use nepl3_wire::foundation::FoundationCodec;
pub fn b() -> Budget {
    Budget::new(Limits {
        work: 1_000_000_000,
        allocation_units: 1_000_000_000,
        nodes: 1_000_000,
        depth: 100_000,
        output_bytes: 100_000_000,
        source_bytes: 1_000_000,
        ..Limits::default()
    })
}
pub fn err(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
pub fn registry() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
        nepl3_markup::schema::descriptor(&mut b()),
        nepl3_doc_html::schema::descriptor(&mut b()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        registry
            .register(
                descriptor.reference(&mut b()).map_err(err)?,
                descriptor,
                &mut b(),
            )
            .map_err(err)?;
    }
    registry.finalize(&mut b()).map_err(err)?;
    Ok(registry)
}
pub fn embed(registry: &SchemaRegistry, kind: EmbedKind, text: &str) -> Result<DocEmbed, String> {
    let source = SourceSnapshot::new(
        SourceId(format!(
            "backend-{:?}",
            nepl3_core::source::Digest::of(text.as_bytes())
        )),
        0,
        "memory:backend".into(),
        text.as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let span = source.span(0, source.text().len() as u64).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = codec
        .environment_digest(&environment, &mut b())
        .map_err(err)?;
    let schema = registry.selected("nepl3.doc", 1).ok_or("schema")?.clone();
    Ok(DocEmbed {
        kind,
        content: DocContent::Syntax {
            closure: Box::new(ForeignClosure {
                syntax: ForeignSyntax {
                    schema: schema.clone(),
                    category: match kind {
                        EmbedKind::Sentence => "Sentence",
                        EmbedKind::SentenceInline => "Inline",
                        _ => "Expr",
                    }
                    .into(),
                    root: NodeRef(0),
                    environment: EnvironmentRef { id: 1, digest },
                    bundle: SyntaxBundle {
                        sources: vec![source],
                        nodes: vec![SyntaxNode {
                            schema,
                            kind: "View:TextRun".into(),
                            fields: vec![],
                            head: Some(span.clone()),
                            cover: Some(span.clone()),
                            origin: OriginId(0),
                            token: None,
                        }],
                        origins: vec![Origin::Direct(span)],
                        root: NodeRef(0),
                        environments: vec![],
                        tokens: vec![],
                        source_maps: vec![],
                    },
                },
                owner_environment: EnvironmentEntry {
                    id: 1,
                    digest,
                    value: environment,
                },
                provenance: OwnerProvenance::from_parts(vec![], vec![], vec![]),
            }),
        },
    })
}
pub fn node(kind: DocKind) -> DocNode {
    DocNode {
        kind,
        locations: vec![],
        origin: None,
        span: None,
    }
}
pub fn document(root: DocRoot, kinds: Vec<DocKind>, embeds: Vec<DocEmbed>) -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root,
            nodes: kinds.into_iter().map(node).collect(),
            embeds,
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
pub fn request(registry: &SchemaRegistry, text: &str) -> Result<LocalHtmlRequest, String> {
    Ok(LocalHtmlRequest {
        document: document(
            DocRoot::Article(ArticleRef(0)),
            vec![
                DocKind::Article {
                    language: "ja".into(),
                    title: SentenceRef(1),
                    body: BodyRef(2),
                },
                DocKind::Sentence {
                    syntax: EmbedRef(0),
                },
                DocKind::Body { blocks: vec![] },
            ],
            vec![embed(registry, EmbedKind::Sentence, text)?],
        ),
        options: options(),
    })
}
pub fn options() -> RenderOptions {
    RenderOptions {
        parallel: ParallelMode::Rows,
    }
}
pub fn text(value: &str) -> HtmlRequest {
    HtmlRequest {
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![HtmlNode::Text { text: value.into() }],
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
    }
}
pub fn adapter(slot: &DocEmbed, _: EmbedRef, _: &mut Budget) -> Result<HtmlRequest, String> {
    let source = slot
        .syntax()
        .and_then(|closure| closure.syntax.bundle.sources.first())
        .ok_or("source")?;
    Ok(text(source.text()))
}
pub fn render_request(
    request: &LocalHtmlRequest,
    registry: &SchemaRegistry,
) -> Result<RenderedWithForeign, String> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &empty, &mut admission).map_err(err)?;
    let prepared = prepare_article_with_foreign(
        &request.document,
        &request.options,
        registry,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    render_article_with_foreign(&prepared, &mut adapter, &mut b()).map_err(err)
}
pub fn deep(count: u64) -> HtmlRequest {
    HtmlRequest {
        fragment: HtmlFragment {
            root: 0,
            nodes: (0..count)
                .map(|index| {
                    if index + 1 == count {
                        HtmlNode::Text {
                            text: "leaf".into(),
                        }
                    } else {
                        HtmlNode::Element {
                            tag: HtmlTag::Span,
                            attributes: vec![],
                            children: vec![index + 1],
                        }
                    }
                })
                .collect(),
        },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
    }
}
