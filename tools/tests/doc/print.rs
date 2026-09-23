use super::*;
use nepl3_doc_core::print::{self, PrintMode, PrintOutcome, PrintRequest};

use entry::entry;
#[path = "print/entry.rs"]
mod entry;
use nepl3_doc_core::{
    check::Category,
    lower,
    model::*,
};

#[test]
fn paragraph_edit_uses_model_span_and_preserves_surrounding_source() -> Result<(), String> {
    let compiled = compiled()?;
    let input = "article en \"T\" body\r\n  cons paragraph cons \"same\" nil\r\n  cons paragraph cons \"[字/じ]{base/note}\" nil\r\n  cons paragraph cons \"same\" nil nil";
    with_input(&compiled, input, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let original = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let DocRoot::Article(root) = original.value.root else {
            return Err("article root".into());
        };
        let DocKind::Article { body, .. } = original.value.nodes[root.0 as usize].kind else {
            return Err("article node".into());
        };
        let DocKind::Body { ref blocks } = original.value.nodes[body.0 as usize].kind else {
            return Err("article body".into());
        };
        assert_eq!(blocks.len(), 3);
        let target = blocks[1];
        assert!(matches!(
            original.value.nodes[target.0 as usize].kind,
            DocKind::Paragraph { .. }
        ));
        let span = original.value.nodes[target.0 as usize]
            .span
            .clone()
            .ok_or("paragraph source span")?;
        let source = original
            .sources
            .iter()
            .find(|s| s.identity() == span.snapshot_ref())
            .ok_or("paragraph source")?;
        let mut fragment = original
            .fragment(
                DocRoot::Block(target),
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        let DocRoot::Block(target) = fragment.value.root else {
            return Err("fragment root".into());
        };
        let next = fragment.value.nodes.len() as u64;
        for kind in [
            DocKind::Body {
                blocks: vec![target],
            },
            DocKind::ListItem {
                checked: None,
                body: BodyRef(next),
            },
            DocKind::List {
                kind: ListKind::Unordered,
                items: vec![ListItemRef(next + 1)],
            },
        ] {
            fragment.value.nodes.push(DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            });
        }
        fragment.value.root = DocRoot::Block(BlockRef(next + 2));
        let request = PrintRequest {
            document: fragment,
            mode: PrintMode::Prefix,
            bindings: vec![],
            guests: vec![],
        };
        let reply =
            print::print(&request, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
        let PrintOutcome::Complete { artifact } = reply.outcome else {
            return Err(format!("fragment print: {reply:?}"));
        };
        let edit = TextEdit {
            expected_digest: Digest::of(source.slice(&span).map_err(err)?.as_bytes()),
            span: span.clone(),
            replacement: artifact.text,
        };
        let mut store = SourceStore::default();
        store
            .insert_with_budget(source.clone(), &mut budget())
            .map_err(err)?;
        let revisions = store
            .apply(
                core::slice::from_ref(&edit),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        assert_eq!(revisions.len(), 1);
        let revised = store
            .get_ref(&revisions[0])
            .ok_or("edited snapshot")?
            .clone();
        assert_ne!(revised.identity(), source.identity());
        assert_eq!(
            &revised.text().as_bytes()[..span.start() as usize],
            &source.text().as_bytes()[..span.start() as usize]
        );
        assert_eq!(
            &revised.text().as_bytes()[span.start() as usize + edit.replacement.len()..],
            &source.text().as_bytes()[span.end() as usize..]
        );
        assert!(
            revised.slice(&span).is_err(),
            "old spans must not address a new revision"
        );
        assert!(
            store
                .apply(
                    core::slice::from_ref(&edit),
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .is_err(),
            "a stale edit must be rejected"
        );
        let parsed = parse_source_as(
            &revised,
            profile,
            "Doc",
            "Article",
            &mut budget(),
            &mut SourceAdmission::default(),
        )?;
        let parsed = parsed
            .bundle
            .validate_with_sources(
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        let actual = lower::document(
            &parsed,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let DocRoot::Article(root) = actual.value.root else {
            return Err("edited article root".into());
        };
        let DocKind::Article { body, .. } = actual.value.nodes[root.0 as usize].kind else {
            return Err("edited article".into());
        };
        let DocKind::Body { ref blocks } = actual.value.nodes[body.0 as usize].kind else {
            return Err("edited body".into());
        };
        assert_eq!(blocks.len(), 3);
        assert!(matches!(
            actual.value.nodes[blocks[0].0 as usize].kind,
            DocKind::Paragraph { .. }
        ));
        assert!(matches!(
            actual.value.nodes[blocks[2].0 as usize].kind,
            DocKind::Paragraph { .. }
        ));
        let DocKind::List { ref items, .. } = actual.value.nodes[blocks[1].0 as usize].kind else {
            return Err("edited list".into());
        };
        assert_eq!(items.len(), 1);
        let DocKind::ListItem { body, .. } = actual.value.nodes[items[0].0 as usize].kind else {
            return Err("edited item".into());
        };
        let DocKind::Body { ref blocks } = actual.value.nodes[body.0 as usize].kind else {
            return Err("item body".into());
        };
        assert_eq!(blocks.len(), 1);
        let DocKind::Paragraph { ref items } = actual.value.nodes[blocks[0].0 as usize].kind else {
            return Err("preserved paragraph".into());
        };
        assert_eq!(items.len(), 1);
        let DocKind::Sentence { ref inlines } = actual.value.nodes[items[0].0 as usize].kind else {
            return Err("preserved sentence".into());
        };
        assert_eq!(inlines.len(), 2);
        let DocKind::Ruby { base, reading } = actual.value.nodes[inlines[0].0 as usize].kind else {
            return Err("preserved Ruby".into());
        };
        let text = |id: InlineRef| -> Result<&str, String> {
            match &actual.value.nodes[id.0 as usize].kind {
                DocKind::Text { text } => Ok(text),
                _ => Err("preserved Text".into()),
            }
        };
        assert_eq!(text(base)?, "字");
        assert_eq!(text(reading)?, "じ");
        let DocKind::Anno { base, ref notes } = actual.value.nodes[inlines[1].0 as usize].kind
        else {
            return Err("preserved Anno".into());
        };
        assert_eq!(text(base)?, "base");
        assert_eq!(notes.len(), 1);
        assert_eq!(text(notes[0])?, "note");
        Ok(())
    })
}

#[test]
fn typed_list_construction_preserves_order_and_inline_structure() -> Result<(), String> {
    let compiled = compiled()?;
    // This source only establishes the host Profile. The tested document is
    // constructed below without assembling or parsing a source template.
    with_input(&compiled, "sentence nil", "Sentence", |_, profile, _, _| {
        let kinds = vec![
            DocKind::Text { text: "字".into() },
            DocKind::Text { text: "じ".into() },
            DocKind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            DocKind::Text {
                text: "note".into(),
            },
            DocKind::Anno {
                base: InlineRef(2),
                notes: vec![InlineRef(3)],
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(4)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(5)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(6)],
            },
            DocKind::ListItem {
                checked: None,
                body: BodyRef(7),
            },
            DocKind::Text {
                text: "second".into(),
            },
            DocKind::Link {
                target: LinkTarget::Relative {
                    path: "guide.md".into(),
                    fragment: Some("start".into()),
                },
                label: InlineRef(9),
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(10)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(11)],
            },
            DocKind::Body {
                blocks: vec![BlockRef(12)],
            },
            DocKind::ListItem {
                checked: Some(false),
                body: BodyRef(13),
            },
            DocKind::List {
                kind: ListKind::Unordered,
                items: vec![ListItemRef(8), ListItemRef(14)],
            },
        ];
        let document = DocumentSyntax {
            value: DocValue {
                root: DocRoot::Block(BlockRef(15)),
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
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        for mode in [PrintMode::Prefix, PrintMode::Compact] {
            let request = PrintRequest {
                document: document.clone(),
                mode,
                bindings: vec![],
                guests: vec![],
            };
            let reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let PrintOutcome::Complete { artifact } = reply.outcome else {
                return Err(format!("typed list print: {reply:?}"));
            };
            let (surface, category) = entry(artifact.entry);
            with_input(&compiled, &artifact.text, surface, |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let actual = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    category,
                    profile.registry(),
                    &mut budget(),
                    &mut codec,
                )
                .map_err(err)?;
                // The constructor fixture is already in postorder normal form.
                // Its fixed references are an independent oracle: computing
                // both sides through normalization could hide a shared defect.
                // Every edge, item order, annotation and link target is checked.
                assert_eq!(actual.value.root, document.value.root);
                assert_eq!(
                    actual
                        .value
                        .nodes
                        .iter()
                        .map(|n| &n.kind)
                        .collect::<Vec<_>>(),
                    document
                        .value
                        .nodes
                        .iter()
                        .map(|n| &n.kind)
                        .collect::<Vec<_>>()
                );
                assert!(!actual.sources.is_empty());
                Ok(())
            })?;
            // Printing does not invent source positions for typed constructors.
            assert_eq!(request.document, document);
            assert!(document.sources.is_empty());
            assert!(
                document
                    .value
                    .nodes
                    .iter()
                    .all(|n| n.span.is_none() && n.origin.is_none())
            );
        }
        Ok(())
    })
}

#[test]
fn printer_expands_shared_paths_with_bounded_output_and_deep_cleanup() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(&compiled, "sentence nil", "Sentence", |_, profile, _, _| {
        let node = |kind| DocNode {
            locations: vec![],
            kind,
            origin: None,
            span: None,
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        for (depth, shared) in [(512, false), (20, true)] {
            let mut nodes = vec![node(DocKind::Text { text: "x".into() })];
            for id in 0..depth {
                nodes.push(node(if shared {
                    DocKind::Concat {
                        inlines: vec![InlineRef(id), InlineRef(id)],
                    }
                } else {
                    DocKind::Strong {
                        inline: InlineRef(id),
                    }
                }));
            }
            let document = DocumentSyntax {
                value: DocValue {
                    root: DocRoot::Inline(InlineRef(depth)),
                    nodes,
                    embeds: vec![],
                },
                sources: vec![],
                origins: vec![],
                views: vec![],
                source_maps: vec![],
            };
            for mode in [PrintMode::Prefix, PrintMode::Compact] {
                let request = PrintRequest {
                    document: document.clone(),
                    mode,
                    bindings: vec![],
                    guests: vec![],
                };
                let mut preparation = budget();
                preparation = Budget::new(Limits {
                    depth: 2000,
                    ..preparation.limits()
                });
                print::identity(&document, profile.registry(), &mut codec, &mut preparation)
                    .map_err(err)?;
                let preparation_output = preparation.usage().output_bytes;
                let mut limits = budget().limits();
                limits.depth = 2000;
                limits.output_bytes = if shared {
                    preparation_output + 1024
                } else {
                    1_000_000
                };
                let mut b = Budget::new(limits);
                let reply =
                    print::print(&request, profile.registry(), &mut codec, &mut b).map_err(err)?;
                if shared {
                    assert_eq!(
                        reply.outcome,
                        PrintOutcome::Stopped {
                            reason: StopReason::OutputLimit
                        }
                    );
                    assert_eq!(b.poll(), Err(StopReason::OutputLimit));
                    assert!(b.usage().output_bytes > preparation_output);
                    assert!(b.usage().output_bytes <= preparation_output + 1024);
                } else {
                    let PrintOutcome::Complete { artifact } = reply.outcome else {
                        return Err(format!("deep prefix unexpectedly stopped: {reply:?}"));
                    };
                    assert_eq!(artifact.text.matches("strong ").count(), depth as usize);
                    assert!(artifact.text.ends_with("text \"x\""));
                }
                assert_eq!(request.document, document);
                let mut limits = budget().limits();
                limits.depth = 8;
                let mut b = Budget::new(limits);
                assert_eq!(
                    print::print(&request, profile.registry(), &mut codec, &mut b)
                        .map_err(err)?
                        .outcome,
                    PrintOutcome::Stopped {
                        reason: StopReason::DepthLimit
                    }
                );
            }
        }
        Ok(())
    })
}
