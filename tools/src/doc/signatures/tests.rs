use super::*;
use crate::doc::source::{budget, compiled, err, with_input_route};
use nepl3_doc_core::{check, lower};

fn sections(doc: &DocumentSyntax) -> Result<Vec<(&str, BodyRef, SentenceRef)>> {
    let DocRoot::Article(root) = doc.value.root else {
        return Err("article root".into());
    };
    let DocKind::Article { body, .. } = doc.value.nodes[root.0 as usize].kind else {
        return Err("article".into());
    };
    let DocKind::Body { ref blocks } = doc.value.nodes[body.0 as usize].kind else {
        return Err("body".into());
    };
    blocks
        .iter()
        .skip(1)
        .map(|block| {
            let DocKind::Section { id, body, title } = &doc.value.nodes[block.0 as usize].kind
            else {
                return Err("section".into());
            };
            Ok((id.as_str(), *body, *title))
        })
        .collect()
}

fn contents(doc: &DocumentSyntax, registry: &SchemaRegistry) -> Result<Vec<SentenceSyntax>> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    doc.value
        .embeds
        .iter()
        .map(|embed| {
            let package = match embed.content {
                DocContent::Syntax { .. } => "nepl3.syntax.sentence",
                DocContent::Value { .. } => "nepl3.sentence",
            };
            let surface = registry.selected(package, 1).ok_or("Sentence schema")?;
            nepl3_suite::adapters::document::sentence::lower(
                embed,
                surface,
                &[],
                registry,
                &mut codec,
                &mut b,
            )
            .map_err(|error| err(error).into())
        })
        .collect()
}

fn sentence_value<'a>(
    doc: &DocumentSyntax,
    contents: &'a [SentenceSyntax],
    sentence: SentenceRef,
) -> Result<&'a SentenceValue> {
    let DocKind::Sentence { syntax } = doc.value.nodes[sentence.0 as usize].kind else {
        return Err("sentence".into());
    };
    Ok(&contents[syntax.0 as usize].value)
}
fn inlines(value: &SentenceValue) -> Result<&[SentenceInlineRef]> {
    let Root::Sentence(root) = value.root else {
        return Err("Sentence root".into());
    };
    let Kind::Sentence { inlines } = &value.nodes[root.0 as usize] else {
        return Err("Sentence node".into());
    };
    Ok(inlines)
}
fn single_text<'a>(
    doc: &DocumentSyntax,
    contents: &'a [SentenceSyntax],
    sentence: SentenceRef,
) -> Result<&'a str> {
    let value = sentence_value(doc, contents, sentence)?;
    let inlines = inlines(value)?;
    assert_eq!(inlines.len(), 1);
    match &value.nodes[inlines[0].0 as usize] {
        Kind::Text { text } | Kind::Code { text } => Ok(text),
        _ => Err("text or code".into()),
    }
}

fn check_leaf(
    doc: &DocumentSyntax,
    contents: &[SentenceSyntax],
    body: BodyRef,
    expected: Option<&str>,
) -> Result<()> {
    let DocKind::Body { ref blocks } = doc.value.nodes[body.0 as usize].kind else {
        return Err("body".into());
    };
    assert_eq!(blocks.len(), if expected.is_some() { 2 } else { 1 });
    if let Some(expected) = expected {
        let DocKind::Paragraph { ref items } = doc.value.nodes[blocks[1].0 as usize].kind else {
            return Err("paragraph".into());
        };
        assert_eq!(items.len(), 1);
        let value = sentence_value(doc, contents, SentenceRef(items[0].0))?;
        let inlines = inlines(value)?;
        assert_eq!(inlines.len(), 6);
        assert_eq!(
            value.nodes[inlines[4].0 as usize],
            Kind::Code {
                text: expected.into()
            }
        );
        for (slot, base_text, reading_text) in [(0, "葉", "は"), (2, "認識規則", "にんしききそく")]
        {
            let Kind::Ruby { base, reading } = value.nodes[inlines[slot].0 as usize] else {
                return Err("ruby".into());
            };
            assert_eq!(
                value.nodes[base.0 as usize],
                Kind::Text {
                    text: base_text.into()
                }
            );
            assert_eq!(
                value.nodes[reading.0 as usize],
                Kind::Text {
                    text: reading_text.into()
                }
            );
        }
    }
    Ok(())
}

fn table(doc: &DocumentSyntax, body: BodyRef) -> Result<(&[Alignment], &[RowRef])> {
    let DocKind::Body { ref blocks } = doc.value.nodes[body.0 as usize].kind else {
        return Err("body".into());
    };
    let DocKind::Table {
        columns,
        rows,
        header,
        ..
    } = &doc.value.nodes[blocks[0].0 as usize].kind
    else {
        return Err("table".into());
    };
    assert!(header.is_some());
    Ok((columns, rows))
}

fn cells<'a>(
    doc: &DocumentSyntax,
    contents: &'a [SentenceSyntax],
    row: RowRef,
) -> Result<Vec<&'a str>> {
    let DocKind::Row { ref cells } = doc.value.nodes[row.0 as usize].kind else {
        return Err("row".into());
    };
    cells
        .iter()
        .map(|cell| single_text(doc, contents, *cell))
        .collect()
}

#[test]
fn typed_tables_preserve_fields_and_survive_the_real_reader() -> Result<()> {
    let catalog: Catalog = serde_json::from_str(
        r#"{"categories":{"Doc/Example":{"leaf":"sentence","forms":{
        "q\"\\\n[]𠮷":{"kind":"Quoted","fields":[{"name":"z","read":{"list":{"list":"@Text"}}},{"name":"a","read":"Doc/Inline"}]},
        "empty":{"kind":"Empty","fields":[]}
    }}}}"#,
    )?;
    let original = document(&catalog, "Doc")?;
    let text = source(original.clone())?;
    let compiled = compiled()?;
    let parsed = with_input_route(true, &compiled, &text, "Article", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            check::Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)
    })?;
    for doc in [&original, &parsed] {
        let contents = contents(doc, &compiled.doc.registry)?;
        let sections = sections(doc)?;
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "category_446f632f4578616d706c65");
        assert_eq!(single_text(doc, &contents, sections[0].2)?, "Doc/Example");
        check_leaf(doc, &contents, sections[0].1, Some("sentence"))?;
        let (columns, rows) = table(doc, sections[0].1)?;
        assert_eq!(
            columns,
            &[
                Alignment::Left,
                Alignment::Left,
                Alignment::Left,
                Alignment::Right
            ]
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(
            cells(doc, &contents, rows[0])?,
            [
                "q\"\\\n[]𠮷",
                "Doc.Quoted",
                "z: List<List<@Text>>, a: Doc/Inline",
                "2"
            ]
        );
        assert_eq!(
            cells(doc, &contents, rows[1])?,
            ["empty", "Doc.Empty", "なし", "0"]
        );
    }
    Ok(())
}

#[test]
fn catalog_order_and_all_form_rows_are_preserved() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace")?;
    let catalog: Catalog = crate::json(root, "design/forms.json")?;
    let registry = registry(&mut budget())?;
    for language in LANGUAGES {
        let doc = document(&catalog, language)?;
        let contents = contents(&doc, &registry)?;
        doc.value.validate_shape(&mut budget()).map_err(err)?;
        let selected: Vec<_> = catalog
            .categories
            .0
            .iter()
            .filter(|(name, _)| name.starts_with(&format!("{language}/")))
            .collect();
        let sections = sections(&doc)?;
        assert_eq!(sections.len(), selected.len());
        for ((_, body, title), (name, category)) in sections.iter().zip(selected) {
            assert_eq!(single_text(&doc, &contents, *title)?, name);
            check_leaf(&doc, &contents, *body, category.leaf.as_deref())?;
            let (_, rows) = table(&doc, *body)?;
            let names = rows
                .iter()
                .map(|row| Ok(cells(&doc, &contents, *row)?[0]))
                .collect::<Result<Vec<_>>>()?;
            assert_eq!(
                names,
                category
                    .forms
                    .0
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
    Ok(())
}

#[test]
fn invalid_catalog_input_is_rejected() -> Result<()> {
    let files = crate::testing::Fixture::new()?;
    for input in [
        r#"{"categories":{},"categories":{}}"#,
        r#"{"categories":{"Doc/A":{},"Doc/A":{}}}"#,
        r#"{"categories":{"Doc/A":{"forms":{"x":{"kind":"X","fields":[{"name":"a","name":"b","read":"@Text"}]}}}}}"#,
    ] {
        files.write("design/forms.json", input)?;
        assert!(crate::json::<Catalog>(files.root(), "design/forms.json").is_err());
    }
    for input in [r#"{"list":"@Text","extra":true}"#, "1", "{}"] {
        assert!(serde_json::from_str::<ReadType>(input).is_err());
    }
    assert!(ReadType::Name(String::new()).label().is_err());
    let empty = Catalog {
        categories: Ordered(vec![]),
    };
    assert!(document(&empty, "Doc").is_err());
    assert!(document(&empty, "Unknown").is_err());
    Ok(())
}
