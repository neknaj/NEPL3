use super::*;
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_tools::doc::math::{
    MathDisplayHost,
    markdown::{EmitError, Placement, PrepareError},
};

#[test]
fn markdown_math_prepares_real_source_and_rejects_loss_or_substitution() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons display Math frac 1 0 cons display Math label add x y Sentence "[字/じ]" cons code Math frac 1 0 nil"#;
    with_input(&compiled, input, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let copy = document.clone();
        let mut host = MathDisplayHost {
            registry: profile.registry(),
            math_surface: &compiled.others[0].schema,
            sentence_surface: Some(&compiled.others[3].schema),
            doc_surface: None,
            codec: &mut codec,
        };
        let mut ready = 0;
        let mut unsupported = 0;
        let mut code = 0;
        for (n, node) in document.value.nodes.iter().enumerate() {
            let n = n as u64;
            match node.kind {
                DocKind::InlineMath { .. } | DocKind::DisplayMath { .. } => {
                    let mut b = budget();
                    let prepared = match host.prepare_markdown_node(&document, n, &mut b) {
                        Ok(prepared) => prepared,
                        Err(PrepareError::Unsupported {
                            reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
                            ..
                        }) => {
                            unsupported += 1;
                            continue;
                        }
                        Err(e) => return Err(err(e)),
                    };
                    let display = matches!(node.kind, DocKind::DisplayMath { .. });
                    let placement = if display {
                        Placement::DisplayBlock
                    } else {
                        Placement::Inline
                    };
                    // Expected TeX follows the explicit fraction constructor, not
                    // evaluation (division by zero) or an HTML-to-text fallback.
                    let expected = if display {
                        "```math\n\\frac{1}{0}\n```"
                    } else {
                        "$`\\frac{1}{0}`$"
                    };
                    assert_eq!(
                        prepared.emit(&document, n, placement, &mut b),
                        Ok(expected.into())
                    );
                    assert_eq!(
                        prepared.emit(&copy, n, placement, &mut b),
                        Err(EmitError::InputMismatch)
                    );
                    assert_eq!(
                        prepared.emit(&document, u64::MAX, placement, &mut b),
                        Err(EmitError::InputMismatch)
                    );
                    let wrong = if display {
                        Placement::Inline
                    } else {
                        Placement::DisplayBlock
                    };
                    assert_eq!(
                        prepared.emit(&document, n, wrong, &mut b),
                        Err(EmitError::Placement)
                    );
                    if !display {
                        assert_eq!(
                            prepared.emit(&document, n, Placement::TableCell, &mut b),
                            Ok(expected.into())
                        );
                    }
                    let mut limits = b.limits();
                    limits.work -= 1;
                    assert_eq!(
                        prepared.emit(&document, n, placement, &mut Budget::new(limits)),
                        Err(EmitError::LimitsMismatch)
                    );
                    b.cancel();
                    assert_eq!(
                        prepared.emit(&document, n, placement, &mut b),
                        Err(EmitError::Stopped(StopReason::Cancelled))
                    );
                    ready += 1;
                }
                DocKind::Code { .. } => {
                    assert!(matches!(
                        host.prepare_markdown_node(&document, n, &mut budget()),
                        Err(PrepareError::Host(nepl3_tools::doc::math::Error::Node(_)))
                    ));
                    code += 1;
                }
                _ => {}
            }
        }
        assert_eq!((ready, unsupported, code), (2, 1, 1));
        assert_eq!(document, copy);
        Ok(())
    })
}

#[test]
fn markdown_math_preserves_table_fences_and_sticky_budget_stops() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math fence "|" "|" x nil nil nil"#;
    with_input(&compiled, input, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let n = document
            .value
            .nodes
            .iter()
            .position(|n| matches!(n.kind, DocKind::InlineMath { .. }))
            .ok_or("missing math")? as u64;
        let mut host = MathDisplayHost {
            registry: profile.registry(),
            math_surface: &compiled.others[0].schema,
            sentence_surface: Some(&compiled.others[3].schema),
            doc_surface: None,
            codec: &mut codec,
        };
        let mut measured = budget();
        let prepared = host
            .prepare_markdown_node(&document, n, &mut measured)
            .map_err(err)?;
        let preparation = measured.usage();
        let rendered = prepared
            .emit(&document, n, Placement::Inline, &mut measured)
            .map_err(err)?;
        assert_eq!(rendered, "$`\\left| \\mathord{\\textit{x}}\\right| `$");
        let complete_usage = measured.usage();
        for resource in 0..3 {
            let mut limits = budget().limits();
            match resource {
                0 => limits.work = complete_usage.work,
                1 => limits.allocation_units = complete_usage.allocation_units,
                _ => limits.output_bytes = complete_usage.output_bytes,
            }
            let mut exact = Budget::new(limits);
            let prepared = host
                .prepare_markdown_node(&document, n, &mut exact)
                .map_err(err)?;
            assert_eq!(
                prepared.emit(&document, n, Placement::Inline, &mut exact),
                Ok(rendered.clone())
            );
            assert_eq!(exact.usage(), complete_usage);
        }

        assert_eq!(
            prepared.emit(&document, n, Placement::TableCell, &mut measured),
            Err(EmitError::UnsafeDelimiter)
        );
        // Each cap admits the entire preparation but fails the final fragment.
        // This checks cumulative usage, not an independently reset emit budget.
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = preparation.work,
                StopReason::AllocationLimit => {
                    limits.allocation_units = preparation.allocation_units
                }
                StopReason::OutputLimit => {
                    limits.output_bytes = preparation.output_bytes + rendered.len() as u64 - 1
                }
                _ => return Err("unexpected resource".into()),
            }
            let mut b = Budget::new(limits);
            let prepared = host
                .prepare_markdown_node(&document, n, &mut b)
                .map_err(err)?;
            assert_eq!(
                prepared.emit(&document, n, Placement::Inline, &mut b),
                Err(EmitError::Stopped(reason))
            );
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(
                prepared.emit(&document, n, Placement::Inline, &mut b),
                Err(EmitError::Stopped(reason))
            );
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            host.prepare_markdown_node(&document, n, &mut cancelled),
            Err(PrepareError::Host(nepl3_tools::doc::math::Error::Stopped(
                StopReason::Cancelled
            )))
        ));
        Ok(())
    })
}

#[test]
fn markdown_math_does_not_emit_empty_delimiter_runs() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math sequence nil nil nil cons display Math sequence nil nil"#;
    with_input(&compiled, input, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
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
            doc_surface: None,
            codec: &mut codec,
        };
        let mut seen = 0;
        for (n, node) in document.value.nodes.iter().enumerate() {
            let placement = match node.kind {
                DocKind::InlineMath { .. } => Placement::Inline,
                DocKind::DisplayMath { .. } => Placement::DisplayBlock,
                _ => continue,
            };
            let mut b = budget();
            let prepared = host
                .prepare_markdown_node(&document, n as u64, &mut b)
                .map_err(err)?;
            assert_eq!(
                prepared.emit(&document, n as u64, placement, &mut b),
                Err(EmitError::EmptyExpression)
            );
            seen += 1;
        }
        assert_eq!(seen, 2);
        Ok(())
    })
}

#[test]
fn markdown_math_preserves_literal_payload_through_independent_markdown_parser()
-> Result<(), String> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
    let compiled = compiled()?;
    // Hostile-looking text is a literal Math Text node, not TeX source. Existing
    // structural TeX escaping runs first; Markdown must not reinterpret it.
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math text "日本 $ [x](y) *z* _a_ # & <tag> \\input{q}" nil nil cons display Math text "日本 $ [x](y) *z* _a_ # & <tag> \\input{q}" nil"#;
    with_input(&compiled, input, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = lower::document(
            tree.syntax(),
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
            doc_surface: None,
            codec: &mut codec,
        };
        let mut seen = 0;
        for (n, node) in document.value.nodes.iter().enumerate() {
            let placement = match node.kind {
                DocKind::InlineMath { .. } => Placement::Inline,
                DocKind::DisplayMath { .. } => Placement::DisplayBlock,
                _ => continue,
            };
            let display = host
                .prepare_node(
                    &document,
                    n as u64,
                    nepl3_tools::doc::math::display::Preference::KaTeXPreferred,
                    &mut budget(),
                )
                .map_err(err)?;
            let nepl3_tools::doc::math::display::TexPreparation::Ready { tex, .. } = display.tex()
            else {
                return Err("expected literal TeX".into());
            };
            let mut b = budget();
            let prepared = host
                .prepare_markdown_node(&document, n as u64, &mut b)
                .map_err(err)?;
            let markdown = prepared
                .emit(&document, n as u64, placement, &mut b)
                .map_err(err)?;
            let mut payload = String::new();
            let mut delimiters = String::new();
            let mut math_fence = false;
            for event in Parser::new(&markdown) {
                match event {
                    Event::Code(text) if placement == Placement::Inline => payload.push_str(&text),
                    Event::Text(text) if placement == Placement::DisplayBlock => {
                        payload.push_str(&text)
                    }
                    Event::Text(text) => delimiters.push_str(&text),
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                        assert_eq!(info.as_ref(), "math");
                        math_fence = true;
                    }
                    Event::Start(Tag::Paragraph) | Event::End(_) => {}
                    other => return Err(format!("unexpected Markdown interpretation: {other:?}")),
                }
            }
            if placement == Placement::Inline {
                assert_eq!(payload, *tex);
                assert_eq!(delimiters, "$$");
                assert!(!math_fence);
            } else {
                assert_eq!(payload, format!("{tex}\n"));
                assert!(math_fence);
            }
            seen += 1;
        }
        assert_eq!(seen, 2);
        // This parser checks delimiter/payload preservation, not GitHub MathJax
        // execution, mathematical equivalence, or actual browser rendering.
        Ok(())
    })
}
