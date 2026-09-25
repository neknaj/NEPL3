use super::*;
use nepl3_doc_core::model::{DocContent, DocKind, DocRoot, ListKind};

#[test]
fn grammar_chapter_preserves_sections_binding_plans_and_code() -> Result<(), String> {
    project(budget().limits(), budget(), budget())
}

#[test]
#[ignore = "explicit Grammar chapter migration validation under corpus limits"]
fn measure_grammar_chapter_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    project(
        policy.output_limits.budget().limits(),
        policy.output_limits.budget(),
        policy.output_limits.budget(),
    )
}

fn project(
    parse_limits: nepl3_core::budget::Limits,
    mut lower_budget: Budget,
    mut render_budget: Budget,
) -> Result<(), String> {
    let c = compiled()?;
    let document = nepl3_tools::doc::source::with_named_validated_input_limits(
        true,
        &c,
        include_str!("../../../doc/spec/04-grammar.nepld"),
        "grammar-chapter",
        "Article",
        parse_limits,
        |tree, profile, parse_budget, _| {
            eprintln!("Grammar parse={:?}", parse_budget.usage());
            lower::prefix(
                &tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut lower_budget,
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("Grammar lower: {e:?}; {:?}", lower_budget.usage()))
        },
    )?;
    eprintln!("Grammar lower={:?}", lower_budget.usage());
    let node = |id: u64| {
        document
            .value
            .nodes
            .get(id as usize)
            .map(|n| &n.kind)
            .ok_or("node")
    };
    let DocRoot::Article(root) = document.value.root else {
        return Err("Article".into());
    };
    let DocKind::Article { body, .. } = node(root.0)? else {
        return Err("Article kind".into());
    };
    let mut pending = vec![(None, None, *body)];
    let mut sections = Vec::new();
    let mut lists = Vec::new();
    while let Some((parent, current, body)) = pending.pop() {
        let DocKind::Body { blocks } = node(body.0)? else {
            return Err("Body".into());
        };
        let mut children = Vec::new();
        for block in blocks {
            match node(block.0)? {
                DocKind::Section { id, body, .. } => {
                    children.push((current, Some(id.as_str()), *body))
                }
                DocKind::List { kind, items } => {
                    assert_eq!(*kind, ListKind::Unordered);
                    lists.push((current, items.len()));
                }
                DocKind::Paragraph { .. } | DocKind::RawCode { .. } => {}
                _ => return Err("unexpected Grammar block".into()),
            }
        }
        // Record each section when entering its body; preserve document order.
        if let Some(id) = current {
            sections.push((parent, id));
        }
        pending.extend(children.into_iter().rev());
    }
    assert_eq!(
        sections,
        [
            (None, "policy"),
            (None, "roots"),
            (None, "forms"),
            (None, "binding"),
            (Some("binding"), "stages"),
            (Some("binding"), "custom"),
            (None, "let_example"),
            (None, "styles"),
            (None, "compile"),
            (None, "syntax_environment"),
            (Some("syntax_environment"), "persistent_selection"),
            (None, "bootstrap"),
            (Some("bootstrap"), "withmode"),
            (None, "native_parse_state")
        ]
    );
    // Twelve BindingPlan cases and six compile rejection classes are authored
    // as separate list items, not derived from the renderer's output.
    assert_eq!(lists, [(Some("binding"), 12), (Some("compile"), 6)]);
    let raw_code: Vec<_> = document
        .value
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            DocKind::RawCode {
                language_hint,
                text,
            } => Some((language_hint.as_deref(), text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(
        raw_code,
        [(
            Some("text"),
            concat!(
                "form Let Expr \"let\"\n",
                "  cons field name builtin Name\n",
                "  cons field init local Expr\n",
                "  cons field body local Expr\n",
                "  nil\n",
                "  group\n",
                "    cons visit init\n",
                "    cons scope\n",
                "      cons bind Value name\n",
                "      cons visit body\n",
                "      nil\n",
                "    nil\n",
                "  cons style head \"marker\"\n",
                "  cons style field name \"name.definition\"\n",
                "  nil\n",
            )
        )]
    );
    let mut sentences = 0;
    for node in &document.value.nodes {
        if let DocKind::Sentence { syntax } = node.kind {
            sentences += 1;
            assert!(matches!(
                document.value.embeds[syntax.0 as usize].content,
                DocContent::Syntax { .. }
            ));
        }
    }
    assert_eq!(sentences, 400);
    let set = PageSet {
        pages: vec![PageDocument {
            registration: PageRegistration {
                id: "grammar".into(),
                source: "doc/spec/04-grammar.nepld".into(),
                route: "doc/spec/04-grammar.md".into(),
            },
            document,
        }],
        files: vec![],
    };
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let started = std::time::Instant::now();
    let artifact = nepl3_tools::doc::projection::annotated::pages::render_profiled(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
        &mut |stage, usage| {
            eprintln!(
                "Grammar projection stage={stage:?} elapsed={:?} usage={usage:?}",
                started.elapsed()
            );
        },
    )
    .map_err(|e| format!("Grammar projection: {e:?}; {:?}", render_budget.usage()))?;
    eprintln!("Grammar projection={:?}", render_budget.usage());
    let markdown = &artifact.pages[0].markdown;
    let codes: Vec<_> = Parser::new(markdown)
        .filter_map(|e| match e {
            Event::Code(code) => Some(code.into_string()),
            _ => None,
        })
        .collect();
    for expected in [
        "language name revision root declarations",
        "builtin Name/Text/Nat/Lang",
        "withmode Code (listof (foreign Guest Sentence))",
        "ValidatedParseTree::syntax",
    ] {
        assert!(
            codes.iter().any(|code| code == expected),
            "missing code {expected}"
        );
    }
    assert!(markdown.contains("cons visit init\n    cons scope\n      cons bind Value name\n"));
    assert!(links(markdown).is_empty());
    Ok(())
}
