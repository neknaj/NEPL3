use super::*;
use nepl3_doc_core::print::{
    self, PrintEntry, PrintFailure, PrintMismatch, PrintMode, PrintOutcome, PrintRequest,
};

#[path = "print/host.rs"]
mod host;
use entry::entry;
use host::{guest_signature, host_request};
#[path = "print/entry.rs"]
mod entry;
use nepl3_doc_core::{
    check::{Category, ShapeError},
    lower,
    model::*,
};

#[test]
fn printer_requires_explicit_current_guest_print_and_binding_at_first_receiver()
-> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "sentence cons math Math add 1 2 cons math Math add 3 4 nil",
        "Sentence",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let request = host_request(&doc, profile, PrintMode::Prefix)?;
            assert_eq!(request.guests.len(), 2);
            let mut cases = Vec::new();
            let mut changed = request.clone();
            changed.bindings.clear();
            cases.push((changed, PrintFailure::MissingBinding { embed: EmbedRef(0) }));
            let mut changed = request.clone();
            changed.bindings.push(changed.bindings[0].clone());
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 4 }));
            let mut changed = request.clone();
            let mut conflicting = changed.bindings[0].clone();
            conflicting.schema.digest = nepl3_core::source::Digest([0; 32]);
            changed.bindings.push(conflicting);
            // One surface alias cannot select two different schemas in one Profile.
            cases.push((changed, PrintFailure::ConflictingBinding { binding: 4 }));
            let mut changed = request.clone();
            changed.bindings[0].category = "Design".into();
            cases.push((changed, PrintFailure::InvalidBinding { binding: 0 }));
            let mut changed = request.clone();
            changed.bindings[0].language = GuestLanguage::Circuit;
            changed.bindings[0].category = "Design".into();
            changed.bindings.remove(1);
            cases.push((changed, PrintFailure::GuestCategory { embed: EmbedRef(0) }));
            let mut changed = request.clone();
            changed.guests.clear();
            // A real retained source cover does not authorize automatic guest printing.
            cases.push((
                changed,
                PrintFailure::UnresolvedGuest { embed: EmbedRef(0) },
            ));
            for (reason, mode) in [
                (PrintMismatch::Document, 0),
                (PrintMismatch::Guest, 1),
                (PrintMismatch::Embed, 2),
                (PrintMismatch::Embed, 3),
            ] {
                let mut changed = request.clone();
                match mode {
                    0 => changed.guests[0].document_digest = nepl3_core::source::Digest([0; 32]),
                    1 => changed.guests[0].guest_digest = changed.guests[1].guest_digest,
                    2 => changed.guests[0].embed = EmbedRef(u64::MAX),
                    _ => changed.guests[0].embed = EmbedRef(1_u64 << 32),
                }
                cases.push((changed, PrintFailure::InvalidGuest { entry: 0, reason }));
            }
            let mut changed = request.clone();
            changed.guests.push(changed.guests[0].clone());
            cases.push((
                changed,
                PrintFailure::InvalidGuest {
                    entry: 2,
                    reason: PrintMismatch::Duplicate,
                },
            ));
            for (request, error) in cases {
                let wire = nepl3_doc_core::portable::print::request_to_value(
                    &request,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&wire, &mut budget()).map_err(err)?;
                let mut a = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                let actual = nepl3_doc_core::portable::print::request_from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    profile.registry(),
                    &mut receiver,
                    &mut budget(),
                )
                .map_err(err)?;
                let reply = print::print(&actual, profile.registry(), &mut receiver, &mut budget())
                    .map_err(err)?;
                assert_eq!(reply.outcome, PrintOutcome::Invalid { error });
                assert_eq!(request, actual);
            }
            let mut synthetic = request.clone();
            let identity = print::identity(&doc, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let identity_value = nepl3_doc_core::portable::print::identity_to_value(
                &identity,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            for length in [31, 33] {
                let mut malformed = identity_value.clone();
                let NdfValue::Record(record) = &mut malformed else {
                    return Err("identity record".into());
                };
                record.fields[0] = NdfValue::Bytes(vec![0; length]);
                assert!(
                    nepl3_doc_core::portable::print::identity_from_value(
                        &malformed,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .is_err()
                );
            }
            let mut reply = print::print(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
            let complete_value = nepl3_doc_core::portable::print::reply_to_value(
                &reply,
                &doc,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            reply.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert!(matches!(
                nepl3_doc_core::portable::print::reply_to_value(
                    &reply,
                    &doc,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(nepl3_doc_core::portable::PortableError::Shape)
            ));
            for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
                reply.outcome = PrintOutcome::Stopped { reason };
                let value = nepl3_doc_core::portable::print::reply_to_value(
                    &reply,
                    &doc,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(
                    nepl3_doc_core::portable::print::reply_from_value(
                        &value,
                        &doc,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .map_err(err)?,
                    reply
                );
                let mut malformed = complete_value.clone();
                let (NdfValue::Record(record), NdfValue::Record(stopped)) =
                    (&mut malformed, &value)
                else {
                    return Err("reply record".into());
                };
                record.fields[1] = stopped.fields[1].clone();
                assert!(matches!(
                    nepl3_doc_core::portable::print::reply_from_value(
                        &malformed,
                        &doc,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(nepl3_doc_core::portable::PortableError::Shape)
                ));
            }
            let bundle = &mut synthetic.document.value.embeds[0].closure.syntax.bundle;
            let root = bundle.root.0 as usize;
            bundle.nodes[root].cover = None;
            bundle.nodes[root].head = None;
            assert!(
                print::original_guest_source(
                    &synthetic.document,
                    EmbedRef(0),
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .map_err(err)?
                .is_none()
            );
            // A host can print a synthetic root explicitly, but the removed
            // cover invalidates the previously issued document identity.
            assert_eq!(
                print::print(&synthetic, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?
                    .outcome,
                PrintOutcome::Invalid {
                    error: PrintFailure::InvalidGuest {
                        entry: 0,
                        reason: PrintMismatch::Document
                    }
                }
            );
            let current = print::identity(
                &synthetic.document,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            for (guest, target) in synthetic.guests.iter_mut().zip(&current.guests) {
                guest.document_digest = current.document_digest;
                guest.guest_digest = target.guest_digest;
            }
            assert!(matches!(
                print::print(&synthetic, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?
                    .outcome,
                PrintOutcome::Complete { .. }
            ));
            Ok(())
        },
    )
}

#[test]
fn fragment_preserves_shared_foreign_syntax_and_rejects_invalid_inputs() -> Result<(), String> {
    use nepl3_doc_core::check::StructureError;
    let compiled = compiled()?;
    with_input(
        &compiled,
        "sentence cons math Math add 1 2 cons math Math add 3 4 cons math Math add 5 6 nil",
        "Sentence",
        |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let mut doc = lower::document(
                &checked,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let DocRoot::Sentence(sentence) = doc.value.root else {
                return Err("sentence root".into());
            };
            let DocKind::Sentence { ref inlines } = doc.value.nodes[sentence.0 as usize].kind
            else {
                return Err("sentence".into());
            };
            let selected = inlines[1];
            let DocKind::InlineMath { syntax } = doc.value.nodes[selected.0 as usize].kind else {
                return Err("math".into());
            };
            let retained = doc.value.embeds[syntax.0 as usize].clone();
            let selected_node = doc.value.nodes[selected.0 as usize].clone();
            let shared_embed = InlineRef(doc.value.nodes.len() as u64);
            doc.value.nodes.push(selected_node.clone());
            let concat = InlineRef(doc.value.nodes.len() as u64);
            doc.value.nodes.push(DocNode {
                kind: DocKind::Concat {
                    inlines: vec![selected, shared_embed, selected],
                },
                locations: vec![],
                origin: None,
                span: None,
            });
            let DocKind::Sentence { ref mut inlines } = doc.value.nodes[sentence.0 as usize].kind
            else {
                return Err("sentence".into());
            };
            inlines[1] = concat;
            let before = doc.clone();
            let root = DocRoot::Inline(concat);
            let mut complete = budget();
            let result = doc
                .fragment(
                    root,
                    profile.registry(),
                    &mut complete,
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
            assert_eq!(doc, before);
            assert_eq!(result.value.root, DocRoot::Inline(InlineRef(2)));
            assert_eq!(result.value.nodes.len(), 3);
            let mut expected_node = selected_node;
            expected_node.kind = DocKind::InlineMath {
                syntax: EmbedRef(0),
            };
            assert_eq!(result.value.nodes[0], expected_node);
            assert_eq!(result.value.nodes[1], expected_node);
            assert_eq!(
                result.value.nodes[2].kind,
                DocKind::Concat {
                    inlines: vec![InlineRef(0), InlineRef(1), InlineRef(0)]
                }
            );
            assert_eq!(result.value.embeds, vec![retained]);
            assert_eq!(result.sources, doc.sources);
            assert_eq!(result.origins, doc.origins);
            assert_eq!(result.views, doc.views);
            assert_eq!(result.source_maps, doc.source_maps);
            for (invalid, expected) in [
                (
                    DocRoot::Inline(InlineRef(u64::MAX)),
                    ShapeError::Reference(u64::MAX),
                ),
                (
                    DocRoot::Article(ArticleRef(concat.0)),
                    ShapeError::Category {
                        node: concat.0,
                        expected: Category::Article,
                    },
                ),
            ] {
                assert_eq!(
                    doc.fragment(
                        invalid,
                        profile.registry(),
                        &mut budget(),
                        &mut SourceAdmission::default()
                    ),
                    Err(StructureError::Shape(expected))
                );
            }
            let mut malformed = doc.clone();
            malformed.value.nodes.push(DocNode {
                kind: DocKind::Text {
                    text: "unreachable".into(),
                },
                locations: vec![],
                origin: None,
                span: None,
            });
            assert_eq!(
                malformed.fragment(
                    root,
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default()
                ),
                Err(StructureError::Shape(ShapeError::Unreachable(
                    doc.value.nodes.len() as u64
                )))
            );
            for (limits, reason) in [
                (
                    Limits {
                        work: complete.usage().work - 1,
                        ..budget().limits()
                    },
                    StopReason::WorkLimit,
                ),
                (
                    Limits {
                        allocation_units: complete.usage().allocation_units - 1,
                        ..budget().limits()
                    },
                    StopReason::AllocationLimit,
                ),
                (
                    Limits {
                        work: 0,
                        ..budget().limits()
                    },
                    StopReason::WorkLimit,
                ),
                (
                    Limits {
                        allocation_units: 0,
                        ..budget().limits()
                    },
                    StopReason::AllocationLimit,
                ),
                (
                    Limits {
                        depth: 1,
                        ..budget().limits()
                    },
                    StopReason::DepthLimit,
                ),
            ] {
                let mut limited = Budget::new(limits);
                assert_eq!(
                    doc.fragment(
                        root,
                        profile.registry(),
                        &mut limited,
                        &mut SourceAdmission::default()
                    ),
                    Err(StructureError::Stopped(reason))
                );
                assert_eq!(doc, before);
            }
            Ok(())
        },
    )
}

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
