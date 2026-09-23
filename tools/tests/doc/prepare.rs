use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{check::Category, lower, model::*, portable, prepare::*};

#[test]
fn sentence_preparation_preserves_requirements_and_local_label_failures() -> Result<(), String> {
    use nepl3_doc_core::labels::{self, LabelError};
    let compiled = compiled()?;
    for (source, valid) in [
        (
            r#"sentence cons ref later text "reference" cons anchor later text "definition" cons link page "guide" none text "page" cons math Math frac 1 0 nil"#,
            true,
        ),
        (r#"sentence cons ref outside text "missing" nil"#, false),
        (
            r#"sentence cons anchor x text "first" cons anchor x text "second" nil"#,
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
            let result = inspect_sentence(&doc, profile.registry(), &mut c, &mut budget());
            if valid {
                let plan = result.map_err(err)?;
                let plan_value = portable::prepare::plan_to_value(
                    &plan,
                    &doc,
                    profile.registry(),
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                let plan_bytes = nepl3_wire::encode(&plan_value, &mut budget()).map_err(err)?;
                assert_eq!(plan.requirements.len(), 2);
                assert!(
                    matches!(&plan.requirements[0], DocRequirement::Link { target: LinkTarget::Page { page, fragment: None }, .. } if page == "guide")
                );
                assert!(matches!(
                    &plan.requirements[1],
                    DocRequirement::Foreign {
                        kind: EmbedKind::InlineMath,
                        ..
                    }
                ));
                let labels = labels::check_sentence(
                    &doc,
                    profile.registry(),
                    &mut budget(),
                    c.source_admission(),
                )
                .map_err(err)?;
                assert_eq!(labels.definitions()[0].name, "later");
                assert_eq!(labels.references()[0].target, labels::DocLabelId(0));
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
                let mut shared = doc.clone();
                let anchor = labels.definitions()[0].node;
                let DocRoot::Sentence(root) = shared.value.root else {
                    return Err("Sentence root".into());
                };
                let DocKind::Sentence { inlines: items } =
                    &mut shared.value.nodes[root.0 as usize].kind
                else {
                    return Err("Sentence node".into());
                };
                items.push(InlineRef(anchor));
                assert!(matches!(
                    inspect_sentence(&shared, profile.registry(), &mut c, &mut budget()),
                    Err(PreparationError::Label(
                        LabelError::DuplicateOccurrence { .. }
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(PreparationError::Label(
                        LabelError::Unresolved { .. } | LabelError::Duplicate { .. }
                    ))
                ));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                inspect_sentence(&doc, profile.registry(), &mut c, &mut cancelled),
                Err(PreparationError::Stopped(_))
            ));
            for resource in 0..4 {
                let mut limits = budget().limits();
                match resource {
                    0 => limits.work = 0,
                    1 => limits.allocation_units = 0,
                    2 => limits.nodes = 0,
                    _ => limits.depth = 0,
                }
                assert!(matches!(
                    inspect_sentence(&doc, profile.registry(), &mut c, &mut Budget::new(limits)),
                    Err(PreparationError::Stopped(_))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}
