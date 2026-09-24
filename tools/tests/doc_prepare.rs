use nepl3_core::source::{Digest, SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower, model::*, portable, prepare::*};
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;
#[path = "doc/article.rs"]
mod article;
#[path = "doc/retention.rs"]
mod retention;

// Both Math guests contain division by zero. Discovery retains syntax and
// identifies dependencies without evaluating guests or fetching resources.
const INPUT: &str = r#"article ja sentence "準備"
body
 cons paragraph
   cons sentence sentence
     cons doc link page "guide" some "intro" text "参照"
     cons doc image asset "logo" none sentence "代替"
     cons doc math Math frac 1 0
     nil
   nil
 cons code Math frac 1 0
 nil"#;

#[test]
fn source_preparation_retains_code_and_identifies_each_foreign_closure() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(&compiled, INPUT, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut c =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut c,
        )
        .map_err(err)?;
        let original = doc.clone();
        assert!(matches!(
            inspect_sentence(&doc, profile.registry(), &mut c, &mut budget()),
            Err(PreparationError::Label(
                nepl3_doc_core::labels::LabelError::ExpectedSentence
            ))
        ));
        let plan = inspect(&doc, profile.registry(), &mut c, &mut budget()).map_err(err)?;
        assert_eq!(plan.requirements.len(), 3);
        assert_eq!(
            doc.value
                .embeds
                .iter()
                .map(|embed| embed.kind)
                .collect::<Vec<_>>(),
            [EmbedKind::Sentence, EmbedKind::Sentence, EmbedKind::Code]
        );
        for (index, embed) in doc.value.embeds.iter().enumerate() {
            // The domain and CBOR digest are calculated independently of inspect.
            let value = portable::embed_value(embed, profile.registry(), &mut c, &mut budget())
                .map_err(err)?;
            let mut bytes = b"NEPL3.Doc.Prepare.Guest.v1\0".to_vec();
            bytes.extend(nepl3_wire::encode(&value, &mut budget()).map_err(err)?);
            assert_eq!(
                plan.requirements[index],
                DocRequirement::Foreign {
                    embed: EmbedRef(index as u64),
                    kind: embed.kind,
                    guest_digest: Digest::of(&bytes),
                }
            );
        }
        let doc_bytes = nepl3_wire::encode(
            &portable::to_value(&doc, profile.registry(), &mut c, &mut budget()).map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let plan_bytes = nepl3_wire::encode(
            &portable::prepare::plan_to_value(
                &plan,
                &doc,
                profile.registry(),
                &mut c,
                &mut budget(),
            )
            .map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        let mut fresh_admission = SourceAdmission::default();
        let mut fresh =
            FoundationCodec::new(profile.registry(), &empty, &mut fresh_admission).map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&doc_bytes, &mut budget()).map_err(err)?,
            profile.registry(),
            &mut fresh,
            &mut budget(),
        )
        .map_err(err)?;
        let actual = portable::prepare::plan_from_value(
            &nepl3_wire::decode(&plan_bytes, &mut budget()).map_err(err)?,
            &received,
            profile.registry(),
            &mut fresh,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(actual, plan);
        assert_eq!(doc, original);
        retention::assert_doc_retention(&doc, &received)?;
        for (document, codec) in [(&doc, &mut c), (&received, &mut fresh)] {
            let mut guests = Vec::new();
            for embed in &document.value.embeds {
                if embed.kind != EmbedKind::Sentence {
                    continue;
                }
                let sentence = nepl3_suite::adapters::document::sentence::lower(
                    embed,
                    embed.schema(),
                    &[nepl3_sentence_core::lower::ForeignInlineForm {
                        kind: "Form:DocumentInline",
                        guest_schema: &compiled.doc.package.schema,
                        guest_category: "Inline",
                    }],
                    profile.registry(),
                    codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let selected = nepl3_suite::adapters::sentence::document_guests::collect(
                    &sentence,
                    &compiled.doc.package.schema,
                    profile.registry(),
                    codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let (documents, occurrences) = selected.into_parts();
                assert_eq!(documents.len(), occurrences.len());
                guests.extend(documents);
            }
            assert_eq!(guests.len(), 3);
            let plans = guests
                .iter()
                .map(|guest| {
                    inspect_inline(guest, profile.registry(), codec, &mut budget()).map_err(err)
                })
                .collect::<Result<Vec<_>, _>>()?;
            for (guest, plan) in guests.iter().zip(&plans) {
                let foreign: Vec<_> = plan
                    .requirements
                    .iter()
                    .filter(|requirement| matches!(requirement, DocRequirement::Foreign { .. }))
                    .collect();
                assert_eq!(foreign.len(), guest.value.embeds.len());
                for (index, embed) in guest.value.embeds.iter().enumerate() {
                    let value =
                        portable::embed_value(embed, profile.registry(), codec, &mut budget())
                            .map_err(err)?;
                    let mut bytes = b"NEPL3.Doc.Prepare.Guest.v1\0".to_vec();
                    bytes.extend(nepl3_wire::encode(&value, &mut budget()).map_err(err)?);
                    assert_eq!(
                        *foreign[index],
                        DocRequirement::Foreign {
                            embed: EmbedRef(index as u64),
                            kind: embed.kind,
                            guest_digest: Digest::of(&bytes),
                        }
                    );
                }
            }
            assert_eq!(
                plans
                    .iter()
                    .map(|plan| plan.requirements.len())
                    .collect::<Vec<_>>(),
                [2, 2, 1]
            );
            assert!(matches!(&plans[0].requirements[0], DocRequirement::Link {
                target: LinkTarget::Page { page, fragment }, ..
            } if page == "guide" && fragment.as_deref() == Some("intro")));
            assert!(matches!(&plans[1].requirements[0], DocRequirement::Asset {
                asset: AssetRef { id, digest: None }, ..
            } if id == "logo"));
            // Labels/alternative text keep their independent Sentence ownership.
            assert!(matches!(
                &plans[0].requirements[1],
                DocRequirement::Foreign {
                    kind: EmbedKind::SentenceInline,
                    ..
                }
            ));
            assert!(matches!(
                &plans[1].requirements[1],
                DocRequirement::Foreign {
                    kind: EmbedKind::Sentence,
                    ..
                }
            ));
            assert!(matches!(
                &plans[2].requirements[0],
                DocRequirement::Foreign {
                    kind: EmbedKind::InlineMath,
                    ..
                }
            ));
        }
        // Source tables and guest NodeRef coordinates canonicalize on wire.
        // The full canonical bytes must still preserve every field and source.
        let reencoded = nepl3_wire::encode(
            &portable::to_value(&received, profile.registry(), &mut fresh, &mut budget())
                .map_err(err)?,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(reencoded, doc_bytes);
        Ok(())
    })
}

#[test]
fn sentence_preparation_preserves_requirements_and_local_label_failures() -> Result<(), String> {
    use nepl3_core::{
        budget::{Budget, StopReason},
        value_codec::FoundationValueCodec,
    };
    use nepl3_doc_core::labels::{self, LabelError, namespace};
    let compiled = compiled()?;
    for (source, valid) in [
        (
            r#"sentence sentence cons doc ref later text "reference" cons doc anchor later text "definition" cons doc link page "guide" none text "page" cons doc math Math frac 1 0 nil"#,
            true,
        ),
        (
            r#"sentence sentence cons doc ref outside text "missing" nil"#,
            false,
        ),
        (
            r#"sentence sentence cons doc anchor x text "first" cons doc anchor x text "second" nil"#,
            false,
        ),
    ] {
        with_input(&compiled, source, "Sentence", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = lower::document(
                &syntax,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                &mut budget(),
                &mut c,
            )
            .map_err(err)?;
            assert!(matches!(
                inspect(&doc, profile.registry(), &mut c, &mut budget()),
                Err(PreparationError::Label(LabelError::ExpectedArticle))
            ));
            let plan =
                inspect_sentence(&doc, profile.registry(), &mut c, &mut budget()).map_err(err)?;
            assert_eq!(plan.requirements.len(), 1);
            assert!(matches!(
                plan.requirements[0],
                DocRequirement::Foreign {
                    kind: EmbedKind::Sentence,
                    ..
                }
            ));
            let select = |document: &DocumentSyntax, codec: &mut FoundationCodec<'_>| {
                let sentence = nepl3_suite::adapters::document::sentence::lower(
                    &document.value.embeds[0],
                    document.value.embeds[0].schema(),
                    &[nepl3_sentence_core::lower::ForeignInlineForm {
                        kind: "Form:DocumentInline",
                        guest_schema: &compiled.doc.package.schema,
                        guest_category: "Inline",
                    }],
                    profile.registry(),
                    codec,
                    &mut budget(),
                )
                .map_err(err)?;
                nepl3_suite::adapters::sentence::document_guests::collect(
                    &sentence,
                    &compiled.doc.package.schema,
                    profile.registry(),
                    codec,
                    &mut budget(),
                )
                .map_err(err)
            };
            let selected = select(&doc, &mut c)?;
            let (guests, occurrences) = selected.into_parts();
            let members = guests
                .iter()
                .map(|guest| {
                    namespace::inspect(
                        guest,
                        profile.registry(),
                        &mut budget(),
                        c.source_admission(),
                    )
                    .map_err(err)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let ordered = occurrences
                .iter()
                .map(|occurrence| &members[occurrence.document.index()])
                .collect::<Vec<_>>();
            let result = namespace::resolve(&ordered, &mut budget());
            if valid {
                let labels = result.map_err(err)?;
                let plan_value = portable::prepare::plan_to_value(
                    &plan,
                    &doc,
                    profile.registry(),
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                let plan_bytes = nepl3_wire::encode(&plan_value, &mut budget()).map_err(err)?;
                assert_eq!(labels.definitions().len(), 1);
                assert_eq!(labels.references().len(), 1);
                assert_eq!(labels.definitions()[0].site.name, "later");
                assert_eq!(labels.references()[0].target, labels::DocLabelId(0));
                let selected_plans =
                    inspect_namespace(&labels, profile.registry(), &mut c, &mut budget())
                        .map_err(err)?;
                assert_eq!(selected_plans.len(), 4);
                assert!(
                    matches!(&selected_plans[2].requirements[0], DocRequirement::Link {
                    target: LinkTarget::Page { page, fragment: None }, ..
                } if page == "guide")
                );
                assert!(matches!(
                    selected_plans[3].requirements[0],
                    DocRequirement::Foreign {
                        kind: EmbedKind::InlineMath,
                        ..
                    }
                ));
                let value = portable::to_value(&doc, profile.registry(), &mut c, &mut budget())
                    .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
                let mut fresh_admission = SourceAdmission::default();
                let mut fresh =
                    FoundationCodec::new(profile.registry(), &empty, &mut fresh_admission)
                        .map_err(err)?;
                let decoded = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                let received =
                    portable::from_value(&decoded, profile.registry(), &mut fresh, &mut budget())
                        .map_err(err)?;
                assert_eq!(
                    inspect_sentence(&received, profile.registry(), &mut fresh, &mut budget())
                        .map_err(err)?,
                    plan
                );
                let (received_guests, received_occurrences) =
                    select(&received, &mut fresh)?.into_parts();
                assert_eq!(received_occurrences, occurrences);
                let received_members = received_guests
                    .iter()
                    .map(|guest| {
                        namespace::inspect(
                            guest,
                            profile.registry(),
                            &mut budget(),
                            fresh.source_admission(),
                        )
                        .map_err(err)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let received_ordered = received_occurrences
                    .iter()
                    .map(|occurrence| &received_members[occurrence.document.index()])
                    .collect::<Vec<_>>();
                let received_labels =
                    namespace::resolve(&received_ordered, &mut budget()).map_err(err)?;
                assert_eq!(received_labels.definitions(), labels.definitions());
                assert_eq!(received_labels.references(), labels.references());
                assert_eq!(
                    inspect_namespace(
                        &received_labels,
                        profile.registry(),
                        &mut fresh,
                        &mut budget()
                    )
                    .map_err(err)?,
                    selected_plans
                );
                let plan_value = nepl3_wire::decode(&plan_bytes, &mut budget()).map_err(err)?;
                assert_eq!(
                    portable::prepare::plan_from_value(
                        &plan_value,
                        &received,
                        profile.registry(),
                        &mut fresh,
                        &mut budget()
                    )
                    .map_err(err)?,
                    plan
                );
                let mut missing = plan.clone();
                let options = nepl3_doc_html::RenderOptions {
                    parallel: nepl3_doc_html::ParallelMode::Rows,
                };
                assert!(
                    matches!(nepl3_doc_html::prepare_local_sentence(&doc, &options, profile.registry(), &mut c, &mut budget()), Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(ref requirements)) if requirements == &plan)
                );
                let mut tampered = plan_value.clone();
                let nepl3_core::value::NdfValue::Record(ref mut record) = tampered else {
                    return Err("plan record".into());
                };
                let nepl3_core::value::NdfValue::List(ref mut requirements) = record.fields[1]
                else {
                    return Err("plan requirements".into());
                };
                requirements.clear();
                assert!(
                    portable::prepare::plan_from_value(
                        &tampered,
                        &received,
                        profile.registry(),
                        &mut fresh,
                        &mut budget()
                    )
                    .is_err()
                );
                missing.requirements.clear();
                assert!(
                    portable::prepare::plan_to_value(
                        &missing,
                        &doc,
                        profile.registry(),
                        &mut c,
                        &mut budget()
                    )
                    .is_err()
                );
                // A repeated declaration occurrence is rejected even when the
                // underlying immutable Doc fragment is shared.
                let mut shared = ordered.clone();
                shared.push(ordered[1]);
                assert!(matches!(
                    namespace::resolve(&shared, &mut budget()),
                    Err(namespace::Error::Duplicate { .. })
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(namespace::Error::Unresolved { .. } | namespace::Error::Duplicate { .. })
                ));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                namespace::resolve(&ordered, &mut cancelled),
                Err(namespace::Error::Stopped(StopReason::Cancelled))
            ));
            assert!(matches!(
                inspect_sentence(&doc, profile.registry(), &mut c, &mut cancelled),
                Err(PreparationError::Stopped(_))
            ));
            for (resource, expected) in [
                StopReason::WorkLimit,
                StopReason::AllocationLimit,
                StopReason::NodeLimit,
                StopReason::DepthLimit,
            ]
            .into_iter()
            .enumerate()
            {
                let mut limits = budget().limits();
                match resource {
                    0 => limits.work = 0,
                    1 => limits.allocation_units = 0,
                    2 => limits.nodes = 0,
                    _ => limits.depth = 0,
                }
                let mut bounded = Budget::new(limits);
                assert!(
                    matches!(inspect_sentence(&doc, profile.registry(), &mut c, &mut bounded),
                    Err(PreparationError::Stopped(reason)) if reason == expected)
                );
                assert_eq!(bounded.poll(), Err(expected));
                // Name resolution consumes Work and allocation; structure
                // traversal separately owns the Nodes/Depth checks above.
                if valid && resource < 2 {
                    let mut bounded = Budget::new(limits);
                    assert!(matches!(namespace::resolve(&ordered, &mut bounded),
                        Err(namespace::Error::Stopped(reason)) if reason == expected));
                    assert_eq!(bounded.poll(), Err(expected));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
