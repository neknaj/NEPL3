use super::*;
use nepl3_tools::doc::{export::pages::{self, Entry}, source::budget};
use nepl3_core::budget::{Budget, StopReason};

#[test]
fn ordinary_export_composes_both_math_modes_without_evaluation() -> Result<(), String> {
 let c = compiled()?;
 // Zero denominator remains authored display: this operation must not evaluate.
 let source = r#"article en "Math" body cons paragraph cons sentence cons text "Before " cons math Math frac 1 0 nil cons display Math frac 2 3 cons "After" nil nil"#;
 for css in [export::CssMode::External, export::CssMode::Inline] {
  let result = export::generate_with_css(&c, source, css)?;
  assert_eq!(result.html.matches("<mfrac>").count(), 2);
  assert!(result.html.contains("display=\"inline\""));
  assert!(result.html.contains("display=\"block\""));
  assert!(!result.html.contains("<script"));
  assert!(result.html.contains("<mn>0</mn>"));
  assert_eq!(result.math.len(), 2);
  assert_ne!(result.math[0].output.node_roots, result.math[1].output.node_roots);
  let manifest: serde_json::Value = serde_json::from_str(&result.manifest).map_err(super::super::err)?;
  assert_eq!(manifest["options"]["math_renderer"], "katex-preferred");
  assert_eq!(manifest["math_diagnostics"][0]["reason"], "KaTeXHostCapabilityUnavailable");
  assert_eq!(manifest["math_diagnostics"].as_array().ok_or("diagnostics")?.len(), 2);
 }
 Ok(())
}

#[test]
fn ordinary_export_preserves_parallel_and_recursive_annotations() -> Result<(), String> {
 let c = compiled()?;
 let input = r#"article ja "注釈" body cons paragraph cons parallel cons variant ja sentence cons anno math Math frac 1 2 cons text "分数" nil nil cons variant en sentence cons text "Fraction " cons math Math frac 1 2 nil nil nil cons display Math label add x y Sentence "{[字/じ]/character}" nil"#;
 let result = export::generate(&c, input)?;
 for expected in ["Fraction", "分数", "character", "nepl-ruby"] { assert!(result.html.contains(expected)); }
 assert_eq!(result.math.len(), 3);
 for record in &result.math {
  assert!(!record.output.syntax.sources.is_empty());
  for root in &record.output.node_roots { assert!((*root as usize) < result.rendered.markup.fragment.nodes.len()); }
 }
 let record = result.math.iter().find(|m| !m.output.annotations.is_empty()).ok_or("annotation lost")?;
 let annotation = &record.output.annotations[0];
 assert!(!annotation.sentence.sources.is_empty());
 for origin in &annotation.origins {
  assert!((origin.element as usize) < result.rendered.markup.fragment.nodes.len());
  assert!((origin.node as usize) < annotation.sentence.value.nodes.len());
 }
 assert!(matches!(record.fallback, Some(export::math::Fallback::UnsupportedTex {reason:nepl3_math_tex::Unsupported::ForeignAnnotation, ..})));
 let explicit = export::generate_with_renderer(&c, input, export::CssMode::External, export::MathRenderer::MathMLOnly)?;
 assert_eq!(explicit.html, result.html);
 assert!(explicit.math.iter().all(|m| m.fallback.is_none()));
 Ok(())
}

#[test]
fn pages_share_source_admission_and_sticky_output_budget() -> Result<(), String> {
 let c = compiled()?;
 let inputs = ["one", "two"].map(|id| (Entry {id:id.into(),source:format!("{id}.nepld"),route:format!("{id}/index.html"),input:None},
 format!(r#"article en "{id}" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons display Math label add x y Sentence "[字/じ]" nil"#)));
 let mut limits = budget().limits();
 // Each source snapshot is admitted once across PageSet validation and every
 // Math occurrence. A fresh ledger per adapter would exceed this exact cap.
 limits.source_bytes = inputs.iter().map(|(_,s)| s.len() as u64).sum();
 let mut b = Budget::new(limits);
 let output = pages::generate_with_output_budget(&c, &inputs, &mut b)?;
 assert_eq!(b.usage().source_bytes, limits.source_bytes);
 let provenance = output.provenance.ok_or("provenance")?;
 for (page, records) in provenance.math.iter().enumerate() {
  assert_eq!(records.len(), 2);
  for record in records { for root in &record.output.node_roots { assert!((*root as usize) < provenance.rendered.fragments[page].markup.fragment.nodes.len()); } }
 }
 limits.work = b.usage().work - 1;
 let mut stopped = Budget::new(limits);
 assert!(pages::generate_with_output_budget(&c, &inputs, &mut stopped).is_err());
 assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
 let explicit = pages::generate_with_renderer(&c, &inputs, export::MathRenderer::MathMLOnly)?;
 assert_eq!(explicit.files, output.files);
 Ok(())
}
