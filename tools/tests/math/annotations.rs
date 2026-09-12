use super::*;
use nepl3_markup::{
    html::HtmlRequest,
    mathml::{Display, serialize, validate},
};
use nepl3_math_core::{check, lower};
use nepl3_tools::doc::annotations::{DocAnnotationRenderer, Error as GuestError};

#[test]
fn real_doc_annotations_survive_mathml_and_reject_unresolved_meanings() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, success) in [
        (
            r#"label frac 1 0 Doc sentence cons anno ruby text "字" text "じ" cons text "character" nil cons break cons code "<&" nil"#,
            true,
        ),
        (
            r#"label x Doc sentence cons ref absent text "missing" nil"#,
            false,
        ),
        (
            r#"label x Doc sentence cons link page "guide" none text "page" nil"#,
            false,
        ),
    ] {
        with_input(&compiled, source, "Expr", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let math = lower::expression(
                &syntax,
                &compiled.others[0].schema,
                check::Category::Expr,
                profile.registry(),
                &mut budget(),
                a,
            )
            .map_err(err)?;
            let checked = check::expression(&math.value, &mut budget()).map_err(err)?;
            let empty = SourceStore::default();
            let mut fresh = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let mut host = DocAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.doc.package.schema,
                options: &options,
                codec: &mut codec,
            };
            let mut records = Vec::new();
            let output = nepl3_math_mathml::render_with_annotations(
                &checked,
                Display::Inline,
                &mut |guest, b| {
                    assert!(core::ptr::eq(guest, &math.value.embeds[0]));
                    let rendered = host.render(guest, b)?;
                    let request = rendered.fragment.markup;
                    records.push((rendered.document, rendered.fragment.origins));
                    Ok::<HtmlRequest, GuestError<_>>(request)
                },
                &mut budget(),
            );
            if success {
                let output = output.map_err(err)?;
                let proof = validate(&output.fragment, &mut budget()).map_err(err)?;
                let xml = serialize(&proof, &mut budget()).map_err(err)?;
                println!(
                    "MATH_ANNOTATION_HTML {}",
                    xml.as_bytes()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                );
                assert_eq!(
                    xml,
                    "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"inline\"><munder><mfrac><mn>1</mn><mn>0</mn></mfrac><mtext><span xmlns=\"http://www.w3.org/1999/xhtml\" class=\"nepl-sentence\"><span><span class=\"nepl-anno\"><span class=\"nepl-base\"><span class=\"nepl-ruby\"><span class=\"nepl-base\">字</span><span class=\"nepl-reading\">じ</span></span></span><span class=\"nepl-notes\"><span class=\"nepl-note\">character</span></span></span><br /><code>&lt;&amp;</code></span></span></mtext></munder></math>"
                );
                assert_eq!(records.len(), 1);
                let raw = nepl3_math_core::portable::to_value(
                    &math,
                    profile.registry(),
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
                let mut received_admission = SourceAdmission::default();
                let mut received_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut received_admission)
                        .map_err(err)?;
                let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                let received = nepl3_math_core::portable::from_value(
                    &raw,
                    profile.registry(),
                    &mut received_codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let received_checked =
                    check::expression(&received.value, &mut budget()).map_err(err)?;
                let mut received_host = DocAnnotationRenderer {
                    registry: profile.registry(),
                    surface: &compiled.doc.package.schema,
                    options: &options,
                    codec: &mut received_codec,
                };
                let received_output = nepl3_math_mathml::render_with_annotations(
                    &received_checked,
                    Display::Inline,
                    &mut |guest, b| received_host.render(guest, b).map(|v| v.fragment.markup),
                    &mut budget(),
                )
                .map_err(err)?;
                assert_eq!(
                    serialize(
                        &validate(&received_output.fragment, &mut budget()).map_err(err)?,
                        &mut budget()
                    )
                    .map_err(err)?,
                    xml
                );
                let (doc, origins) = &records[0];
                assert!(!doc.sources.is_empty());
                assert!(
                    origins
                        .iter()
                        .all(|o| o.node < doc.value.nodes.len() as u64)
                );
                assert_eq!(output.node_roots.len(), math.value.nodes.len());
                let mut wrong = compiled.doc.package.schema.clone();
                wrong.digest = Digest::of(b"wrong schema");
                let mut host = DocAnnotationRenderer {
                    registry: profile.registry(),
                    surface: &wrong,
                    options: &options,
                    codec: &mut codec,
                };
                assert!(matches!(
                    host.render(&math.value.embeds[0], &mut budget()),
                    Err(GuestError::Selection)
                ));
                let mut stopped = budget();
                for resource in 0..4 {
                    let mut limits = budget().limits();
                    match resource {
                        0 => limits.work = 0,
                        1 => limits.nodes = 0,
                        2 => limits.allocation_units = 0,
                        _ => limits.depth = 0,
                    }
                    let mut host = DocAnnotationRenderer {
                        registry: profile.registry(),
                        surface: &compiled.doc.package.schema,
                        options: &options,
                        codec: &mut codec,
                    };
                    let result = nepl3_math_mathml::render_with_annotations(
                        &checked,
                        Display::Inline,
                        &mut |guest, b| host.render(guest, b).map(|v| v.fragment.markup),
                        &mut Budget::new(limits),
                    );
                    assert!(matches!(
                        result,
                        Err(nepl3_math_mathml::AnnotationFailure::Stopped(_))
                    ));
                }
                let result = nepl3_math_mathml::render_with_annotations(
                    &checked,
                    Display::Inline,
                    &mut |_, b: &mut Budget| {
                        b.cancel();
                        Err::<HtmlRequest, _>("host cancelled")
                    },
                    &mut stopped,
                );
                assert!(matches!(
                    result,
                    Err(nepl3_math_mathml::AnnotationFailure::Stopped(
                        StopReason::Cancelled
                    ))
                ));
            } else {
                assert!(matches!(
                    output,
                    Err(nepl3_math_mathml::AnnotationFailure::Guest(
                        GuestError::Label(_) | GuestError::NeedsResolution(_)
                    ))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn annotation_boundaries_preserve_grouping_and_reject_hostile_output() -> Result<(), String> {
    use nepl3_markup::html::{HtmlNode, HtmlSlot, HtmlTag};
    use nepl3_math_mathml::{AnnotationFailure, Error};
    let compiled = compiled()?;
    for source in [
        r#"mul label add x y Doc "sum" z"#,
        r#"add label x Doc sentence cons anchor same text "A" nil label y Doc sentence cons anchor same text "B" nil"#,
        r#"label x Doc sentence cons math Math frac 1 0 nil"#,
    ] {
        with_input(&compiled, source, "Expr", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let math = lower::expression(
                &syntax,
                &compiled.others[0].schema,
                check::Category::Expr,
                profile.registry(),
                &mut budget(),
                a,
            )
            .map_err(err)?;
            let checked = check::expression(&math.value, &mut budget()).map_err(err)?;
            let empty = SourceStore::default();
            let mut fresh = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let mut host = DocAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.doc.package.schema,
                options: &options,
                codec: &mut codec,
            };
            let output = nepl3_math_mathml::render_with_annotations(
                &checked,
                Display::Inline,
                &mut |guest, b| host.render(guest, b).map(|v| v.fragment.markup),
                &mut budget(),
            );
            if source.starts_with("mul") {
                let output = output.map_err(err)?;
                let xml = serialize(
                    &validate(&output.fragment, &mut budget()).map_err(err)?,
                    &mut budget(),
                )
                .map_err(err)?;
                assert!(xml.contains("<munder><mrow><mo>(</mo><mrow><mi>x</mi><mo>+</mo><mi>y</mi></mrow><mo>)</mo></mrow><mtext>"));
                assert!(xml.contains("</munder><mo>·</mo><mi>z</mi>"));
                for mode in 0..3 {
                    let result = nepl3_math_mathml::render_with_annotations(
                        &checked,
                        Display::Inline,
                        &mut |guest, b| {
                            let mut request = host.render(guest, b)?.fragment.markup;
                            if mode == 0 {
                                request.slot = HtmlSlot::Block;
                            } else if mode == 1 {
                                if let HtmlNode::Element { tag, .. } =
                                    &mut request.fragment.nodes[request.fragment.root as usize]
                                {
                                    *tag = HtmlTag::P;
                                }
                            } else {
                                request.policy.classes.clear();
                            }
                            Ok::<_, GuestError<_>>(request)
                        },
                        &mut budget(),
                    );
                    assert!(matches!(
                        result,
                        Err(AnnotationFailure::Render(
                            Error::AnnotationSlot(_) | Error::Markup(_)
                        ))
                    ));
                }
            } else if source.starts_with("add") {
                assert!(matches!(
                    output,
                    Err(AnnotationFailure::Render(Error::Markup(
                        nepl3_markup::mathml::Error::Html(
                            nepl3_markup::html::HtmlError::DuplicateId(_)
                        )
                    )))
                ));
            } else {
                assert!(matches!(
                    output,
                    Err(AnnotationFailure::Guest(GuestError::NeedsResolution(_)))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}
