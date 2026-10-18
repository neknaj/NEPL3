use super::*;
use nepl3_doc_core::labels::namespace::{self, MemberId};
use nepl3_suite::adapters::sentence::document_guests::{self as guests, OwnerId, Placement};

#[test]
fn placed_sentence_guests_resolve_forward_names_and_reject_duplicate_placements()
-> Result<(), String> {
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "placed document fragment test",
        }])?;
    for native in [false, true] {
        let mut owners = Vec::new();
        for (name, fragment) in [
            ("placed-reference", "ref target text \"参照\""),
            ("placed-definition", "anchor target text \"定義\""),
        ] {
            let source = format!(
                "article en \"Title\" body cons display Math label x Sentence sentence cons doc {fragment} nil nil"
            );
            with_named_input(
                native,
                &compiled,
                &source,
                name,
                "Article",
                |tree, profile, b, a| {
                    let registry = profile.registry();
                    let input = tree
                        .tree()
                        .bundle
                        .validate_with_sources(registry, b, a)
                        .map_err(err)?;
                    let store = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                    let doc = nepl3_suite::adapters::sentence::literal::lower(
                        &input,
                        &compiled.doc.package.schema,
                        Category::Article,
                        registry,
                        b,
                        &mut codec,
                    )
                    .map_err(err)?;
                    let math_input = doc
                        .value
                        .embeds
                        .get(0)
                        .ok_or("Math embed")?
                        .closure
                        .syntax()
                        .bundle
                        .validate_with_sources(registry, b, codec.source_admission())
                        .map_err(err)?;
                    let math = nepl3_math_core::lower::expression(
                        &math_input,
                        &compiled.others[0].schema,
                        nepl3_math_core::check::Category::Expr,
                        registry,
                        b,
                        codec.source_admission(),
                    )
                    .map_err(err)?;
                    let guest = math.value.embeds.first().ok_or("Sentence embed")?;
                    let sentence_input = guest
                        .view()
                        .validate_with_bundle(registry, b, codec.source_admission())
                        .map_err(err)?;
                    let sentence = nepl3_sentence_core::lower::presentation::sentence_with_foreign(
                        &sentence_input,
                        &compiled.others[3].schema,
                        &[nepl3_sentence_core::lower::ForeignInlineForm {
                            kind: "Form:DocumentInline",
                            guest_schema: &compiled.doc.package.schema,
                            guest_category: "Inline",
                        }],
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    owners.push(sentence);
                    Ok(())
                },
            )?;
        }
        with_input_route(
            native,
            &compiled,
            "text \"context\"",
            "Inline",
            |_, profile, b, _| {
                let registry = profile.registry();
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                for duplicate in [false, true] {
                    let mut placements = vec![
                        Placement {
                            owner: OwnerId(0),
                            depth: 0,
                        },
                        Placement {
                            owner: OwnerId(1),
                            depth: 1,
                        },
                    ];
                    if duplicate {
                        placements.push(Placement {
                            owner: OwnerId(1),
                            depth: 2,
                        });
                    }
                    let selected = guests::collect_placed(
                        &owners,
                        &placements,
                        &compiled.doc.package.schema,
                        registry,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    assert_eq!(selected.selections().len(), 2);
                    let mut members = Vec::new();
                    for occurrence in selected.occurrences() {
                        members.push(
                            namespace::inspect(
                                selected.document(occurrence.document).ok_or("document")?,
                                registry,
                                b,
                                codec.source_admission(),
                            )
                            .map_err(err)?,
                        );
                    }
                    let refs: Vec<_> = members.iter().collect();
                    let result = namespace::resolve(&refs, b);
                    if duplicate {
                        assert!(
                            matches!(result, Err(namespace::Error::Duplicate { definition, previous })
                        if definition.member == MemberId(2) && previous.member == MemberId(1))
                        );
                    } else {
                        let checked = result.map_err(err)?;
                        assert_eq!(checked.references().len(), 1);
                        let reference = &checked.references()[0];
                        let target = &checked.definitions()[reference.target.0 as usize];
                        assert_eq!(reference.reference.member, MemberId(0));
                        assert_eq!(target.member, MemberId(1));
                        assert_ne!(
                            reference
                                .reference
                                .site
                                .selection
                                .ok_or("reference source")?
                                .snapshot_ref(),
                            target
                                .site
                                .selection
                                .ok_or("definition source")?
                                .snapshot_ref()
                        );
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}
