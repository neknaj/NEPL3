use super::*;
use nepl3_doc_core::model::{DocKind, DocRoot};

#[test]
fn model_invariants_preserve_structure_constraints_and_link() -> Result<(), String> {
    project(budget())
}

#[test]
#[ignore = "explicit model invariants projection measurement under corpus limits"]
fn measure_model_invariants_under_corpus_limits() -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct Policy {
        output_limits: nepl3_tools::doc::export::pages::resources::OutputLimits,
    }
    let policy: Policy =
        serde_json::from_str(include_str!("../../../doc/canonical.json")).map_err(err)?;
    project(policy.output_limits.budget())
}

fn project(mut render_budget: Budget) -> Result<(), String> {
    let c = compiled()?;
    let set = PageSet {
        pages: vec![page(
            &c,
            "model-invariants",
            "doc/spec/12-model-invariants.nepld",
            "doc/spec/12-model-invariants.md",
            include_str!("../../../doc/spec/12-model-invariants.nepld"),
        )?],
        // The migration preserves the original explicit Markdown destination.
        // Its content is passive input; no unrelated chapter is parsed here.
        files: vec![PageFile {
            registration: PageRegistration {
                id: "html-fragment".into(),
                source: "doc/spec/19-html-fragment.md".into(),
                route: "doc/spec/19-html-fragment.md".into(),
            },
            content: FileBytes(include_bytes!("../../../doc/spec/19-html-fragment.md").to_vec()),
        }],
    };
    let document = &set.pages[0].document.value;
    let node = |id: u64| -> Result<&DocKind, String> {
        document
            .nodes
            .get(usize::try_from(id).map_err(err)?)
            .map(|node| &node.kind)
            .ok_or_else(|| "document node".into())
    };
    let DocRoot::Article(root) = document.root else {
        return Err("Article root".into());
    };
    let DocKind::Article { body, .. } = node(root.0)? else {
        return Err("Article".into());
    };
    let DocKind::Body { blocks } = node(body.0)? else {
        return Err("Article Body".into());
    };
    let mut sections = Vec::new();
    let mut paragraphs = Vec::new();
    let mut tables = 0;
    for block in blocks {
        let DocKind::Section { id, body, .. } = node(block.0)? else {
            return Err("Section".into());
        };
        sections.push(id.as_str());
        let DocKind::Body { blocks } = node(body.0)? else {
            return Err("Section Body".into());
        };
        for block in blocks {
            match node(block.0)? {
                DocKind::Paragraph { items } => paragraphs.push(items.len()),
                DocKind::Table { .. } => tables += 1,
                _ => return Err("chapter block".into()),
            }
        }
    }
    // Independent expectations from the authored chapter: seven sections,
    // thirty-two paragraphs with their original Sentence boundaries, one table.
    assert_eq!(
        sections,
        [
            "policy",
            "values",
            "constraints",
            "checked",
            "circuit",
            "markup",
            "operations"
        ]
    );
    assert_eq!(
        paragraphs,
        [
            3, 10, 4, 6, 5, 7, 4, 4, 4, 6, 3, 6, 4, 3, 2, 4, 8, 4, 4, 2, 4, 4, 6, 2, 4, 8, 4, 3, 4,
            3, 3, 3
        ]
    );
    assert_eq!(tables, 1);

    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let artifact = render(
        &set,
        &c.doc.registry,
        &mut codec,
        &mut render_budget,
        &[&[]],
    )
    .map_err(|e| {
        format!(
            "model invariants projection: {e:?}; {:?}",
            render_budget.usage()
        )
    })?;
    eprintln!(
        "model invariants projection usage={:?}",
        render_budget.usage()
    );
    let markdown = &artifact.pages[0].markdown;
    assert_eq!(links(markdown), ["19-html-fragment.md"]);
    assert!(code_blocks(markdown)?.is_empty());
    assert!(markdown.contains("<ruby>補足不変条件<rt>ほそくふへんじょうけん</rt></ruby>"));
    let mut codes = Vec::new();
    let mut rows = 0;
    let mut cells = Vec::new();
    let mut cell = None;
    for event in Parser::new_ext(markdown, pulldown_cmark::Options::ENABLE_TABLES) {
        match event {
            Event::Code(text) => codes.push(text.into_string()),
            Event::Start(Tag::Table(alignments)) => {
                assert_eq!(alignments, [pulldown_cmark::Alignment::None; 2])
            }
            Event::Start(Tag::TableRow) => rows += 1,
            Event::Start(Tag::TableCell) => cell = Some(String::new()),
            Event::Text(text) if cell.is_some() => cell.as_mut().ok_or("cell")?.push_str(&text),
            Event::End(pulldown_cmark::TagEnd::TableCell) => {
                cells.push(cell.take().ok_or("cell end")?)
            }
            _ => {}
        }
    }
    assert_eq!(rows, 17);
    assert_eq!(cells.len(), 36);
    assert_eq!(
        cells
            .iter()
            .skip(2)
            .step_by(2)
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "source.identity",
            "source.content",
            "source.bundle",
            "source.span",
            "schema.reference / schema.type-reference",
            "schema.descriptor",
            "operation.reporting",
            "report.trace-overflow",
            "namespace.reference",
            "environment.bindings / environment.digest",
            "syntax.graph / syntax.foreign",
            "source.map / origin.graph",
            "source.reservation",
            "schema.kind-id",
            "view.graph",
            "token.boundary",
            "view.presentation",
        ]
    );
    assert_eq!(
        codes,
        [
            "interfaces/model.json",
            "record",
            "[fieldName, typeExpression]",
            "sum",
            "union",
            "NEPL3-ENVIRONMENT-1",
            "design/markup.json",
            "design/markup.json",
            "interfaces/contracts.json"
        ]
    );
    Ok(())
}
