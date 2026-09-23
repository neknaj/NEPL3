use super::*;
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_sentence_core::model::Kind;
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn math_annotation_uses_registered_sentence_on_both_reader_routes() -> Result<(), String> {
    let compiled = compiled()?;
    let sentence = compiled.others.last().ok_or("Sentence package")?;
    assert_eq!(sentence.root, "Sentence");
    // Ruby belongs to the independent Sentence value inside the Math closure.
    // Both routes must preserve that value and its original Unicode source.
    for (native, input) in [false, true].into_iter().flat_map(|native| {
        [
            r#"article en "Math" body cons display Math label x Sentence "[字/じ]" nil"#,
            r#"article en "Math" body cons display Math label x Sentence sentence cons ruby text "字" text "じ" nil nil"#,
        ].into_iter().map(move |input| (native, input))
    }) {
        with_input_route(
            native,
            &compiled,
            input,
            "Article",
            |tree, profile, b, a| {
                let checked = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let document = lower::document(
                    &checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let display = document
                    .value
                    .nodes
                    .iter()
                    .position(|node| matches!(node.kind, DocKind::DisplayMath { .. }))
                    .ok_or("display Math")?;
                let mut host = crate::doc::math::MathDisplayHost {
                    registry: profile.registry(),
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&sentence.schema),
                    codec: &mut codec,
                };
                let rendered = host
                    .render_node(&document, display as u64, b)
                    .map_err(err)?;
                assert_eq!(rendered.annotations.len(), 1);
                let annotation = &rendered.annotations[0];
                let ruby = annotation
                    .sentence
                    .value
                    .nodes
                    .iter()
                    .find_map(|node| match node {
                        Kind::Ruby { base, reading } => Some((*base, *reading)),
                        _ => None,
                    })
                    .ok_or("Ruby")?;
                assert_eq!(
                    annotation.sentence.value.nodes[ruby.0.0 as usize],
                    Kind::Text { text: "字".into() }
                );
                assert_eq!(
                    annotation.sentence.value.nodes[ruby.1.0 as usize],
                    Kind::Text { text: "じ".into() }
                );
                assert!(
                    annotation
                        .sentence
                        .sources
                        .iter()
                        .any(|source| source.text() == input)
                );
                assert!(!annotation.origins.is_empty());
                let shape = rendered.syntax.value.validate_shape(b).map_err(err)?;
                let mut printer = crate::doc::printing::SentenceGuestPrinter {
                    registry: profile.registry(), surface: &sentence.schema,
                    math_surface: None, codec: &mut codec,
                };
                let printed = nepl3_math_core::print::prefix(&shape, &mut printer, b)
                    .map_err(err)?;
                assert_eq!(printed.text,
                    "label symbol \"x\" Sentence sentence cons ruby text \"字\" text \"じ\" nil");
                let html = rendered.into_html(b).map_err(err)?;
                assert_eq!(html.annotations.len(), 1);
                assert!(!html.annotations[0].origins.is_empty());
                Ok(())
            },
        )?;
    }
    Ok(())
}
