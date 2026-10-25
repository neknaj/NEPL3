use crate::doc::{
    export::{
        self, CssMode, DisplayOptions, MathRenderer, Stage, pages::resources::ExecutionLimits,
    },
    source::compiled,
};
use nepl3_doc_html::assets::{SvgInput, SvgMode};
use nepl3_markup::{html::HtmlNode, mathml::Tag};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const SOURCE: &str = r#"article en Sentence "Title" body cons image asset "logo" none Sentence "{[漢/かん]/note}" some Sentence "Caption" cons paragraph cons sentence cons image asset "logo" none Sentence "inline & <alt>" nil cons code Doc article en "guest" body nil cons sentence cons math Math frac 1 2 nil cons Sentence sentence cons text "Body " cons math frac 3 4 nil cons Sentence sentence cons document math Math frac 5 6 nil cons Sentence sentence cons math frac 3 4 nil nil nil"#;
#[test]
fn mixed_svg_retains_real_math_owner_paths_and_both_sentence_containers() -> Result<(), String> {
    let compiled = compiled()?;
    let inputs = [SvgInput {
        id: "logo",
        svg: SVG,
    }];
    for renderer in [MathRenderer::MathMLOnly, MathRenderer::KaTeXPreferred] {
        let output = export::generate_impl(
            &compiled,
            SOURCE,
            CssMode::Inline,
            Some((&inputs, SvgMode::Embedded)),
            DisplayOptions {
                math_renderer: renderer,
                ..Default::default()
            },
            ExecutionLimits::default(),
            &mut |_| {},
        )?;
        let sources = output.sentence_sources.as_ref().ok_or("display owners")?;
        let alt = output.sentence_alt_sources.as_ref().ok_or("alt owners")?;
        assert_eq!(sources.sentences.len(), 7);
        assert_eq!(alt.sentences.len(), 2);
        assert_eq!(
            output
                .sentences
                .iter()
                .map(|r| r.occurrence.0)
                .collect::<Vec<_>>(),
            vec![0, 2, 4, 5, 6]
        );
        assert!(alt.sentences.iter().all(|(_, s)| !s.sources.is_empty()));
        assert_eq!(output.math.len(), 1);
        assert_eq!(output.math_generation.len(), 4);
        assert_eq!(output.html.matches("<math").count(), 4);
        assert_eq!(output.html.matches("<mfrac>").count(), 4);
        assert!(output.html.contains("nepl-code-marker"));
        for (index, generated) in output.math_generation.iter().enumerate() {
            assert!(matches!(
                output.rendered.markup.fragment.nodes[generated.root as usize],
                HtmlNode::MathElement { tag: Tag::Math, .. }
            ));
            assert!(
                output.math_generation[..index]
                    .iter()
                    .all(|prior| prior.root < generated.root && prior.owner != generated.owner)
            );
            if renderer == MathRenderer::MathMLOnly {
                assert!(generated.fallback.is_none());
            }
        }
        for record in &output.sentences {
            assert_eq!(sources.sentences[record.occurrence.0].owner, record.owner);
            assert!(
                record
                    .origins
                    .iter()
                    .all(|origin| origin.element
                        < output.rendered.markup.fragment.nodes.len() as u64)
            );
        }
        let manifest: serde_json::Value =
            serde_json::from_str(&output.manifest).map_err(|e| e.to_string())?;
        assert_eq!(manifest["sentence_svg"]["structural_occurrences"], 7);
        assert_eq!(manifest["sentence_svg"]["display_occurrences"], 5);
        assert_eq!(manifest["sentence_alt"]["owners"], 2);
    }
    Ok(())
}
#[test]
fn mixed_svg_output_limits_are_exact_and_failed_stages_are_not_reported() -> Result<(), String> {
    let compiled = compiled()?;
    let inputs = [SvgInput {
        id: "logo",
        svg: SVG,
    }];
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let run = |limits, stages: &mut Vec<_>| {
            export::generate_impl(
                &compiled,
                SOURCE,
                CssMode::Inline,
                Some((&inputs, mode)),
                DisplayOptions {
                    math_renderer: MathRenderer::MathMLOnly,
                    ..Default::default()
                },
                limits,
                &mut |s| stages.push(s),
            )
        };
        let mut stages = Vec::new();
        let baseline = run(ExecutionLimits::default(), &mut stages)?;
        assert_eq!(
            stages.iter().map(|s| s.stage).collect::<Vec<_>>(),
            vec![
                Stage::ParseAndValidate,
                Stage::Lower,
                Stage::Prepare,
                Stage::RenderAndSerialize
            ]
        );
        let prepare = stages[2].usage;
        let end = stages[3].usage;
        assert!(end.work > prepare.work && end.allocation_units > prepare.allocation_units);
        for dimension in 0..6 {
            for short in [false, true] {
                let less = u64::from(short);
                let mut limits = ExecutionLimits::default();
                match dimension {
                    0 => limits.output_limits.work = end.work - less,
                    1 => limits.output_limits.allocation_units = end.allocation_units - less,
                    2 => limits.output_limits.source_bytes = end.source_bytes - less,
                    3 => limits.output_limits.nodes = end.nodes - less,
                    4 => limits.output_limits.depth = end.depth - less,
                    _ => limits.output_limits.output_bytes = end.output_bytes - less,
                }
                let mut observed = Vec::new();
                let result = run(limits, &mut observed);
                if short {
                    assert!(result.is_err(), "dimension {dimension}");
                    assert!(
                        !observed
                            .iter()
                            .any(|s| s.stage == Stage::RenderAndSerialize)
                    );
                } else {
                    assert_eq!(result?.html, baseline.html);
                }
            }
        }
    }
    Ok(())
}
