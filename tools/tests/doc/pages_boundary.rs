use super::*;
use nepl3_doc_core::{
    check::Category,
    labels::namespace as labels,
    pages::{PageDocument, PageRegistration, PageSet},
};
use nepl3_doc_html::{ParallelMode, RenderOptions, namespace::pages as routed};
use nepl3_markup::html::*;
#[test]
fn namespace_pages_check_actual_code_output_and_callback_stops() -> Result<(), String> {
    let compiled = compiled()?;
    with_input_route(
        true,
        &compiled,
        r#"article en "Code" body cons paragraph cons code Math frac 1 2 nil nil"#,
        "Article",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let set = PageSet {
                pages: vec![PageDocument {
                    registration: PageRegistration {
                        id: "code".into(),
                        source: "code.nepld".into(),
                        route: "code.html".into(),
                    },
                    document,
                }],
                files: vec![],
            };
            let member = labels::inspect(
                &set.pages[0].document,
                profile.registry(),
                b,
                codec.source_admission(),
            )
            .map_err(err)?;
            let members = [&member];
            let namespace = labels::resolve(&members, b).map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared = routed::prepare(
                &set,
                &[&namespace],
                &options,
                profile.registry(),
                &mut codec,
                b,
            )
            .map_err(err)?;
            for scenario in 0..7 {
                let mut called = 0;
                let result = routed::render(
                    &prepared,
                    &mut |_, _, view, _, b| {
                        called += 1;
                        assert_eq!(view.kind, nepl3_doc_core::model::EmbedKind::Code);
                        if scenario == 2 {
                            b.cancel();
                            return Err("adapter cancelled".to_string());
                        }
                        let mut nodes = vec![HtmlNode::Text {
                            text: "code".into(),
                        }];
                        let attributes = if scenario == 1 {
                            vec![HtmlAttribute::Id {
                                value: "forged".into(),
                            }]
                        } else if scenario == 4 {
                            vec![HtmlAttribute::Href {
                                value: HtmlHref::Artifact {
                                    path: "unregistered.html".into(),
                                    fragment: None,
                                },
                            }]
                        } else {
                            vec![]
                        };
                        nodes.push(HtmlNode::Element {
                            tag: if scenario == 4 {
                                HtmlTag::A
                            } else {
                                HtmlTag::Span
                            },
                            attributes,
                            children: vec![0],
                        });
                        if scenario == 3 {
                            for _ in 0..256 {
                                let previous = nodes.len() as u64 - 1;
                                nodes.push(HtmlNode::Element {
                                    tag: HtmlTag::Span,
                                    attributes: vec![],
                                    children: vec![previous],
                                });
                            }
                        }
                        if scenario == 5 {
                            b.cancel();
                        }
                        if scenario == 6 {
                            let mut limits = b.limits();
                            limits.work += 1;
                            *b = Budget::new(limits);
                        }
                        Ok(HtmlRequest {
                            slot: HtmlSlot::Phrasing,
                            policy: HtmlPolicy { classes: vec![] },
                            fragment: HtmlFragment {
                                root: nodes.len() as u64 - 1,
                                nodes,
                            },
                        })
                    },
                    &mut budget(),
                );
                assert_eq!(called, 1);
                match scenario {
                    0 => assert!(result.is_ok()),
                    1 => assert!(matches!(result, Err(routed::RenderError::CodeId { .. }))),
                    2 => assert!(matches!(
                        result,
                        Err(routed::RenderError::Stopped(StopReason::Cancelled))
                    )),
                    3 => assert!(matches!(
                        result,
                        Err(routed::RenderError::Document {
                            error: nepl3_doc_html::RenderError::OutputDepth { .. },
                            ..
                        })
                    )),
                    4 => assert!(matches!(
                        result,
                        Err(routed::RenderError::Destination { .. })
                    )),
                    5 => assert!(matches!(
                        result,
                        Err(routed::RenderError::Stopped(StopReason::Cancelled))
                    )),
                    6 => assert!(matches!(result, Err(routed::RenderError::ScopeMismatch))),
                    _ => unreachable!(),
                }
            }
            Ok(())
        },
    )
}
