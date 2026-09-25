use super::*;
use crate::doc::source::{budget, compiled, with_input_route};
use nepl3_doc_core::{
    lower,
    model::DocKind,
    pages::{PageDocument, PageRegistration, PageSet},
};
use nepl3_doc_html::{ParallelMode, RenderOptions};

fn request(compiled: &Compiled, source: &str, shared: bool) -> Result<PagesHtmlRequest, String> {
    with_input_route(true, compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let mut document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            nepl3_doc_core::check::Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        if shared {
            let blocks = document
                .value
                .nodes
                .iter_mut()
                .find_map(|node| match &mut node.kind {
                    DocKind::Body { blocks } => Some(blocks),
                    _ => None,
                })
                .ok_or("body")?;
            blocks.push(*blocks.first().ok_or("paragraph")?);
        }
        Ok(PagesHtmlRequest {
            set: PageSet {
                pages: vec![PageDocument {
                    registration: PageRegistration {
                        id: "page".into(),
                        source: "page.nepld".into(),
                        route: "page.html".into(),
                    },
                    document,
                }],
                files: vec![],
            },
            options: RenderOptions {
                parallel: ParallelMode::Single {
                    language: "en".into(),
                    fallbacks: vec![],
                },
            },
        })
    })
}

#[test]
fn complete_export_rejects_shared_ids_unresolved_links_and_hidden_unsafe_slots()
-> Result<(), String> {
    let compiled = compiled()?;
    for (source, shared, reason, expected) in [
        (
            r#"article en sentence "Title" body cons paragraph cons sentence sentence cons doc anchor target text "Target" nil nil nil"#,
            true,
            "Duplicate",
            vec![],
        ),
        (
            r#"article en sentence "Title" body cons paragraph cons sentence sentence cons doc ref missing text "Label" nil nil nil"#,
            false,
            "Unresolved",
            vec![],
        ),
        (
            r#"article en sentence "Title" body cons paragraph cons parallel cons variant en sentence "Visible" cons variant ja sentence sentence cons link "javascript:alert(1)" text "Hidden" nil nil nil nil"#,
            false,
            "Attribute",
            vec![Stage::Prepare],
        ),
    ] {
        let request = request(&compiled, source, shared)?;
        let mut stages = Vec::new();
        let result = render_observed(&request, &compiled, &mut budget(), &mut |m| {
            stages.push(m.stage)
        });
        let error = result.err().ok_or("unexpected exported document")?;
        assert!(error.contains(reason), "{error}");
        assert_eq!(stages, expected);
    }
    // Repeated references may share one owner; their target occurs once.
    let request = request(
        &compiled,
        r#"article en sentence sentence cons doc anchor target text "Target" nil body cons paragraph cons sentence sentence cons doc ref target text "Label" nil nil nil"#,
        true,
    )?;
    let actual = render(&request, &compiled, &mut budget())?;
    assert_eq!(actual.pages.len(), 1);
    // The serializer uses n- followed by the UTF-8 bytes of the authored ID.
    // Occurrence ownership is checked by the namespace and composition tests.
    assert!(actual.pages[0].contains("href=\"#n-746172676574\""));
    Ok(())
}

#[test]
fn complete_export_keeps_exact_limits_and_stopped_publication() -> Result<(), String> {
    let compiled = compiled()?;
    let request = request(
        &compiled,
        r#"article en sentence "Title" body cons paragraph cons sentence "Body" nil nil"#,
        false,
    )?;
    let mut measured = budget();
    let expected = render(&request, &compiled, &mut measured)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::OutputLimit,
        StopReason::SourceLimit,
    ] {
        for below in [false, true] {
            let mut limits = budget().limits();
            let used = measured.usage();
            let (limit, used) = match reason {
                StopReason::WorkLimit => (&mut limits.work, used.work),
                StopReason::AllocationLimit => {
                    (&mut limits.allocation_units, used.allocation_units)
                }
                StopReason::NodeLimit => (&mut limits.nodes, used.nodes),
                StopReason::DepthLimit => (&mut limits.depth, used.depth),
                StopReason::OutputLimit => (&mut limits.output_bytes, used.output_bytes),
                StopReason::SourceLimit => (&mut limits.source_bytes, used.source_bytes),
                _ => unreachable!(),
            };
            *limit = used
                .checked_sub(u64::from(below))
                .ok_or("nonzero resource")?;
            let mut receiver = Budget::new(limits);
            let mut stages = Vec::new();
            let result = render_observed(&request, &compiled, &mut receiver, &mut |m| {
                stages.push(m.stage)
            });
            if below {
                assert!(result.is_err());
                assert_eq!(receiver.poll(), Err(reason));
                assert!(!stages.contains(&Stage::RenderAndSerialize));
            } else {
                let result = result?;
                assert_eq!(result.pages, expected.pages);
                assert_eq!(result.identity, expected.identity);
            }
        }
    }
    Ok(())
}

#[test]
fn retained_export_matches_public_raw_pipeline_with_less_preparation_work() -> Result<(), String> {
    use nepl3_doc_core::labels::namespace as labels;
    let compiled = compiled()?;
    let request = request(
        &compiled,
        r#"article en sentence sentence cons doc anchor target text "Target" nil body cons paragraph cons sentence sentence cons doc ref target text "Label" nil nil nil"#,
        true,
    )?;
    let registry = &compiled.doc.registry;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut raw = budget();
    let found = discovery::collect(
        &request.set.pages[0].document,
        registry
            .selected("nepl3.syntax.sentence", 1)
            .ok_or("Sentence")?,
        &compiled.doc.package.schema,
        &[ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: &compiled.doc.package.schema,
            guest_category: "Inline",
        }],
        registry,
        &mut codec,
        &mut raw,
    )
    .map_err(err)?;
    let mut render_admission = SourceAdmission::default();
    let plan = discovery::namespace::inspect(&found, registry, &mut raw, &mut render_admission)
        .map_err(err)?;
    let members = plan.member_refs(&mut raw).map_err(err)?;
    let namespace = labels::resolve(&members, &mut raw).map_err(err)?;
    let namespaces = [&namespace];
    let checked =
        scopes::resolve(&request.set, &namespaces, registry, &mut codec, &mut raw).map_err(err)?;
    let prepared = html::prepare(&checked, &request.options, &mut raw).map_err(err)?;
    let baseline = raw.usage().work;
    let output = composition::render(
        plan.selection(),
        &prepared,
        0,
        registry,
        &mut |document, slot, _, embed, _| {
            Err(AdapterError::Sentence {
                document,
                slot,
                embed,
            })
        },
        &mut |document, _, embed, _| Err(AdapterError::Document { document, embed }),
        &mut raw,
        &mut render_admission,
    )
    .map_err(err)?;
    let markup = &output
        .members()
        .first()
        .ok_or("root")?
        .document()
        .output()
        .fragment
        .markup;
    let outputs = [markup];
    let final_output = html::output::check(&prepared, &outputs, &mut raw).map_err(err)?;
    let expected = final_output
        .pages()
        .iter()
        .map(|page| checked_shell(page, &mut raw))
        .collect::<Result<Vec<_>, _>>()?;
    let mut work = None;
    let actual = render_observed(&request, &compiled, &mut budget(), &mut |measurement| {
        if measurement.stage == Stage::Prepare {
            work = Some(measurement.usage.work);
        }
    })?;
    assert_eq!(actual.pages, expected);
    assert_eq!(actual.identity, final_output.namespace_identity());
    assert!(work.ok_or("Prepare observation")? < baseline);
    Ok(())
}
