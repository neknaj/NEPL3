use super::*;
use nepl3_core::budget::{Budget, StopReason};
use nepl3_suite::adapters::document::{sentence, sentences};

#[test]
fn sentence_html_preflight_identifies_shared_guests_and_stops_callbacks() -> Result<(), String> {
    use nepl3_markup::html::*;
    use nepl3_sentence_core::{lower::ForeignInlineForm, model::EmbedRef as SentenceEmbed};
    use nepl3_suite::adapters::sentence::html::{Error, RenderFailure};
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body
        cons paragraph cons sentence sentence cons doc ref first text "A" nil nil
        cons paragraph cons sentence sentence cons doc ref second text "B" nil nil
        cons code Math add 1 2 nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
        let mut doc = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        let blocks = doc
            .value
            .nodes
            .iter_mut()
            .find_map(|node| match &mut node.kind {
                DocKind::Body { blocks } => Some(blocks),
                _ => None,
            })
            .ok_or("Body")?;
        blocks.push(blocks[0]); // The first Sentence slot occurs twice in the document.
        let original = doc.clone();
        let surface = registry
            .selected("nepl3.syntax.sentence", 1)
            .ok_or("surface")?;
        let forms = [ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: &compiled.doc.package.schema,
            guest_category: "Inline",
        }];
        let selected =
            sentences::collect(&doc, surface, &forms, registry, &mut codec, &mut budget())
                .map_err(err)?;
        assert_eq!(
            selected
                .occurrences()
                .iter()
                .map(|o| o.embed)
                .collect::<Vec<_>>(),
            [EmbedRef(0), EmbedRef(1), EmbedRef(2), EmbedRef(1)]
        );
        let markup = || HtmlRequest {
            fragment: HtmlFragment {
                nodes: vec![HtmlNode::Text {
                    text: "guest".into(),
                }],
                root: 0,
            },
            slot: HtmlSlot::Phrasing,
            policy: HtmlPolicy { classes: vec![] },
        };
        let mut calls = Vec::new();
        let prepared = sentences::html::prepare(
            &selected,
            registry,
            &mut |slot, content, guest, _| {
                let sentence = selected.sentence(slot).ok_or(Error::InternalShape)?;
                assert!(core::ptr::eq(
                    content,
                    &sentence.value.embeds[guest.0 as usize]
                ));
                calls.push((slot, guest));
                Ok::<_, Error>(markup())
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        // Both independent Sentences use local guest index zero. Their Doc slots
        // distinguish the owners; the shared first slot is prepared only once.
        assert_eq!(
            calls,
            [
                (EmbedRef(1), SentenceEmbed(0)),
                (EmbedRef(2), SentenceEmbed(0))
            ]
        );
        let parts = prepared.into_parts();
        assert_eq!(parts.len(), 4);
        assert!(parts[..3].iter().all(Option::is_some));
        assert!(parts[3].is_none()); // Non-Sentence Code keeps its table position.
        calls.clear();
        let failed = sentences::html::prepare(
            &selected,
            registry,
            &mut |slot, _, guest, _| {
                calls.push((slot, guest));
                Err::<HtmlRequest, _>(Error::ForeignAdapterRequired(guest))
            },
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            failed,
            Err(sentences::html::Error::Sentence {
                embed: EmbedRef(1),
                error: RenderFailure::Foreign(Error::ForeignAdapterRequired(SentenceEmbed(0))),
            })
        ));
        assert_eq!(calls, [(EmbedRef(1), SentenceEmbed(0))]);
        calls.clear();
        let mut cancelled = budget();
        let stopped = sentences::html::prepare(
            &selected,
            registry,
            &mut |slot, _, guest, b| {
                calls.push((slot, guest));
                b.cancel();
                Ok::<_, Error>(markup())
            },
            &mut cancelled,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            stopped,
            Err(sentences::html::Error::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(calls, [(EmbedRef(1), SentenceEmbed(0))]);
        assert_eq!(doc, original);
        Ok(())
    })
}

#[test]
fn sentence_collection_checks_all_variants_sharing_and_resource_boundaries() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body
        cons paragraph cons parallel cons variant en sentence "A" cons variant ja sentence "B" nil nil
        cons code Math add 1 2 nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let registry = profile.registry();
        let store = SourceStore::default();
        let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
        let mut doc = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            registry,
            b,
            &mut codec,
        )
        .map_err(err)?;
        let surface = registry
            .selected("nepl3.syntax.sentence", 1)
            .ok_or("surface")?;
        let body = doc
            .value
            .nodes
            .iter_mut()
            .find_map(|n| match &mut n.kind {
                DocKind::Body { blocks } => Some(blocks),
                _ => None,
            })
            .ok_or("Body")?;
        body.push(body[0]); // One immutable Parallel is displayed twice.
        let original = doc.clone();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let mut measured = budget();
        let selected = sentences::collect(&doc, surface, &[], registry, &mut codec, &mut measured)
            .map_err(err)?;
        assert!(core::ptr::eq(selected.document(), &doc));
        assert_eq!(
            selected
                .occurrences()
                .iter()
                .map(|o| (o.embed, o.depth))
                .collect::<Vec<_>>(),
            [
                (EmbedRef(0), 2),
                (EmbedRef(1), 6),
                (EmbedRef(2), 6),
                (EmbedRef(1), 6),
                (EmbedRef(2), 6)
            ]
        );
        assert!(selected.sentence(EmbedRef(3)).is_none()); // Code keeps its original guest.
        assert!(selected.sentence(EmbedRef(u64::MAX)).is_none());
        for (embed, expected) in [(0, "Title"), (1, "A"), (2, "B")] {
            let input = selected.sentence(EmbedRef(embed)).ok_or("Sentence")?;
            let text: Vec<_> = input
                .value
                .nodes
                .iter()
                .filter_map(|n| match n {
                    nepl3_sentence_core::model::Kind::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(text, [expected]);
        }
        let usage = measured.usage();
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::SourceLimit,
        ] {
            for shortage in [0, 1] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = usage.work - shortage,
                    StopReason::AllocationLimit => {
                        limits.allocation_units = usage.allocation_units - shortage
                    }
                    StopReason::DepthLimit => limits.depth = usage.depth - shortage,
                    StopReason::SourceLimit => limits.source_bytes = usage.source_bytes - shortage,
                    _ => return Err("fixture resource".into()),
                }
                let mut limited = Budget::new(limits);
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let result =
                    sentences::collect(&doc, surface, &[], registry, &mut codec, &mut limited);
                if shortage == 0 {
                    let actual = result.map_err(err)?;
                    assert_eq!(actual.occurrences(), selected.occurrences());
                    for embed in 0..4 {
                        assert_eq!(
                            actual.sentence(EmbedRef(embed)),
                            selected.sentence(EmbedRef(embed))
                        );
                    }
                } else {
                    assert!(
                        matches!(result, Err(sentences::Error::Stopped(actual)) if actual == reason),
                        "{reason:?}"
                    );
                    assert_eq!(limited.poll(), Err(reason));
                }
                assert_eq!(doc, original);
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            sentences::collect(&doc, surface, &[], registry, &mut codec, &mut cancelled),
            Err(sentences::Error::Stopped(StopReason::Cancelled))
        ));
        let mut wrong = doc.clone();
        let DocContent::Syntax { closure } = &mut wrong.value.embeds[2].content else {
            return Err("syntax slot".into());
        };
        closure.syntax.category = "Inline".into();
        // The ja variant participates even when a renderer would select en.
        assert!(matches!(
            sentences::collect(&wrong, surface, &[], registry, &mut codec, &mut budget()),
            Err(sentences::Error::Sentence {
                embed: EmbedRef(2),
                error: sentence::Error::Category
            })
        ));
        wrong = doc.clone();
        wrong.value.root = DocRoot::Article(ArticleRef(u64::MAX));
        assert!(matches!(
            sentences::collect(&wrong, surface, &[], registry, &mut codec, &mut budget()),
            Err(sentences::Error::Document(_))
        ));
        Ok(())
    })
}

#[test]
fn standalone_doc_label_collects_independent_inline_without_resolving_names() -> Result<(), String>
{
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"ref later text "go""#,
        "Inline",
        |tree, profile, b, a| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut codec = FoundationCodec::new(registry, &store, a).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Inline,
                registry,
                b,
                &mut codec,
            )
            .map_err(err)?;
            let surface = registry
                .selected("nepl3.syntax.sentence", 1)
                .ok_or("surface")?;
            let selected =
                sentences::collect(&doc, surface, &[], registry, &mut codec, b).map_err(err)?;
            assert_eq!(selected.occurrences().len(), 1);
            assert_eq!(selected.occurrences()[0].kind, EmbedKind::SentenceInline);
            assert_eq!(selected.occurrences()[0].depth, 1);
            let label = selected.sentence(EmbedRef(0)).ok_or("label")?;
            assert!(matches!(
                label.value.root,
                nepl3_sentence_core::model::Root::Inline(_)
            ));
            assert!(
                matches!(&label.value.nodes[0], nepl3_sentence_core::model::Kind::Text { text } if text == "go")
            );
            Ok(())
        },
    )
}
