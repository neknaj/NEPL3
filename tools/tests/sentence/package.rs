use super::*;
use nepl3_core::source::Digest;
use nepl3_core::value_codec::FoundationValueCodec;

#[test]
fn sentence_surface_compiles_from_production_grammar_with_own_root_and_payload()
-> Result<(), String> {
    let mut budget = b();
    let mut admission = SourceAdmission::default();
    // The existing checked Grammar seed is a bootstrap artifact, not a Doc dependency.
    let seed = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/doc/grammar.json"),
        &mut budget,
        &mut admission,
    )
    .map_err(err)?;
    let grammar = nepl3_tools::bootstrap::catalog::compile(
        &seed,
        "nepl3.syntax.grammar",
        &mut budget,
        &mut admission,
    )?;
    let source = SourceSnapshot::new(
        SourceId("sentence-grammar".into()),
        1,
        "repository:languages/Sentence/syntax.neplg".into(),
        include_bytes!("../../../languages/Sentence/syntax.neplg").to_vec(),
        &mut budget,
    )
    .map_err(err)?;
    // This in-process fixture binds the selected adapter sources on both native
    // and WASI. It does not claim to identify a deployed executable: WASI has no
    // current_exe, and obtaining a binary identity is the deployment host's job.
    let identity = Digest::of(
        concat!(
            include_str!("package.rs"),
            include_str!("../../src/bootstrap/runtime.rs"),
            include_str!("../../src/bootstrap/runtime/host.rs"),
            include_str!("../../src/sentence/source.rs"),
            include_str!("../../src/sentence/reader.rs"),
            include_str!("../../src/source/driver.rs"),
            include_str!("../../src/source/host.rs"),
            include_str!("../../src/source/host/dispatch.rs"),
            include_str!("../../../crates/foundation/reader/src/builtin/provider.rs")
        )
        .as_bytes(),
    );
    let document = nepl3_tools::bootstrap::runtime::parse(
        &source,
        &grammar,
        identity,
        &mut budget,
        &mut admission,
    )
    .map_err(|e| format!("Sentence grammar: {e:?}; {:?}", budget.usage()))?;
    let compiled = nepl3_tools::sentence::catalog::compile(
        &document,
        "nepl3.syntax.sentence",
        &mut budget,
        &mut admission,
    )?;
    compiled
        .package
        .check(&compiled.registry, &mut budget)
        .map_err(err)?;
    assert_eq!(compiled.package.root, "Sentence");
    assert!(compiled.registry.selected("nepl3.doc", 1).is_none());
    assert!(compiled.registry.selected("nepl3.math", 1).is_none());
    assert!(compiled.package.namespaces.is_empty());
    let expected = [
        ("sentence", &["inlines"][..]),
        ("text", &["text"][..]),
        ("concat", &["inlines"][..]),
        ("ruby", &["base", "reading"][..]),
        ("anno", &["base", "notes"][..]),
        ("code", &["text"][..]),
        ("em", &["inline"][..]),
        ("strong", &["inline"][..]),
        ("break", &[][..]),
        ("link", &["uri", "label"][..]),
    ];
    assert_eq!(compiled.package.forms.len(), expected.len());
    for (spelling, fields) in expected {
        let form = compiled
            .package
            .forms
            .iter()
            .find(|f| f.spelling == spelling)
            .ok_or("form")?;
        assert_eq!(
            form.fields
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            fields
        );
        assert_eq!(
            form.category,
            if spelling == "sentence" {
                "Sentence"
            } else {
                "Inline"
            }
        );
    }
    let [leaf] = compiled.package.leaves.as_slice() else {
        return Err("one literal leaf".into());
    };
    assert_eq!(leaf.category, "Sentence");
    assert!(matches!(&leaf.payload, TypeDescriptor::Named(t)
        if t.package == "nepl3.sentence" && t.name == "SentenceLiteralPayload"));
    assert_eq!(compiled.package.reader.providers.len(), 2);
    assert!(
        !compiled
            .package
            .reader
            .providers
            .iter()
            .any(|p| p.operation.name.contains("trivia"))
    );
    use nepl3_sentence_core::model::{InlineRef, Kind, Root, SentenceRef, SentenceValue};
    let escaped = "[not ruby]{not anno}/\"\\\n\r\t\0\u{85}😀";
    let value = SentenceValue {
        root: Root::Sentence(SentenceRef(10)),
        embeds: vec![],
        nodes: vec![
            Kind::Text {
                text: escaped.into(),
            },
            Kind::Code {
                text: "code".into(),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            Kind::InlineAnno {
                base: InlineRef(2),
                notes: vec![InlineRef(1), InlineRef(0)],
            },
            Kind::Emphasis {
                inline: InlineRef(3),
            },
            Kind::Strong {
                inline: InlineRef(4),
            },
            Kind::Break,
            Kind::ExternalLink {
                uri: "https://example.invalid/".into(),
                label: InlineRef(5),
            },
            Kind::Concat {
                inlines: vec![InlineRef(6), InlineRef(7)],
            },
            Kind::Concat { inlines: vec![] },
            Kind::Sentence {
                inlines: vec![InlineRef(8), InlineRef(9)],
            },
        ],
    };
    let printed = nepl3_sentence_core::print::prefix(&value, &mut b()).map_err(err)?;
    for input in [
        "\"[漢/かん]{語/note}\"",
        "sentence cons ruby text \"漢\" text \"かん\" cons anno text \"語\" cons text \"note\" nil nil",
        "sentence cons concat cons text \"a\" cons text \"b\" nil cons code \"x\" cons em text \"e\" cons strong text \"s\" cons break cons link \"https://example.invalid/\" text \"link\" nil",
        "sentence nil\n",
        printed.as_str(),
        "sentence cons ruby text \"\" text \"reading\" nil",
        "sentence cons anno text \"base\" nil nil",
    ] {
        let source = SourceSnapshot::new(
            SourceId("sentence-input".into()),
            1,
            "memory:sentence-input".into(),
            input.as_bytes().to_vec(),
            &mut b(),
        )
        .map_err(err)?;
        let mut trees = vec![];
        for native in [false, true] {
            let tree = nepl3_tools::sentence::source::with_tree(
                &compiled,
                &source,
                identity,
                &mut b(),
                &mut SourceAdmission::default(),
                native,
                |tree, resolved, budget, admission| {
                    assert!(!tree.is_recovered());
                    let bundle = tree.syntax().bundle();
                    if input == printed {
                        // Decode through the real BuiltinText reader, not a
                        // printer-owned inverse. Delimiters stay ordinary Text.
                        assert!(bundle.tokens.iter().any(
                            |t| matches!(&t.payload, NdfValue::Text(text) if text == escaped)
                        ));
                    }
                    let root = &bundle.nodes[bundle.root.0 as usize];
                    assert_eq!(
                        root.kind,
                        if input.starts_with('"') {
                            "Leaf:SentenceLiteral"
                        } else {
                            "Form:Sentence"
                        }
                    );
                    let mut wrong_surface = root.schema.clone();
                    wrong_surface.digest = nepl3_core::source::Digest([0; 32]);
                    assert!(matches!(
                        nepl3_sentence_core::lower::prefix(
                            tree.syntax(),
                            &wrong_surface,
                            resolved.registry(),
                            &mut b(),
                            &mut SourceAdmission::default()
                        ),
                        Err(nepl3_sentence_core::lower::Error::Unsupported(_))
                    ));
                    if !input.starts_with('"') {
                        let projection = nepl3_sentence_core::lower::prefix(
                            tree.syntax(),
                            &root.schema,
                            resolved.registry(),
                            budget,
                            admission,
                        );
                        if input.contains("ruby text \"\"")
                            || input.contains("anno text \"base\" nil")
                        {
                            // Prefix shape is valid, but the Sentence domain
                            // rejects empty Ruby content and a missing note.
                            assert!(matches!(
                                projection,
                                Err(nepl3_sentence_core::lower::Error::Shape(_))
                            ));
                            return Ok(tree.tree().clone());
                        }
                        let projection = projection.map_err(err)?;
                        let empty = SourceStore::default();
                        let mut codec =
                            FoundationCodec::new(resolved.registry(), &empty, admission)
                                .map_err(err)?;
                        let syntax = nepl3_sentence_core::lower::presentation::sentence(
                            tree.syntax(),
                            &root.schema,
                            resolved.registry(),
                            &mut codec,
                            budget,
                        )
                        .map_err(err)?;
                        assert_eq!(syntax.value, projection.value);
                        assert_eq!(syntax.sources, bundle.sources);
                        assert_eq!(syntax.origins, bundle.origins);
                        assert_eq!(syntax.source_maps, bundle.source_maps);
                        assert_eq!(syntax.views.len(), bundle.tokens.len());
                        let doc = nepl3_tools::doc::sentence::document(
                            &syntax,
                            resolved.registry(),
                            budget,
                            codec.source_admission(),
                        )
                        .map_err(err)?;
                        assert_eq!(doc.sources, syntax.sources);
                        assert_eq!(doc.origins, syntax.origins);
                        assert_eq!(doc.source_maps, syntax.source_maps);
                        assert_eq!(doc.value.nodes.len(), syntax.value.nodes.len());
                        if input == printed {
                            use nepl3_doc_core::model::{DocKind, LinkTarget};
                            assert!(doc.value.nodes.iter().any(
                                |n| matches!(&n.kind,DocKind::InlineCode { text } if text == "code")
                            ));
                            assert!(
                                doc.value
                                    .nodes
                                    .iter()
                                    .any(|n| matches!(n.kind, DocKind::Break))
                            );
                            assert!(
                                doc.value
                                    .nodes
                                    .iter()
                                    .any(|n| matches!(n.kind, DocKind::Emphasis { .. }))
                            );
                            assert!(
                                doc.value
                                    .nodes
                                    .iter()
                                    .any(|n| matches!(n.kind, DocKind::Strong { .. }))
                            );
                            assert!(doc.value.nodes.iter().any(|n| matches!(&n.kind,DocKind::Link { target:LinkTarget::External { uri }, .. } if uri == "https://example.invalid/")));
                        }
                        if input.starts_with("sentence cons ruby text \"漢\"") {
                            assert_eq!(
                                nepl3_sentence_core::literal::print(&syntax.value, budget)
                                    .map_err(err)?,
                                "\"[漢/かん]{語/note}\""
                            );
                        }
                        let output = nepl3_sentence_core::print::prefix(&projection.value, budget)
                            .map_err(err)?;
                        assert_eq!(output, input.trim_end());
                        assert_eq!(projection.syntax_to_meaning.len(), bundle.nodes.len());
                        assert!(projection.syntax_to_meaning[bundle.root.0 as usize].is_some());
                        let mut limits = b().limits();
                        limits.work = 0;
                        assert_eq!(
                            nepl3_sentence_core::lower::prefix(
                                tree.syntax(),
                                &root.schema,
                                resolved.registry(),
                                &mut Budget::new(limits),
                                &mut SourceAdmission::default()
                            )
                            .err(),
                            Some(nepl3_sentence_core::lower::Error::Stopped(
                                StopReason::WorkLimit
                            ))
                        );
                    } else {
                        assert!(matches!(
                            nepl3_sentence_core::lower::prefix(
                                tree.syntax(),
                                &root.schema,
                                resolved.registry(),
                                budget,
                                admission
                            ),
                            Err(nepl3_sentence_core::lower::Error::Unsupported(_))
                        ));
                        let empty = SourceStore::default();
                        let mut codec =
                            FoundationCodec::new(resolved.registry(), &empty, admission)
                                .map_err(err)?;
                        let sentence = nepl3_sentence_core::lower::presentation::sentence(
                            tree.syntax(),
                            &root.schema,
                            resolved.registry(),
                            &mut codec,
                            budget,
                        )
                        .map_err(err)?;
                        assert_eq!(
                            nepl3_sentence_core::literal::print(&sentence.value, budget)
                                .map_err(err)?,
                            input
                        );
                        assert_eq!(sentence.sources[0], source);
                        let mut tampered = bundle.clone();
                        let token = tampered.nodes[tampered.root.0 as usize]
                            .token
                            .ok_or("literal token")?;
                        tampered.tokens[token.0 as usize].views.elements.clear();
                        tampered.tokens[token.0 as usize].views.roots.clear();
                        // Both views are independently valid, but only the
                        // original one belongs to this literal's reader result.
                        let changed = tampered
                            .validate(resolved.registry(), budget)
                            .map_err(err)?;
                        assert!(matches!(
                            nepl3_sentence_core::lower::literal::sentence(
                                &changed,
                                &root.schema,
                                resolved.registry(),
                                &mut codec,
                                budget
                            ),
                            Err(nepl3_sentence_core::lower::literal::Error::TokenMismatch(_))
                        ));
                    }
                    Ok(tree.tree().clone())
                },
            )?;
            trees.push(tree);
        }
        assert_eq!(trees[0], trees[1]);
    }
    for input in [
        "# old comment\nsentence nil",
        "sentence cons break",
        "sentence nil extra",
        "comment \"unowned\"",
    ] {
        let source = SourceSnapshot::new(
            SourceId("invalid-sentence".into()),
            1,
            "memory:invalid-sentence".into(),
            input.as_bytes().to_vec(),
            &mut b(),
        )
        .map_err(err)?;
        assert!(
            nepl3_tools::sentence::source::with_tree(
                &compiled,
                &source,
                identity,
                &mut b(),
                &mut SourceAdmission::default(),
                true,
                |_, _, _, _| Ok(())
            )
            .is_err(),
            "{input}"
        );
    }
    Ok(())
}
