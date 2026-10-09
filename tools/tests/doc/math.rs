use super::*;
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_tools::doc::math::display::request;
use nepl3_tools::doc::math::display::{Preference, TexPreparation};
use nepl3_tools::doc::math::{Error, MathDisplayHost};
pub(super) fn request_controls() -> request::Controls {
    request::Controls {
        limits: request::RenderLimits {
            input_bytes: 10000,
            output_bytes: 100000,
        },
        parse_limits: request::ParseLimits {
            input_bytes: 100000,
            nodes: 10000,
            depth: 100,
        },
        options: request::Options {
            timeout_millis: 5000,
            diagnostic_bytes: 10000,
            reply_bytes: 1000000,
            module_bytes: 953744,
        },
    }
}

#[test]
fn article_math_nodes_select_display_without_evaluating_code() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons display Math label add x y Sentence "[字/じ]" cons code Math frac 1 0 nil"#;
    with_input(&compiled, input, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut fresh = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
        let document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let raw = nepl3_doc_core::portable::to_value(
            &document,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let mut receiver_admission = SourceAdmission::default();
        let mut receiver_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut receiver_admission)
                .map_err(err)?;
        let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        let received = nepl3_doc_core::portable::from_value(
            &raw,
            profile.registry(),
            &mut receiver_codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut wrong = compiled.others[0].schema.clone();
        wrong.digest = Digest::of(b"not Math");
        let mut host = MathDisplayHost {
            registry: profile.registry(),
            math_surface: &compiled.others[0].schema,
            sentence_surface: Some(&compiled.others[3].schema),
            doc_surface: None,
            codec: &mut codec,
        };
        let mut seen = 0;
        for (i, node) in document.value.nodes.iter().enumerate() {
            match node.kind {
                DocKind::InlineMath { .. } | DocKind::DisplayMath { .. } => {
                    let result = host
                        .render_node(&document, i as u64, &mut budget())
                        .map_err(err)?;
                    let proof =
                        nepl3_markup::mathml::validate(&result.rendered.fragment, &mut budget())
                            .map_err(err)?;
                    let xml =
                        nepl3_markup::mathml::serialize(&proof, &mut budget()).map_err(err)?;
                    let mut receiver = MathDisplayHost {
                        registry: profile.registry(),
                        math_surface: &compiled.others[0].schema,
                        sentence_surface: Some(&compiled.others[3].schema),
                        doc_surface: None,
                        codec: &mut receiver_codec,
                    };
                    let received_output = receiver
                        .render_node(&received, i as u64, &mut budget())
                        .map_err(err)?;
                    let received_proof = nepl3_markup::mathml::validate(
                        &received_output.rendered.fragment,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    assert_eq!(
                        nepl3_markup::mathml::serialize(&received_proof, &mut budget())
                            .map_err(err)?,
                        xml
                    );
                    host.math_surface = &wrong;
                    assert!(matches!(
                        host.render_node(&document, i as u64, &mut budget()),
                        Err(Error::Selection)
                    ));
                    host.math_surface = &compiled.others[0].schema;
                    // Both generation paths retain the same input; annotations
                    // require MathML, whereas frac 1 0 must display without eval.
                    let display = host
                        .prepare_node(
                            &document,
                            i as u64,
                            Preference::KaTeXPreferred,
                            &mut budget(),
                        )
                        .map_err(err)?;
                    let expected_mode = if matches!(node.kind, DocKind::InlineMath { .. }) {
                        nepl3_markup::mathml::Display::Inline
                    } else {
                        nepl3_markup::mathml::Display::Block
                    };
                    assert_eq!(display.doc_node(), i as u64);
                    assert_eq!(display.display(), expected_mode);
                    assert_eq!(display.mathml().syntax, result.syntax);
                    assert_eq!(display.mathml().rendered.fragment, result.rendered.fragment);
                    if matches!(node.kind, DocKind::InlineMath { .. }) {
                        let TexPreparation::Ready { tex, occurrences } = display.tex() else {
                            return Err("fraction must prepare TeX".into());
                        };
                        assert_eq!(tex, "\\frac{1}{0}");
                        let mut encoding_budget = budget();
                        let encoded = request::prepare(
                            &display,
                            request_controls(),
                            4096,
                            &mut encoding_budget,
                        )
                        .map_err(err)?
                        .ok_or("missing request")?;
                        assert!(core::ptr::eq(encoded.owner(), &display));
                        assert_eq!(encoded.controls(), request_controls());
                        let mut copy = encoded.controls();
                        copy.limits.input_bytes = 0;
                        assert_ne!(copy, encoded.controls());

                        let value: serde_json::Value =
                            serde_json::from_slice(encoded.bytes()).map_err(err)?;
                        assert_eq!(value["version"], 1);
                        assert_eq!(value["request"]["tex"], tex.as_str());
                        assert_eq!(value["request"]["displayMode"], false);
                        assert_eq!(value["request"]["output"], "html");
                        assert_eq!(
                            encoding_budget.usage().output_bytes,
                            encoded.bytes().len() as u64
                        );
                        assert_eq!(encoding_budget.usage().allocation_units, 4096);
                        let exact = request::prepare(
                            &display,
                            request_controls(),
                            encoded.bytes().len(),
                            &mut budget(),
                        )
                        .map_err(err)?
                        .ok_or("missing exact request")?;
                        assert_eq!(exact.bytes(), encoded.bytes());
                        let mut short = budget();
                        assert!(matches!(
                            request::prepare(
                                &display,
                                request_controls(),
                                encoded.bytes().len() - 1,
                                &mut short
                            ),
                            Err(request::Error::Stopped(StopReason::OutputLimit))
                        ));
                        assert_eq!(short.poll(), Err(StopReason::OutputLimit));
                        let mut invalid = request_controls();
                        invalid.limits.input_bytes = 1u64 << 53;
                        assert!(matches!(
                            request::prepare(&display, invalid, 4096, &mut budget()),
                            Err(request::Error::InvalidControls)
                        ));
                        let mut cancelled = budget();
                        cancelled.cancel();
                        assert!(matches!(
                            request::prepare(&display, request_controls(), 4096, &mut cancelled),
                            Err(request::Error::Stopped(StopReason::Cancelled))
                        ));

                        assert!(!occurrences.is_empty());
                        for occurrence in occurrences {
                            assert!(
                                occurrence.node < display.mathml().syntax.value.nodes.len() as u64
                            );
                            assert!(
                                tex.get(occurrence.start as usize..occurrence.end as usize)
                                    .is_some()
                            );
                        }
                    } else {
                        assert!(matches!(
                            display.tex(),
                            TexPreparation::Unsupported {
                                reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
                                ..
                            }
                        ));
                        assert_eq!(display.mathml().annotations.len(), 1);
                        let mut skipped = budget();
                        assert!(
                            request::prepare(&display, request_controls(), 0, &mut skipped)
                                .map_err(err)?
                                .is_none()
                        );
                        assert_eq!(skipped.usage(), budget().usage());
                    }
                    let mut mathml_budget = budget();
                    let only = host
                        .prepare_node(
                            &document,
                            i as u64,
                            Preference::MathMLOnly,
                            &mut mathml_budget,
                        )
                        .map_err(err)?;
                    assert_eq!(only.doc_node(), i as u64);
                    assert_eq!(only.display(), expected_mode);
                    assert_eq!(only.tex(), &TexPreparation::MathMLOnly);
                    assert!(
                        request::prepare(&only, request_controls(), 0, &mut budget())
                            .map_err(err)?
                            .is_none()
                    );
                    assert_eq!(only.mathml().rendered.fragment, result.rendered.fragment);
                    // Exact MathML-only work is insufficient for the next TeX
                    // phase. The available fallback cannot turn that stop into Ok.
                    let mut limits = budget().limits();
                    limits.work = mathml_budget.usage().work;
                    let mut limited = Budget::new(limits);
                    assert!(matches!(
                        host.prepare_node(
                            &document,
                            i as u64,
                            Preference::KaTeXPreferred,
                            &mut limited,
                        ),
                        Err(Error::Stopped(nepl3_core::budget::StopReason::WorkLimit))
                    ));
                    assert_eq!(
                        limited.poll(),
                        Err(nepl3_core::budget::StopReason::WorkLimit)
                    );
                    if matches!(node.kind, DocKind::InlineMath { .. }) {
                        // Allow the independently measured MathML preparation
                        // exactly, but no additional TeX output bytes.
                        let mut limits = budget().limits();
                        limits.output_bytes = mathml_budget.usage().output_bytes;
                        let mut limited = Budget::new(limits);
                        assert!(matches!(
                            host.prepare_node(
                                &document,
                                i as u64,
                                Preference::KaTeXPreferred,
                                &mut limited,
                            ),
                            Err(Error::Stopped(nepl3_core::budget::StopReason::OutputLimit))
                        ));
                    }
                    for preference in [Preference::MathMLOnly, Preference::KaTeXPreferred] {
                        let mut cancelled = budget();
                        cancelled.stop(nepl3_core::budget::StopReason::Cancelled);
                        assert!(matches!(
                            host.prepare_node(&document, i as u64, preference, &mut cancelled,),
                            Err(Error::Stopped(nepl3_core::budget::StopReason::Cancelled))
                        ));
                    }
                    let received_display = receiver
                        .prepare_node(
                            &received,
                            i as u64,
                            Preference::KaTeXPreferred,
                            &mut budget(),
                        )
                        .map_err(err)?;
                    assert_eq!(received_display.tex(), display.tex());
                    let embed = match node.kind {
                        DocKind::InlineMath { syntax } | DocKind::DisplayMath { syntax } => syntax,
                        _ => return Err("Math kind".into()),
                    };
                    let mut bad_closure = document.value.embeds[embed.0 as usize].closure.clone();
                    bad_closure.syntax.category = "Sentence".into();
                    assert!(matches!(
                        host.render(
                            &bad_closure,
                            nepl3_markup::mathml::Display::Inline,
                            &mut budget()
                        ),
                        Err(Error::Selection)
                    ));
                    let mut broken = document.clone();
                    broken.sources.clear();
                    assert!(matches!(
                        host.render_node(&broken, i as u64, &mut budget()),
                        Err(Error::Document(_))
                    ));
                    if matches!(node.kind, DocKind::InlineMath { .. }) {
                        assert!(
                            xml.contains("display=\"inline\"><mfrac><mn>1</mn><mn>0</mn></mfrac>")
                        );
                        assert!(result.annotations.is_empty());
                    } else {
                        assert!(xml.contains("display=\"block\"><munder>"));
                        assert!(xml.contains("nepl-ruby"));
                        assert_eq!(result.annotations.len(), 1);
                        let annotation = &result.annotations[0];
                        assert!(annotation.embed < result.syntax.value.embeds.len() as u64);
                        assert!(!annotation.sentence.sources.is_empty());
                        assert!(!annotation.origins.is_empty());
                        host.sentence_surface = None;
                        assert!(matches!(
                            host.render_node(&document, i as u64, &mut budget()),
                            Err(Error::Render(
                                nepl3_math_mathml::Error::AnnotationRequiresPreparation(_)
                            ))
                        ));
                        host.sentence_surface = Some(&compiled.others[3].schema);
                    }
                    assert_eq!(
                        result.rendered.node_roots.len(),
                        result.syntax.value.nodes.len()
                    );
                    let html = result.into_html(&mut budget()).map_err(err)?;
                    let html_proof = nepl3_markup::html::validate(
                        &html.markup.fragment,
                        html.markup.slot,
                        &html.markup.policy,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    assert_eq!(
                        nepl3_markup::html::serialize_xhtml(&html_proof, &mut budget())
                            .map_err(err)?,
                        xml
                    );
                    let receiver_html = received_output.into_html(&mut budget()).map_err(err)?;
                    assert_eq!(receiver_html.markup, html.markup);
                    assert_eq!(receiver_html.node_roots, html.node_roots);
                    assert_eq!(html.annotation_roots.len(), html.annotations.len());
                    for (root, record) in html.annotation_roots.iter().zip(&html.annotations) {
                        assert!(root.node < html.syntax.value.nodes.len() as u64);
                        assert!(root.markup < html.markup.fragment.nodes.len() as u64);
                        assert_eq!(record.origins.first().map(|o| o.element), Some(root.markup));
                        for origin in &record.origins {
                            assert!(origin.element < html.markup.fragment.nodes.len() as u64);
                            assert!(origin.node < record.sentence.value.nodes.len() as u64);
                        }
                    }
                    for resource in 0..4 {
                        let mut limits = budget().limits();
                        match resource {
                            0 => limits.work = 0,
                            1 => limits.depth = 0,
                            2 => limits.nodes = 0,
                            _ => limits.allocation_units = 0,
                        }
                        let mut b = Budget::new(limits);
                        assert!(matches!(
                            host.render_node(&document, i as u64, &mut b),
                            Err(Error::Stopped(_))
                        ));
                    }
                    seen += 1;
                }
                DocKind::Code { .. } => assert!(matches!(
                    host.prepare_node(
                        &document,
                        i as u64,
                        Preference::KaTeXPreferred,
                        &mut budget()
                    ),
                    Err(Error::Node(_))
                )),
                _ => {}
            }
        }
        assert_eq!(seen, 2);
        assert!(matches!(
            host.render_node(&document, u64::MAX, &mut budget()),
            Err(Error::Node(_))
        ));
        Ok(())
    })
}

#[test]
fn block_math_request_keeps_mode_and_charges_each_encoding() -> Result<(), String> {
    check_block_request(None, false, false)
}
#[test]
#[ignore = "requires installed Node for canonical wire interoperability"]
fn prepared_math_request_matches_node_canonical_wire() -> Result<(), String> {
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    check_block_request(Some(&node), false, false)
}
#[test]
#[ignore = "requires Node and fixed Math packages"]
fn prepared_math_native_process_lifecycle() -> Result<(), String> {
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    check_block_request(Some(&node), true, false)
}
#[test]
#[ignore = "requires Node and fixed Math packages"]
fn prepared_math_composite_preserves_document_foreign_mappings() -> Result<(), String> {
    let node = std::env::var("NEPL3_TEST_NODE").unwrap_or_else(|_| "node".into());
    check_block_request(Some(&node), true, true)
}
fn check_block_request(
    node_program: Option<&str>,
    supervise: bool,
    nested_doc: bool,
) -> Result<(), String> {
    let compiled = if nested_doc {
        nepl3_tools::doc::source::compiled_with_sentence_forms(&[
            nepl3_grammar_core::compile::package::ForeignForm {
                kind: "DocumentInline",
                category: "Inline",
                spelling: "doc",
                field: "syntax",
                alias: "Doc",
                guest_category: "Inline",
                origin_reason: "composite document mapping regression",
            },
            nepl3_grammar_core::compile::package::ForeignForm {
                kind: "InlineMath",
                category: "Inline",
                spelling: "math",
                field: "syntax",
                alias: "Math",
                guest_category: "Expr",
                origin_reason: "composite nested Math regression",
            },
        ])?
    } else {
        compiled()?
    };
    let input = if nested_doc {
        r#"article en "Math" body cons display Math frac 1 0 cons display Math label add x y Sentence sentence cons ruby text "字" text "じ" cons math label x Sentence "inner" cons doc anchor target math Math label y Sentence "doc inner" cons doc ref target text "go" nil cons paragraph cons sentence cons math Math sqrt x nil nil nil"#
    } else {
        r#"article en "Math" body cons display Math frac 1 0 cons display Math label add x y Sentence sentence cons ruby text "字" text "じ" cons math label x Sentence "inner" nil cons paragraph cons sentence cons math Math sqrt x nil nil nil"#
    };
    with_input(&compiled, input, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let mut host = MathDisplayHost {
            registry: profile.registry(),
            math_surface: &compiled.others[0].schema,
            sentence_surface: Some(&compiled.others[3].schema),
            doc_surface: nested_doc.then_some(&compiled.doc.package.schema),
            codec: &mut codec,
        };
        let node = document
            .value
            .nodes
            .iter()
            .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
            .ok_or("missing block")?;
        let prepared = host
            .prepare_node(
                &document,
                node as u64,
                Preference::KaTeXPreferred,
                &mut budget(),
            )
            .map_err(err)?;
        let mut b = budget();
        let first = request::prepare(&prepared, request_controls(), 4096, &mut b)
            .map_err(err)?
            .ok_or("missing wire")?;
        let usage = b.usage();
        let wire: serde_json::Value = serde_json::from_slice(first.bytes()).map_err(err)?;
        assert_eq!(wire["request"]["displayMode"], true);
        if let Some(program) = node_program {
            let text = std::str::from_utf8(first.bytes()).map_err(err)?;
            let status = std::process::Command::new(program)
                .env_remove("NODE_OPTIONS")
                .env_remove("NODE_PATH")
                .args([
                    "-e",
                    "const s=process.argv[1]; if(JSON.stringify(JSON.parse(s))!==s)process.exit(1)",
                    text,
                ])
                .status()
                .map_err(err)?;
            assert!(status.success());
            if supervise {
                super::math_process::check(&first, program)?;
            }
        }
        let mut boundaries = request_controls();
        boundaries.options.timeout_millis = 2147483647;
        boundaries.options.reply_bytes = (1u64 << 53) - 1;
        assert!(
            request::prepare(&prepared, boundaries, 4096, &mut budget())
                .map_err(err)?
                .is_some()
        );
        for timeout in [0, 2147483648] {
            boundaries.options.timeout_millis = timeout;
            assert!(matches!(
                request::prepare(&prepared, boundaries, 4096, &mut budget()),
                Err(request::Error::InvalidControls)
            ));
        }

        let second = request::prepare(&prepared, request_controls(), 4096, &mut b)
            .map_err(err)?
            .ok_or("missing repeated wire")?;
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(b.usage().allocation_units, usage.allocation_units * 2);
        assert_eq!(b.usage().output_bytes, usage.output_bytes * 2);
        assert_eq!(b.usage().work, usage.work * 2);
        assert_eq!(b.usage().source_bytes, 0);
        assert_eq!(b.usage().nodes, 0);
        for resource in 0..3 {
            let mut limits = budget().limits();
            match resource {
                0 => limits.work = 0,
                1 => limits.allocation_units = 0,
                _ => limits.output_bytes = 0,
            }
            assert!(matches!(
                request::prepare(
                    &prepared,
                    request_controls(),
                    4096,
                    &mut Budget::new(limits)
                ),
                Err(request::Error::Stopped(_))
            ));
        }
        if supervise && let Some(program) = node_program {
            drop(first);
            drop(second);
            super::math_process::check_owned(
                &mut |preference| {
                    host.prepare_node(&document, node as u64, preference, &mut budget())
                        .map_err(err)
                },
                program,
            )?;
            let inline_node = document
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::InlineMath { .. }))
                .ok_or("inline math")?;
            super::math_process::check_owned(
                &mut |preference| {
                    host.prepare_node(&document, inline_node as u64, preference, &mut budget())
                        .map_err(err)
                },
                program,
            )?;
            let unsupported_node = document
                .value
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| matches!(n.kind, DocKind::DisplayMath { .. }))
                .nth(1)
                .ok_or("annotated math")?
                .0;
            let unsupported = host
                .prepare_node(
                    &document,
                    unsupported_node as u64,
                    Preference::KaTeXPreferred,
                    &mut budget(),
                )
                .map_err(err)?;
            if nested_doc {
                assert!(
                    unsupported.mathml().annotations[0]
                        .foreign
                        .iter()
                        .any(|record| {
                            let nepl3_tools::doc::annotations::ForeignRecord::Document(doc) =
                                record
                            else {
                                return false;
                            };
                            !doc.origins.is_empty()
                                && doc.foreign.iter().any(|math| {
                                    !math.output.node_roots.is_empty()
                                        && !math.output.annotation_roots.is_empty()
                                        && math
                                            .output
                                            .annotations
                                            .iter()
                                            .any(|annotation| !annotation.origins.is_empty())
                                })
                        })
                );
            }
            let expected = host
                .prepare_node(
                    &document,
                    unsupported_node as u64,
                    Preference::KaTeXPreferred,
                    &mut budget(),
                )
                .map_err(err)?
                .into_parts()
                .0
                .into_html(&mut budget())
                .map_err(err)?;
            super::math_process::check_owned_not_requested(unsupported, expected, program)?;
        }
        Ok(())
    })
}
