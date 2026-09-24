//! Public API composition of the actual Article roots, including nested
//! Sentence and Doc Inline occurrences and all three origin coordinate spaces.
use super::*;
use nepl3_core::{budget::StopReason, schema::SchemaRegistry};
use nepl3_doc_html::pages::namespace as html;
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode, HtmlTag};
use nepl3_suite::adapters::sentence::html as sentence_html;

#[derive(Debug)]
struct Failure(String);
impl From<StopReason> for Failure {
    fn from(reason: StopReason) -> Self {
        Self(err(reason))
    }
}

pub(super) fn verify<C: FoundationValueCodec>(
    prepared: &html::PreparedPages<'_, '_, '_, '_>,
    compiled: &Compiled,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    let mut requests = Vec::new();
    for page in 0..2 {
        let mut records = Vec::new();
        let article = html::render_member(
            prepared,
            scopes::Owner {
                page,
                member: namespace::MemberId(0),
            },
            &mut |guest, embed, b| {
                let mut nested_origins = Vec::new();
                let sentence = nepl3_suite::adapters::document::sentence::lower(
                    guest,
                    &compiled.others[3].schema,
                    &[nepl3_sentence_core::lower::ForeignInlineForm {
                        kind: "Form:DocumentInline",
                        guest_schema: &compiled.doc.package.schema,
                        guest_category: "Inline",
                    }],
                    registry,
                    codec,
                    b,
                )
                .map_err(err)?;
                let selection = nepl3_suite::adapters::sentence::document_guests::collect(
                    &sentence,
                    &compiled.doc.package.schema,
                    registry,
                    codec,
                    b,
                )
                .map_err(err)?;
                let (documents, occurrences) = selection.into_parts();
                assert!(documents.len() <= 1);
                assert_eq!(occurrences.len(), documents.len());
                let owner = scopes::Owner {
                    page,
                    member: namespace::MemberId(1),
                };
                // Bind the independently lowered guest to the exact checked member.
                if let Some(document) = documents.first() {
                    let expected = prepared.checked().document(owner).ok_or("member")?;
                    assert_eq!(document, expected);
                }
                let mut count = 0;
                let mut admission = SourceAdmission::default();
                let rendered = sentence_html::render_part_with_foreign(
                    &sentence,
                    registry,
                    &mut |_, embed, b| {
                        assert_eq!(embed, occurrences[0].embed);
                        count += 1;
                        let member = html::render_member(
                            prepared,
                            owner,
                            &mut |guest, _, b| {
                                super::super::namespace::sentence_label(
                                    guest, compiled, registry, codec, b,
                                )
                            },
                            b,
                        )
                        .map_err(|error| Failure(err(error)))?;
                        let (_, _, rendered) = member.into_parts();
                        nested_origins = rendered.fragment.origins;
                        Ok::<_, Failure>(rendered.fragment.markup)
                    },
                    b,
                    &mut admission,
                )
                .map_err(|error| match error {
                    sentence_html::RenderFailure::Foreign(Failure(detail)) => detail,
                    error => err(error),
                })?;
                assert_eq!(count, occurrences.len());
                let (_, markup, origins, placements) = rendered.into_parts();
                assert_eq!(placements.len(), occurrences.len());
                if let Some(placement) = placements.first() {
                    for origin in &mut nested_origins {
                        assert!(origin.element < placement.elements);
                        origin.element += placement.first_element;
                    }
                }
                records.push((embed, nested_origins, origins, sentence));
                Ok::<_, String>(markup)
            },
            b,
        )
        .map_err(err)?;
        let (_, _, article) = article.into_parts();
        assert_eq!(article.foreign.len(), 2);
        assert_eq!(records.len(), 2);
        let nodes = &article.fragment.markup.fragment.nodes;
        assert!(matches!(
            nodes[article.fragment.markup.fragment.root as usize],
            HtmlNode::Element {
                tag: HtmlTag::Article,
                ..
            }
        ));
        for ((embed, nested_origins, sentence_origins, _), placement) in
            records.iter_mut().zip(&article.foreign)
        {
            assert_eq!(*embed, placement.embed);
            for origin in nested_origins.iter_mut() {
                assert!(origin.element < placement.elements);
                origin.element += placement.first_element;
            }
            for origin in sentence_origins.iter_mut() {
                assert!(origin.element < placement.elements);
                origin.element += placement.first_element;
            }
            assert!(
                sentence_origins
                    .iter()
                    .all(|origin| origin.element < nodes.len() as u64)
            );
        }
        assert!(records[0].1.iter().any(|origin| matches!(
            &nodes[origin.element as usize],
            HtmlNode::Element {
                tag: HtmlTag::A,
                ..
            } | HtmlNode::Element {
                tag: HtmlTag::Span,
                ..
            }
        )));
        assert!(records[1].1.is_empty());
        let root_document = prepared
            .checked()
            .document(scopes::Owner {
                page,
                member: namespace::MemberId(0),
            })
            .ok_or("Article owner")?;
        let mut article_mapped = 0;
        let mut paragraph_mapped = 0;
        for origin in &article.fragment.origins {
            match (
                &root_document.value.nodes[origin.node as usize].kind,
                &nodes[origin.element as usize],
            ) {
                (
                    DocKind::Article { .. },
                    HtmlNode::Element {
                        tag: HtmlTag::Article,
                        ..
                    },
                ) => article_mapped += 1,
                (
                    DocKind::Paragraph { .. },
                    HtmlNode::Element {
                        tag: HtmlTag::P, ..
                    },
                ) => paragraph_mapped += 1,
                _ => (),
            }
        }
        assert_eq!(article_mapped, 1);
        assert_eq!(paragraph_mapped, 1);
        let (_, _, origins, sentence) = &records[1];
        let expected_body = if page == 0 {
            "First body"
        } else {
            "Second body"
        };
        let matched = origins.iter().filter(|origin| matches!((&sentence.value.nodes[origin.node as usize], &nodes[origin.element as usize]), (nepl3_sentence_core::model::Kind::Text { text: source }, HtmlNode::Text { text: output }) if source == expected_body && output == expected_body)).count();
        assert_eq!(matched, 1);
        let document = prepared
            .checked()
            .document(scopes::Owner {
                page,
                member: namespace::MemberId(1),
            })
            .ok_or("Doc source owner")?;
        let mut mapped = 0;
        for origin in &records[0].1 {
            let cause = &document.value.nodes[origin.node as usize];
            let expected = match (&cause.kind, page) {
                (DocKind::Link { .. }, 0) => HtmlAttribute::Href {
                    value: HtmlHref::BetweenArtifacts {
                        source: "docs/first.html".into(),
                        target: "docs/second.html".into(),
                        fragment: Some("n-746172676574".into()),
                    },
                },
                (DocKind::Anchor { .. }, 1) => HtmlAttribute::Id {
                    value: "n-746172676574".into(),
                },
                _ => continue,
            };
            if let HtmlNode::Element { attributes, .. } = &nodes[origin.element as usize]
                && attributes.contains(&expected)
            {
                let span = cause.span.as_ref().ok_or("mapped source span")?;
                let snapshot = document
                    .sources
                    .iter()
                    .find(|source| source.identity() == span.snapshot_ref())
                    .ok_or("mapped snapshot")?;
                let text = snapshot
                    .slice_range(span.start(), span.end())
                    .map_err(err)?;
                assert_eq!(
                    text,
                    if page == 0 {
                        r#"link page "second" some "target" text "Go""#
                    } else {
                        r#"anchor target text "Goal""#
                    }
                );
                mapped += 1;
            }
        }
        assert_eq!(mapped, 1);
        requests.push(article.fragment.markup);
    }
    let checked = html::output::check(prepared, &requests, b).map_err(err)?;
    let rendered = checked
        .pages()
        .iter()
        .map(|page| nepl3_markup::html::serialize_xhtml(page, b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    assert!(rendered[0].contains("href=\"second.html#n-746172676574\""));
    assert!(rendered[0].contains("Go"));
    assert!(rendered[1].contains("id=\"n-746172676574\""));
    assert!(rendered[1].contains("Goal"));
    assert!(rendered[0].contains("First body"));
    assert!(rendered[1].contains("Second body"));
    // A semantic target does not guarantee that language selection emitted it.
    let mut hidden = requests.clone();
    for node in &mut hidden[1].fragment.nodes {
        if let HtmlNode::Element { attributes, .. } = node {
            attributes.retain(|attribute| !matches!(attribute, HtmlAttribute::Id { .. }));
        }
    }
    assert!(matches!(
        html::output::check(prepared, &hidden, b),
        Err(html::output::Error::MissingAnchor {
            page: 0,
            target: 1,
            ..
        })
    ));
    Ok(())
}
