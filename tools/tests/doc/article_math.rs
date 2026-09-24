use super::*;
use nepl3_doc_html::{
    ParallelMode, RenderOptions, prepare_article_with_foreign, render_article_with_foreign,
};
use nepl3_tools::doc::math::MathDisplayHost;

#[test]
fn article_composes_real_math_and_its_sentence_annotation() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en sentence "Math" body cons display Math label frac 1 0 Sentence "[字/じ]" nil"#;
    with_input(&compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        let raw =
            portable::to_value(&doc, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
        let bytes = nepl3_wire::encode(&raw, &mut budget()).map_err(err)?;
        let mut fresh_admission = SourceAdmission::default();
        let mut receiver =
            FoundationCodec::new(profile.registry(), &empty, &mut fresh_admission).map_err(err)?;
        let received = portable::from_value(
            &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
            profile.registry(),
            &mut receiver,
            &mut budget(),
        )
        .map_err(err)?;
        retention::assert_doc_retention(&doc, &received)?;
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let mut original = None;
        for (document, in_namespace) in [
            (&doc, false),
            (&doc, true),
            (&received, false),
            (&received, true),
        ] {
            let prepared = prepare_article_with_foreign(
                document,
                &options,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            let member = nepl3_doc_core::labels::namespace::inspect(
                document,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let members = [&member];
            let namespace =
                nepl3_doc_core::labels::namespace::resolve(&members, &mut budget()).map_err(err)?;
            let namespace = nepl3_doc_html::namespace::prepare_with_foreign(
                &namespace,
                &options,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            let mut math_input = None;
            let mut adapter = |slot: &DocEmbed, _: EmbedRef, b: &mut nepl3_core::budget::Budget| {
                if slot.kind == EmbedKind::Sentence {
                    let sentence = nepl3_suite::adapters::document::sentence::lower(
                        slot,
                        slot.schema(),
                        &[],
                        profile.registry(),
                        &mut receiver,
                        b,
                    )
                    .map_err(err)?;
                    return nepl3_suite::adapters::sentence::html::render(
                        &sentence,
                        profile.registry(),
                        b,
                        &mut SourceAdmission::default(),
                    )
                    .map(|v| v.into_markup())
                    .map_err(err);
                }
                let mut host = MathDisplayHost {
                    registry: profile.registry(),
                    math_surface: &compiled.others[0].schema,
                    sentence_surface: Some(&compiled.others[3].schema),
                    doc_surface: None,
                    codec: &mut receiver,
                };
                let output = host
                    .render_embed(slot, b)
                    .map_err(err)?
                    .into_html(b)
                    .map_err(err)?;
                assert_eq!(output.annotations.len(), 1);
                assert!(!output.annotations[0].origins.is_empty());
                math_input = Some((output.syntax, output.annotations));
                Ok(output.markup)
            };
            let result = if in_namespace {
                let part = nepl3_doc_html::namespace::render_part_with_foreign(
                    &namespace,
                    nepl3_doc_core::labels::namespace::MemberId(0),
                    &mut adapter,
                    &mut budget(),
                )
                .map_err(err)?;
                let (_, document_digest, markup, origins) = part.part.into_parts();
                nepl3_doc_html::RenderedWithForeign {
                    fragment: nepl3_doc_html::RenderedFragment {
                        document_digest,
                        options: options.clone(),
                        markup,
                        origins,
                    },
                    foreign: part.foreign,
                }
            } else {
                render_article_with_foreign(&prepared, &mut adapter, &mut budget()).map_err(err)?
            };
            let (syntax, annotations) = math_input.ok_or("missing Math operation")?;
            assert!(!syntax.sources.is_empty());
            assert_eq!(result.foreign.len(), 2);
            let markup = &result.fragment.markup;
            let html = nepl3_markup::html::serialize(
                &nepl3_markup::html::validate(
                    &markup.fragment,
                    markup.slot,
                    &markup.policy,
                    &mut budget(),
                )
                .map_err(err)?,
                &mut budget(),
            )
            .map_err(err)?;
            // Rendering preserves the zero denominator; it does not evaluate division.
            assert!(html.contains("display=\"block\""));
            assert!(html.contains("<mfrac>"));
            assert!(html.contains("<mn>1</mn>"));
            assert!(html.contains("<mn>0</mn>"));
            assert!(html.contains("字") && html.contains("じ"));
            assert_eq!(annotations.len(), 1);
            let placement = result
                .foreign
                .iter()
                .find(|placement| {
                    document.value.embeds[placement.embed.0 as usize].kind == EmbedKind::DisplayMath
                })
                .ok_or("Math placement")?;
            let mut texts = Vec::new();
            for origin in &annotations[0].origins {
                assert!(origin.element < placement.elements);
                let element =
                    &markup.fragment.nodes[(placement.first_element + origin.element) as usize];
                if let nepl3_markup::html::HtmlNode::Text { text } = element {
                    let node = &annotations[0].sentence.value.nodes[origin.node as usize];
                    let nepl3_sentence_core::model::Kind::Text { text: expected } = node else {
                        return Err("annotation text owner".into());
                    };
                    assert_eq!(text, expected);
                    texts.push(text.as_str());
                }
            }
            assert_eq!(texts, ["字", "じ"]);
            if let Some(prior) = &original {
                assert_eq!(prior, &result.fragment);
            } else {
                original = Some(result.fragment);
            }
            let math = document
                .value
                .embeds
                .iter()
                .find(|slot| slot.kind == EmbedKind::DisplayMath)
                .ok_or("math slot")?;
            let mut host = MathDisplayHost {
                registry: profile.registry(),
                math_surface: &compiled.others[0].schema,
                sentence_surface: Some(&compiled.others[3].schema),
                doc_surface: None,
                codec: &mut receiver,
            };
            let mut code = math.clone();
            let mut inline = math.clone();
            inline.kind = EmbedKind::InlineMath;
            let inline = host.render_embed(&inline, &mut budget()).map_err(err)?;
            let xml = nepl3_markup::mathml::serialize(
                &nepl3_markup::mathml::validate(&inline.rendered.fragment, &mut budget())
                    .map_err(err)?,
                &mut budget(),
            )
            .map_err(err)?;
            assert!(xml.contains("display=\"inline\""));
            code.kind = EmbedKind::Code;
            assert!(matches!(
                host.render_embed(&code, &mut budget()),
                Err(nepl3_tools::doc::math::Error::Selection)
            ));
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                host.render_embed(math, &mut cancelled),
                Err(nepl3_tools::doc::math::Error::Stopped(
                    nepl3_core::budget::StopReason::Cancelled
                ))
            ));
        }
        Ok(())
    })
}
