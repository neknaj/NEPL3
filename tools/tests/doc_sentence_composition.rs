//! The standard host preserves Doc -> Sentence -> Doc -> Sentence ownership.
use nepl3_core::source::SourceStore;
use nepl3_doc_core::{check::Category, lower, model::*};
use nepl3_sentence_core::{lower::ForeignInlineForm, model::Kind};
use nepl3_suite::adapters::{document::sentence, sentence::document_guests};
use nepl3_tools::doc::source::{compiled, err, with_input_route};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn standard_host_preserves_nested_document_labels() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Title" body cons paragraph
        cons sentence sentence
          cons doc anchor target ruby text "漢字" text "かんじ"
          cons doc ref target text "reference"
          nil nil nil"#;
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            source,
            "Article",
            |tree, profile, b, a| {
                let store = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, a).map_err(err)?;
                let document = lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let surface = profile
                    .registry()
                    .selected("nepl3.syntax.sentence", 1)
                    .ok_or("Sentence surface")?;
                let forms = [ForeignInlineForm {
                    kind: "Form:DocumentInline",
                    guest_schema: &compiled.doc.package.schema,
                    guest_category: "Inline",
                }];
                let mut occurrences = Vec::new();
                for embed in &document.value.embeds {
                    let content =
                        sentence::lower(embed, surface, &forms, profile.registry(), &mut codec, b)
                            .map_err(err)?;
                    let selection = document_guests::collect(
                        &content,
                        &compiled.doc.package.schema,
                        profile.registry(),
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let (documents, selected) = selection.into_parts();
                    for occurrence in selected {
                        let nested = &documents[occurrence.document.index()];
                        let DocRoot::Inline(root) = nested.value.root else {
                            return Err("Doc Inline root".into());
                        };
                        let (name, label, anchor) = match &nested.value.nodes[root.0 as usize].kind
                        {
                            DocKind::Anchor { id, label } => (id, label, true),
                            DocKind::Reference { target, label } => (target, label, false),
                            other => return Err(format!("unexpected Doc node: {other:?}")),
                        };
                        let label = sentence::lower(
                            &nested.value.embeds[label.0 as usize],
                            surface,
                            &forms,
                            profile.registry(),
                            &mut codec,
                            b,
                        )
                        .map_err(err)?;
                        let nepl3_sentence_core::model::Root::Inline(root) = label.value.root
                        else {
                            return Err("Sentence Inline root".into());
                        };
                        if anchor {
                            let Kind::Ruby { base, reading } = &label.value.nodes[root.0 as usize]
                            else {
                                return Err("Ruby label".into());
                            };
                            assert!(
                                matches!(&label.value.nodes[base.0 as usize], Kind::Text { text } if text == "漢字")
                            );
                            assert!(
                                matches!(&label.value.nodes[reading.0 as usize], Kind::Text { text } if text == "かんじ")
                            );
                        } else {
                            assert!(
                                matches!(&label.value.nodes[root.0 as usize], Kind::Text { text } if text == "reference")
                            );
                        }
                        occurrences.push((name.clone(), anchor));
                    }
                }
                // Independent expectations identify both Doc occurrences, their order,
                // and the Sentence-owned label structure. Roundtrip alone is insufficient.
                assert_eq!(
                    occurrences,
                    [("target".into(), true), ("target".into(), false)]
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}
