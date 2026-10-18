use super::*;
use crate::doc::annotations::{self, ForeignRecord, SentenceAnnotationRenderer, paragraph};
use nepl3_markup::html::{HtmlNode, HtmlTag};
use nepl3_suite::adapters::sentence::document_guests::{OwnerId, Placement};

#[test]
fn independent_sentence_paragraph_renders_forward_refs_and_retains_origins() -> Result<(), String> {
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "paragraph document fragment test",
        }])?;
    for native in [false, true] {
        let inputs = placed::sentence_owners(
            &compiled,
            native,
            &[
                ("paragraph-reference", "ref target text \"参照\""),
                ("paragraph-definition", "anchor target text \"定義\""),
            ],
        )?;
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
                let mut host = SentenceAnnotationRenderer {
                    registry,
                    surface: &compiled.others[3].schema,
                    math_surface: Some(&compiled.others[0].schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                let placements = [
                    Placement {
                        owner: OwnerId(0),
                        depth: 0,
                    },
                    Placement {
                        owner: OwnerId(1),
                        depth: 0,
                    },
                ];
                let result = paragraph::render(&inputs, &placements, &mut host, b).map_err(err)?;
                assert_eq!(result.paragraph.placements().len(), 2);
                assert_eq!(result.foreign.len(), 2);
                let markup = result.paragraph.markup();
                assert!(matches!(
                    markup.fragment.nodes[markup.fragment.root as usize],
                    HtmlNode::Element {
                        tag: HtmlTag::P,
                        ..
                    }
                ));
                let checked =
                    nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, b)
                        .map_err(err)?;
                let text = nepl3_markup::html::serialize_xhtml(&checked, b).map_err(err)?;
                assert!(text.contains("参照") && text.contains("定義"));
                assert!(
                    text.contains("href=\"#n-746172676574\"")
                        && text.contains("id=\"n-746172676574\"")
                );
                for (index, (foreign, placement)) in result
                    .foreign
                    .iter()
                    .zip(result.paragraph.placements())
                    .enumerate()
                {
                    assert!(core::ptr::eq(placement.input(), &inputs[index]));
                    let [ForeignRecord::Document(record)] = foreign.as_slice() else {
                        return Err("Doc record".into());
                    };
                    assert!(!record.origins.is_empty());
                    let guest = inputs[index].value.embeds.entry(0).ok_or("source guest")?;
                    assert!(!record.document.sources.is_empty());
                    for source in &record.document.sources {
                        assert!(
                            guest
                                .syntax()
                                .bundle
                                .sources
                                .iter()
                                .any(|original| original.identity() == source.identity())
                        );
                    }

                    for origin in &record.origins {
                        assert!(origin.node < record.document.value.nodes.len() as u64);
                        assert!(origin.element >= placement.first_element());
                        assert!(origin.element < placement.first_element() + placement.elements());
                    }
                }
                let duplicates = [placements[0], placements[1], placements[1]];
                assert!(
                    matches!(paragraph::render(&inputs, &duplicates, &mut host, b),
                Err(annotations::Error::Document(error)) if matches!(*error, annotations::document::Error::NamespaceDuplicate { .. }))
                );
                let repeated = [placements[0], placements[0], placements[1]];
                let repeated_result =
                    paragraph::render(&inputs, &repeated, &mut host, b).map_err(err)?;
                let [ForeignRecord::Document(first)] = repeated_result.foreign[0].as_slice() else {
                    return Err("first".into());
                };
                let [ForeignRecord::Document(second)] = repeated_result.foreign[1].as_slice()
                else {
                    return Err("second".into());
                };
                assert!(std::sync::Arc::ptr_eq(&first.document, &second.document));
                assert_ne!(first.origins[0].element, second.origins[0].element);
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn independent_sentence_paragraph_keeps_math_positions_and_exact_stops() -> Result<(), String> {
    use nepl3_core::budget::StopReason;
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "paragraph Math fragment test",
        }])?;
    let inputs = placed::sentence_owners(
        &compiled,
        true,
        &[
            ("paragraph-math", "ref target math Math 7"),
            ("paragraph-anchor", "anchor target text \"定義\""),
        ],
    )?;
    with_input_route(
        true,
        &compiled,
        "text \"context\"",
        "Inline",
        |_, profile, _, _| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let placements = [
                Placement {
                    owner: OwnerId(0),
                    depth: 0,
                },
                Placement {
                    owner: OwnerId(0),
                    depth: 2,
                },
                Placement {
                    owner: OwnerId(1),
                    depth: 1,
                },
            ];
            let run = |limit: &mut Budget| {
                let mut admission = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
                let mut host = SentenceAnnotationRenderer {
                    registry,
                    surface: &compiled.others[3].schema,
                    math_surface: Some(&compiled.others[0].schema),
                    doc_surface: Some(&compiled.doc.package.schema),
                    codec: &mut codec,
                };
                Ok::<_, String>(limit.with_depth_at_least(3, |b| {
                    paragraph::render(&inputs, &placements, &mut host, b)
                }))
            };
            let mut measured = budget();
            let result = run(&mut measured)?.map_err(err)?;
            for (index, foreign) in result.foreign.iter().enumerate().take(2) {
                let [ForeignRecord::Document(record)] = foreign.as_slice() else {
                    return Err("document record".into());
                };
                let [math] = record.foreign.as_slice() else {
                    return Err("Math record".into());
                };
                assert!(!math.output.node_roots.is_empty());
                let placement = &result.paragraph.placements()[index];
                for node in &math.output.node_roots {
                    assert!(
                        *node >= placement.first_element()
                            && *node < placement.first_element() + placement.elements()
                    );
                    assert!(matches!(
                        result.paragraph.markup().fragment.nodes[*node as usize],
                        HtmlNode::MathElement { .. }
                    ));
                }
            }
            let used = measured.usage();
            for (amount, reason) in [
                (used.work, StopReason::WorkLimit),
                (used.allocation_units, StopReason::AllocationLimit),
                (used.nodes, StopReason::NodeLimit),
                (used.source_bytes, StopReason::SourceLimit),
                (used.depth, StopReason::DepthLimit),
            ] {
                assert!(amount > 0);
                for exact in [false, true] {
                    let mut limits = budget().limits();
                    let limit = amount - u64::from(!exact);
                    match reason {
                        StopReason::WorkLimit => limits.work = limit,
                        StopReason::AllocationLimit => limits.allocation_units = limit,
                        StopReason::NodeLimit => limits.nodes = limit,
                        StopReason::SourceLimit => limits.source_bytes = limit,
                        StopReason::DepthLimit => limits.depth = limit,
                        _ => unreachable!(),
                    }
                    let mut limited = Budget::new(limits);
                    let actual = run(&mut limited)?;
                    if exact {
                        actual.map_err(err)?;
                    } else {
                        assert!(
                            matches!(actual, Err(annotations::Error::Stopped(actual)) if actual == reason)
                        );
                        assert_eq!(limited.poll(), Err(reason));
                    }
                }
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                run(&mut cancelled)?,
                Err(annotations::Error::Stopped(StopReason::Cancelled))
            ));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn independent_sentence_paragraph_keeps_math_annotation_namespace_local() -> Result<(), String> {
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "paragraph namespace isolation test",
        }])?;
    let inputs = placed::sentence_owners(
        &compiled,
        true,
        &[
            ("outer-reference", "ref secret text \"outer\""),
            (
                "inner-math-definition",
                "math Math label 7 Sentence sentence cons doc anchor secret text \"inner\" nil",
            ),
        ],
    )?;
    with_input_route(
        true,
        &compiled,
        "text \"context\"",
        "Inline",
        |_, profile, b, _| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry,
                surface: &compiled.others[3].schema,
                math_surface: Some(&compiled.others[0].schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            let placements = [
                Placement {
                    owner: OwnerId(0),
                    depth: 0,
                },
                Placement {
                    owner: OwnerId(1),
                    depth: 0,
                },
            ];
            assert!(
                matches!(paragraph::render(&inputs, &placements, &mut host, b),
            Err(annotations::Error::Document(error)) if matches!(*error, annotations::document::Error::Label(_)))
            );
            // The nested annotation itself is valid; its definition is only local.
            let local = [Placement {
                owner: OwnerId(0),
                depth: 0,
            }];
            let result = paragraph::render(&inputs[1..], &local, &mut host, b).map_err(err)?;
            assert_eq!(result.foreign.len(), 1);
            let [ForeignRecord::Document(doc)] = result.foreign[0].as_slice() else {
                return Err("doc".into());
            };
            assert_eq!(doc.foreign.len(), 1);
            assert!(!doc.foreign[0].output.annotations.is_empty());
            let markup = result.paragraph.markup();
            let checked =
                nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, b)
                    .map_err(err)?;
            nepl3_markup::html::serialize_xhtml(&checked, b).map_err(err)?;
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn independent_sentence_paragraph_rejects_empty_foreign_ruby_base() -> Result<(), String> {
    use nepl3_core::origin::{Origin, OriginId};
    use nepl3_sentence_core::{
        model::{InlineRef, Kind, Root},
        syntax::NodeLocation,
    };
    let compiled =
        compiled_with_sentence_forms(&[nepl3_grammar_core::compile::package::ForeignForm {
            kind: "DocumentInline",
            category: "Inline",
            spelling: "doc",
            field: "syntax",
            alias: "Doc",
            guest_category: "Inline",
            origin_reason: "paragraph empty content test",
        }])?;
    let mut inputs = placed::sentence_owners(
        &compiled,
        true,
        &[("empty-base", "anchor target text \"\"")],
    )?;
    let input = &mut inputs[0];
    let Root::Sentence(root) = input.value.root else {
        return Err("root".into());
    };
    let Kind::Sentence { inlines } = &input.value.nodes[root.0 as usize] else {
        return Err("sentence".into());
    };
    let base = inlines[0];
    let reading = InlineRef(input.value.nodes.len() as u64);
    let ruby = InlineRef(reading.0 + 1);
    let origin = OriginId(input.origins.len() as u64);
    input.origins.push(Origin::Synthetic {
        reason: "generated ruby".into(),
        anchor: None,
    });
    input.value.nodes.push(Kind::Text {
        text: "reading".into(),
    });
    input.value.nodes.push(Kind::Ruby { base, reading });
    input.locations.extend(vec![
        NodeLocation {
            origin,
            head: None,
            cover: None,
        };
        2
    ]);
    input.value.nodes[root.0 as usize] = Kind::Sentence {
        inlines: vec![ruby],
    };
    with_input_route(
        true,
        &compiled,
        "text \"context\"",
        "Inline",
        |_, profile, b, _| {
            let registry = profile.registry();
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
            let mut host = SentenceAnnotationRenderer {
                registry,
                surface: &compiled.others[3].schema,
                math_surface: Some(&compiled.others[0].schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            };
            assert!(
                matches!(paragraph::render(&inputs, &[Placement { owner: OwnerId(0), depth: 0 }], &mut host, b),
            Err(annotations::Error::Document(error)) if matches!(*error, annotations::document::Error::Content(_)))
            );
            Ok(())
        },
    )?;
    Ok(())
}
