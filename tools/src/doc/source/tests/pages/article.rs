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

#[test]
fn cached_article_keeps_shared_sentence_occurrences_distinct() -> Result<(), String> {
    use crate::doc::export::pages::sentences as host;
    use nepl3_suite::adapters::document::sentences;
    let compiled = compiled()?;
    with_named_input(
        true,
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons sentence "Shared" nil nil"#,
        "shared",
        "Article",
        |tree, profile, b, a| {
            let registry = profile.registry();
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(registry, &empty, a).map_err(err)?;
            let input = tree
                .tree()
                .bundle
                .validate_with_sources(registry, b, &mut SourceAdmission::default())
                .map_err(err)?;
            let mut document = lower::document(
                &input,
                &compiled.doc.package.schema,
                Category::Article,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let blocks = document
                .value
                .nodes
                .iter_mut()
                .find_map(|node| match &mut node.kind {
                    DocKind::Body { blocks } => Some(blocks),
                    _ => None,
                })
                .ok_or("body")?;
            blocks.push(blocks[0]);
            let set = pages::PageSet {
                pages: vec![pages::PageDocument {
                    registration: pages::PageRegistration {
                        id: "shared".into(),
                        source: "shared.nepld".into(),
                        route: "shared.html".into(),
                    },
                    document,
                }],
                files: vec![],
            };
            let document = &set.pages[0].document;
            let member = namespace::inspect(document, registry, b, &mut SourceAdmission::default())
                .map_err(err)?;
            let members = [&member];
            let local = namespace::resolve(&members, b).map_err(err)?;
            let namespaces = [&local];
            let checked =
                scopes::resolve(&set, &namespaces, registry, &mut codec, b).map_err(err)?;
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let prepared = html::prepare(&checked, &options, b).map_err(err)?;
            let selection = sentences::collect(
                document,
                &compiled.others[3].schema,
                &[],
                registry,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let owner = scopes::Owner {
                page: 0,
                member: namespace::MemberId(0),
            };
            let cached = host::prepare(
                &selection,
                &prepared,
                owner,
                registry,
                &mut |_, _, _, _| Err::<_, Failure>(Failure("unexpected Sentence guest".into())),
                b,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            // The same root Doc can participate in a larger checked namespace.
            // Its prepared Sentence output is bound to the original membership.
            let expanded_members = [&member, &member];
            let expanded_local = namespace::resolve(&expanded_members, b).map_err(err)?;
            let expanded_namespaces = [&expanded_local];
            let expanded = scopes::resolve(&set, &expanded_namespaces, registry, &mut codec, b)
                .map_err(err)?;
            let expanded = html::prepare(&expanded, &options, b).map_err(err)?;
            assert!(matches!(host::render_member(&cached, &expanded, owner,
                &mut |_, _, _| Err::<_, Failure>(Failure("unexpected changed-namespace adapter".into())), b),
                Err(host::Error::Selection(actual)) if actual == owner));
            let changed_options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Single {
                    language: "en".into(),
                    fallbacks: vec![],
                },
            };
            let changed = html::prepare(&checked, &changed_options, b).map_err(err)?;
            assert!(matches!(host::render_member(&cached, &changed, owner,
                &mut |_, _, _| Err::<_, Failure>(Failure("unexpected changed-options adapter".into())), b),
                Err(host::Error::Selection(actual)) if actual == owner));
            let output = host::render_member(
                &cached,
                &prepared,
                owner,
                &mut |_, _, _| Err::<_, Failure>(Failure("unexpected Doc guest".into())),
                b,
            )
            .map_err(err)?;
            let occurrences: Vec<_> = output.sentences().collect();
            assert_eq!(occurrences.len(), 3);
            let (first, first_input) = occurrences[1];
            let (second, second_input) = occurrences[2];
            assert_eq!(first.embed, second.embed);
            assert!(core::ptr::eq(first_input, second_input));
            assert_eq!(first.elements, second.elements);
            assert!(first.first_element + first.elements <= second.first_element);
            let nodes = &output.member().output().fragment.markup.fragment.nodes;
            let mut mapped = 0;
            for origin in first_input.origins() {
                if matches!(&first_input.input().value.nodes[origin.node as usize], nepl3_sentence_core::model::Kind::Text { text } if text == "Shared")
                {
                    for placement in [first, second] {
                        assert!(
                            matches!(&nodes[(placement.first_element + origin.element) as usize], HtmlNode::Text { text } if text == "Shared")
                        );
                        mapped += 1;
                    }
                }
            }
            assert_eq!(mapped, 2);
            let (output, retained) = output.into_parts();
            assert!(core::ptr::eq(retained, &cached));
            let requests = [output.into_parts().2.fragment.markup];
            let complete = html::output::check(&prepared, &requests, b).map_err(err)?;
            assert_eq!(complete.pages().len(), 1);
            Ok(())
        },
    )
}

pub(super) fn verify<C: FoundationValueCodec>(
    prepared: &html::PreparedPages<'_, '_, '_, '_>,
    plans: &[crate::doc::export::pages::discovery::namespace::Plan<'_, '_>],
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
    verify_cached(prepared, compiled, registry, codec, &requests)?;
    verify_discovered(prepared, plans, registry, &requests)?;
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

fn verify_discovered(
    prepared: &html::PreparedPages<'_, '_, '_, '_>,
    plans: &[crate::doc::export::pages::discovery::namespace::Plan<'_, '_>],
    registry: &SchemaRegistry,
    expected: &[nepl3_markup::html::HtmlRequest],
) -> Result<(), String> {
    use crate::doc::export::pages::composition as host;
    for (page, (plan, expected)) in plans.iter().zip(expected).enumerate() {
        let run = |b: &mut Budget| {
            host::render(
                plan,
                prepared,
                page as u64,
                registry,
                &mut |_, _, _, _, _| Err::<_, Failure>(Failure("unexpected Sentence guest".into())),
                &mut |_, _, _, _| Err::<_, Failure>(Failure("unexpected Doc guest".into())),
                b,
                &mut SourceAdmission::default(),
            )
        };
        let mut measured = budget();
        let output = run(&mut measured).map_err(err)?;
        assert_eq!(
            &output.members()[0].document().output().fragment.markup,
            expected
        );
        assert!(core::ptr::eq(output.plan(), plan));
        // The nested Doc label belongs to a distinct Sentence arena; each
        // insertion retains its own placement and original syntax reference.
        for (input, member) in plan.input().members().iter().zip(output.members()) {
            for placement in &member.document().output().foreign {
                let sentence = member.sentence(placement.embed).ok_or("Sentence output")?;
                assert!(core::ptr::eq(
                    sentence.input(),
                    input.sentences()[placement.embed.0 as usize]
                        .as_ref()
                        .ok_or("input slot")?
                ));
                assert_eq!(
                    placement.elements,
                    sentence.markup().fragment.nodes.len() as u64
                );
                for origin in sentence.origins() {
                    let mut expected =
                        sentence.markup().fragment.nodes[origin.element as usize].clone();
                    match &mut expected {
                        HtmlNode::Text { .. } => {}
                        HtmlNode::Element { children, .. }
                        | HtmlNode::MathElement { children, .. } => {
                            for child in children {
                                *child += placement.first_element;
                            }
                        }
                    }
                    assert_eq!(
                        member.document().output().fragment.markup.fragment.nodes
                            [(placement.first_element + origin.element) as usize],
                        expected
                    );
                }
            }
        }
        for (reason, amount) in [
            (StopReason::WorkLimit, measured.usage().work),
            (
                StopReason::AllocationLimit,
                measured.usage().allocation_units,
            ),
            (StopReason::NodeLimit, measured.usage().nodes),
            (StopReason::DepthLimit, measured.usage().depth),
        ] {
            for shortage in [0, 1] {
                let mut limits = measured.limits();
                match reason {
                    StopReason::WorkLimit => limits.work = amount - shortage,
                    StopReason::AllocationLimit => limits.allocation_units = amount - shortage,
                    StopReason::NodeLimit => limits.nodes = amount - shortage,
                    StopReason::DepthLimit => limits.depth = amount - shortage,
                    _ => return Err("resource".into()),
                }
                let result = run(&mut Budget::new(limits));
                if shortage == 0 {
                    assert!(result.is_ok(), "{reason:?}");
                } else {
                    assert!(
                        matches!(result, Err(host::Error::Stopped(actual)) if actual == reason),
                        "{reason:?}"
                    );
                }
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            run(&mut cancelled),
            Err(host::Error::Stopped(StopReason::Cancelled))
        ));
        assert!(matches!(
            host::render(
                plan,
                prepared,
                99,
                registry,
                &mut |_, _, _, _, _| Err::<_, Failure>(Failure("unexpected callback".into())),
                &mut |_, _, _, _| Err::<_, Failure>(Failure("unexpected callback".into())),
                &mut budget(),
                &mut SourceAdmission::default()
            ),
            Err(host::Error::Selection(_))
        ));
    }
    Ok(())
}

fn verify_cached<C: FoundationValueCodec>(
    namespace: &html::PreparedPages<'_, '_, '_, '_>,
    compiled: &Compiled,
    registry: &SchemaRegistry,
    codec: &mut C,
    expected: &[nepl3_markup::html::HtmlRequest],
) -> Result<(), String>
where
    C::Error: core::fmt::Debug,
{
    use nepl3_suite::adapters::document::sentences;
    for (page, expected) in expected.iter().enumerate() {
        let owner = scopes::Owner {
            page: page as u64,
            member: namespace::MemberId(0),
        };
        let document = namespace.checked().document(owner).ok_or("Article")?;
        let selection = sentences::collect(
            document,
            &compiled.others[3].schema,
            &[nepl3_sentence_core::lower::ForeignInlineForm {
                kind: "Form:DocumentInline",
                guest_schema: &compiled.doc.package.schema,
                guest_category: "Inline",
            }],
            registry,
            codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut guest_calls = 0;
        let cached = crate::doc::export::pages::sentences::prepare(
            &selection,
            namespace,
            owner,
            registry,
            &mut |_, _, _, b| {
                guest_calls += 1;
                let guest = html::render_member(
                    namespace,
                    scopes::Owner {
                        page: owner.page,
                        member: namespace::MemberId(1),
                    },
                    &mut |guest, _, b| {
                        super::super::namespace::sentence_label(guest, compiled, registry, codec, b)
                    },
                    b,
                )
                .map_err(|e| Failure(err(e)))?;
                Ok::<_, Failure>(guest.into_parts().2.fragment.markup)
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        assert_eq!(guest_calls, 1);
        let mut measured = budget();
        let output = crate::doc::export::pages::sentences::render_member(
            &cached,
            namespace,
            owner,
            &mut |_, _, _| Err::<_, Failure>(Failure("unexpected Doc adapter".into())),
            &mut measured,
        )
        .map_err(err)?;
        assert_eq!(&output.member().output().fragment.markup, expected);
        let occurrences: Vec<_> = output.sentences().collect();
        assert_eq!(occurrences.len(), 2);
        for (placement, sentence) in occurrences {
            assert_eq!(
                placement.elements,
                sentence.markup().fragment.nodes.len() as u64
            );
            assert!(core::ptr::eq(
                sentence.input(),
                selection.sentence(placement.embed).ok_or("slot")?
            ));
            for origin in sentence.origins() {
                assert!(origin.element < placement.elements);
                assert!(origin.node < sentence.input().value.nodes.len() as u64);
                if let HtmlNode::Text { text } =
                    &sentence.markup().fragment.nodes[origin.element as usize]
                {
                    assert!(
                        matches!(&expected.fragment.nodes[(placement.first_element + origin.element) as usize], HtmlNode::Text { text: output } if output == text)
                    );
                }
            }
        }
        for (reason, amount) in [
            (StopReason::WorkLimit, measured.usage().work),
            (
                StopReason::AllocationLimit,
                measured.usage().allocation_units,
            ),
            (StopReason::NodeLimit, measured.usage().nodes),
            (StopReason::DepthLimit, measured.usage().depth),
        ] {
            for shortage in [0, 1] {
                let mut limits = measured.limits();
                match reason {
                    StopReason::WorkLimit => limits.work = amount - shortage,
                    StopReason::AllocationLimit => limits.allocation_units = amount - shortage,
                    StopReason::NodeLimit => limits.nodes = amount - shortage,
                    StopReason::DepthLimit => limits.depth = amount - shortage,
                    _ => return Err("fixture resource".into()),
                }
                let result = crate::doc::export::pages::sentences::render_member(
                    &cached,
                    namespace,
                    owner,
                    &mut |_, _, _| Err::<_, Failure>(Failure("unexpected Doc adapter".into())),
                    &mut Budget::new(limits),
                );
                if shortage == 0 {
                    assert_eq!(
                        &result.map_err(err)?.member().output().fragment.markup,
                        expected
                    );
                } else {
                    assert!(
                        matches!(result, Err(crate::doc::export::pages::sentences::Error::Stopped(actual)) if actual == reason)
                    );
                }
            }
        }
        let wrong = scopes::Owner {
            page: 1 - owner.page,
            member: owner.member,
        };
        assert!(
            matches!(crate::doc::export::pages::sentences::render_member(
            &cached, namespace, wrong,
            &mut |_, _, _| Err::<_, Failure>(Failure("unexpected adapter".into())),
            &mut budget(),
        ), Err(crate::doc::export::pages::sentences::Error::Selection(actual)) if actual == wrong)
        );
        assert_eq!(guest_calls, 1); // Rendering repeatedly consumes only prepared output.
    }
    Ok(())
}
