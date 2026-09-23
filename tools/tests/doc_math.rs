use nepl3_core::{
    budget::Budget,
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_tools::doc::math::display::{Preference, TexPreparation};
use nepl3_tools::doc::math::{Error, MathDisplayHost};
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;

#[test]
fn article_math_nodes_select_display_without_evaluating_code() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en sentence "Math" body cons paragraph cons sentence sentence cons doc math Math frac 1 0 nil nil cons display Math label add x y Sentence "[字/じ]" cons code Math frac 1 0 nil"#;
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
        let collect = |article: nepl3_doc_core::model::DocumentSyntax,
                       codec: &mut FoundationCodec<'_>|
         -> Result<Vec<nepl3_doc_core::model::DocumentSyntax>, String> {
            let mut documents = Vec::new();
            for embed in &article.value.embeds {
                if embed.kind != nepl3_doc_core::model::EmbedKind::Sentence {
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
                let (guests, occurrences) = selected.into_parts();
                // This input owns exactly one InlineMath guest in its body.
                assert_eq!(guests.len(), occurrences.len());
                documents.extend(guests);
            }
            documents.push(article);
            Ok(documents)
        };
        let documents = collect(document, &mut codec)?;
        let received_documents = collect(received, &mut receiver_codec)?;
        assert_eq!(documents.len(), 2);
        assert_eq!(received_documents.len(), 2);
        let mut seen = [0; 3];
        for (document, received) in documents.into_iter().zip(received_documents) {
            let mut wrong = compiled.others[0].schema.clone();
            wrong.digest = Digest::of(b"not Math");
            let mut host = MathDisplayHost {
                registry: profile.registry(),
                math_surface: &compiled.others[0].schema,
                sentence_surface: Some(&compiled.others[3].schema),
                doc_surface: None,
                codec: &mut codec,
            };
            for (i, node) in document.value.nodes.iter().enumerate() {
                match node.kind {
                    DocKind::InlineMath { .. } | DocKind::DisplayMath { .. } => {
                        let result = host
                            .render_node(&document, i as u64, &mut budget())
                            .map_err(err)?;
                        let proof = nepl3_markup::mathml::validate(
                            &result.rendered.fragment,
                            &mut budget(),
                        )
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
                        assert_eq!(display.mathml().syntax, result.syntax);
                        assert_eq!(display.mathml().rendered.fragment, result.rendered.fragment);
                        if matches!(node.kind, DocKind::InlineMath { .. }) {
                            let TexPreparation::Ready { tex, occurrences } = display.tex() else {
                                return Err("fraction must prepare TeX".into());
                            };
                            assert_eq!(tex, "\\frac{1}{0}");
                            assert!(!occurrences.is_empty());
                            for occurrence in occurrences {
                                assert!(
                                    occurrence.node
                                        < display.mathml().syntax.value.nodes.len() as u64
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
                        assert_eq!(only.tex(), &TexPreparation::MathMLOnly);
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
                            DocKind::InlineMath { syntax } | DocKind::DisplayMath { syntax } => {
                                syntax
                            }
                            _ => return Err("Math kind".into()),
                        };
                        let mut bad_closure = document.value.embeds[embed.0 as usize]
                            .syntax()
                            .ok_or("Math syntax")?
                            .clone();
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
                            assert!(xml.contains(
                                "display=\"inline\"><mfrac><mn>1</mn><mn>0</mn></mfrac>"
                            ));
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
                        let receiver_html =
                            received_output.into_html(&mut budget()).map_err(err)?;
                        assert_eq!(receiver_html.markup, html.markup);
                        assert_eq!(receiver_html.node_roots, html.node_roots);
                        assert_eq!(html.annotation_roots.len(), html.annotations.len());
                        for (root, record) in html.annotation_roots.iter().zip(&html.annotations) {
                            assert!(root.node < html.syntax.value.nodes.len() as u64);
                            assert!(root.markup < html.markup.fragment.nodes.len() as u64);
                            assert_eq!(
                                record.origins.first().map(|o| o.element),
                                Some(root.markup)
                            );
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
                        seen[usize::from(matches!(node.kind, DocKind::DisplayMath { .. }))] += 1;
                    }
                    DocKind::Code { .. } => {
                        seen[2] += 1;
                        assert!(matches!(
                            host.prepare_node(
                                &document,
                                i as u64,
                                Preference::KaTeXPreferred,
                                &mut budget()
                            ),
                            Err(Error::Node(_))
                        ));
                    }
                    _ => {}
                }
            }
            assert!(matches!(
                host.render_node(&document, u64::MAX, &mut budget()),
                Err(Error::Node(_))
            ));
        }
        assert_eq!(seen, [1, 1, 1]);
        Ok(())
    })
}
