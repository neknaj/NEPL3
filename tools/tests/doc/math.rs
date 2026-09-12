use super::*;
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_tools::doc::math::{Error, MathDisplayHost};

#[test]
fn article_math_nodes_select_display_without_evaluating_code() -> Result<(), String> {
    let compiled = compiled()?;
    let input = r#"article en "Math" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons display Math label add x y Doc "[字/じ]" cons code Math frac 1 0 nil"#;
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
        let options = nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        };
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
            doc_surface: Some(&compiled.doc.package.schema),
            doc_options: &options,
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
                        doc_surface: Some(&compiled.doc.package.schema),
                        doc_options: &options,
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
                        assert!(!annotation.document.sources.is_empty());
                        assert!(!annotation.origins.is_empty());
                        host.doc_surface = None;
                        assert!(matches!(
                            host.render_node(&document, i as u64, &mut budget()),
                            Err(Error::Render(
                                nepl3_math_mathml::Error::AnnotationRequiresPreparation(_)
                            ))
                        ));
                        host.doc_surface = Some(&compiled.doc.package.schema);
                    }
                    assert_eq!(
                        result.rendered.node_roots.len(),
                        result.syntax.value.nodes.len()
                    );
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
                    host.render_node(&document, i as u64, &mut budget()),
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
