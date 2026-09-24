use super::*;
use nepl3_core::budget::{Budget, StopReason};
use nepl3_suite::adapters::document::{sentence, sentences};

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
