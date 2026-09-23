use nepl3_core::source::{Digest, SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower, model::*, portable, prepare::*};
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;
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
