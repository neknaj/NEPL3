//! Explicit MathML-only completion requires no renderer configuration or assets.
use super::*;
use nepl3_doc_core::{check::Category, lower, model::DocumentSyntax};
use nepl3_doc_html::{ParallelMode, RenderOptions, guests};
use nepl3_markup::html::{HtmlFragment, HtmlNode, HtmlPolicy, HtmlRequest, HtmlSlot};
use nepl3_tools::doc::math::{
    self, MathDisplayHost,
    display::{TexPreparation, generation::composite::Representation},
    occurrence,
};

fn doc(c: &Compiled, text: &str) -> Result<DocumentSyntax, String> {
    with_input(c, text, "Article", |tree, profile, _, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        lower::document(
            tree.syntax(),
            &c.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)
    })
}
fn render(
    c: &Compiled,
    doc: &DocumentSyntax,
    b: &mut Budget,
) -> Result<(String, Vec<u64>, Vec<u64>), String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = guests::prepare(doc, &options, &c.doc.registry, &mut codec, b).map_err(err)?;
    let mut math_ordinals = Vec::new();
    let mut code_ordinals = Vec::new();
    let mut source_buffers = Vec::new();
    let rendered = math::document::render(
        &prepared,
        8,
        &mut |context, b| {
            math_ordinals.push(context.ordinal());
            let scope = format!(
                "nepl-math-no-renderer-{}-<script>inert-scope</script>",
                context.ordinal()
            );
            let mut host = MathDisplayHost {
                registry: &c.doc.registry,
                math_surface: &c.others[0].schema,
                sentence_surface: Some(&c.others[3].schema),
                doc_surface: Some(&c.doc.package.schema),
                codec: &mut codec,
            };
            let composed = occurrence::mathml_only(context, &mut host, &scope, b).map_err(err)?;
            source_buffers.push((
                composed.math().math().syntax.value.nodes.as_ptr(),
                composed.math().math().annotations.as_ptr(),
            ));
            Ok(composed)
        },
        &mut |context, _| {
            code_ordinals.push(context.ordinal());
            // Minimal Code-placement fixture, not a production Code implementation.
            Ok::<_, String>(HtmlRequest {
                fragment: HtmlFragment {
                    root: 0,
                    nodes: vec![HtmlNode::Text {
                        text: "code fixture".into(),
                    }],
                },
                slot: HtmlSlot::Phrasing,
                policy: HtmlPolicy { classes: vec![] },
            })
        },
        b,
    )
    .map_err(err)?;
    for (i, imported) in rendered.math().iter().enumerate() {
        assert_eq!(imported.context().ordinal(), math_ordinals[i]);
        assert_eq!(
            imported.source().syntax.value.nodes.as_ptr(),
            source_buffers[i].0
        );
        assert_eq!(imported.source().annotations.as_ptr(), source_buffers[i].1);
        assert_eq!(
            imported.scope(),
            format!(
                "nepl-math-no-renderer-{}-<script>inert-scope</script>",
                math_ordinals[i]
            )
        );
        assert!(core::ptr::eq(imported.context().document(), doc));
        assert_eq!(imported.representation(), Representation::MathML);
        assert_eq!(imported.tex(), &TexPreparation::MathMLOnly);
        assert!(imported.selected_config().is_none());
        assert!(imported.controls().is_none());
        assert!(imported.assets().is_none());
        assert!(imported.generated().is_none());
        assert!(imported.stylesheet().is_empty());
        assert!(imported.observations().is_none());
        assert!(imported.termination_failure().is_none());
        assert!(imported.fallback().is_none());
    }
    let bundle = rendered.materialize_bundle(b).map_err(err)?;
    assert!(bundle.katex_assets().is_empty());
    assert!(bundle.math_stylesheet().is_empty());
    assert!(!bundle.html().contains("assets/katex"));
    assert!(!bundle.html().contains("assets/math.css"));
    assert!(!bundle.html().contains("<script"));
    assert!(!bundle.html().contains("inert-scope"));
    assert_eq!(bundle.source().math().len(), math_ordinals.len());
    Ok((bundle.html().into(), math_ordinals, code_ordinals))
}
#[test]
fn explicit_mathml_without_renderer_keeps_annotations_and_ordinals() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en sentence cons text "No Renderer " cons math Math frac 1 0 nil body cons code Math 1 cons display Math label x Sentence "[字/じ] {base/note}" nil"#;
    let document = doc(&c, source)?;
    let (html, math, code) = render(&c, &document, &mut budget())?;
    assert_eq!(math, vec![0, 2]);
    assert_eq!(code, vec![1]);
    assert!(html.contains("<math"));
    assert!(html.contains("class=\"nepl-ruby\""));
    assert!(html.contains("class=\"nepl-reading\">じ</span>"));
    assert!(html.contains("note"));
    let empty = doc(
        &c,
        "article en \"Plain\" body cons paragraph cons \"No renderer setup exists.\" nil nil",
    )?;
    let (_, math, code) = render(&c, &empty, &mut budget())?;
    assert!(math.is_empty());
    assert!(code.is_empty());
    Ok(())
}
#[test]
fn no_renderer_path_preserves_cumulative_boundaries_and_rejects_invalid_structure()
-> Result<(), String> {
    let c = compiled()?;
    let document = doc(
        &c,
        "article en \"T\" body cons display Math frac 1 0 cons display Math add x y nil",
    )?;
    let mut limits_without_diagnostics = budget().limits();
    limits_without_diagnostics.diagnostics = 0;
    let mut no_diagnostics = Budget::new(limits_without_diagnostics);
    render(&c, &document, &mut no_diagnostics)?;
    assert_eq!(no_diagnostics.usage().diagnostics, 0);
    let mut measured = budget();
    let expected = render(&c, &document, &mut measured)?;
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::OutputLimit,
    ] {
        let mut limits = measured.limits();
        let usage = measured.usage();
        match reason {
            StopReason::WorkLimit => limits.work = usage.work,
            StopReason::AllocationLimit => limits.allocation_units = usage.allocation_units,
            StopReason::NodeLimit => limits.nodes = usage.nodes,
            _ => limits.output_bytes = usage.output_bytes,
        }
        let mut exact = Budget::new(limits);
        assert_eq!(render(&c, &document, &mut exact)?, expected);
        assert_eq!(exact.usage(), usage);
        match reason {
            StopReason::WorkLimit => limits.work -= 1,
            StopReason::AllocationLimit => limits.allocation_units -= 1,
            StopReason::NodeLimit => limits.nodes -= 1,
            _ => limits.output_bytes -= 1,
        }
        let mut short = Budget::new(limits);
        assert!(render(&c, &document, &mut short).is_err());
        assert_eq!(short.poll(), Err(reason));
        assert!(render(&c, &document, &mut short).is_err());
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(render(&c, &document, &mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    let mut limits = measured.limits();
    limits.depth = 0;
    let mut shallow = Budget::new(limits);
    assert!(render(&c, &document, &mut shallow).is_err());
    assert_eq!(shallow.poll(), Err(StopReason::DepthLimit));
    let mut invalid = document.clone();
    invalid.value.nodes.push(invalid.value.nodes[0].clone());
    assert!(render(&c, &invalid, &mut budget()).is_err());
    Ok(())
}

#[derive(Debug)]
struct DepthFailure;
impl From<StopReason> for DepthFailure {
    fn from(_: StopReason) -> Self {
        Self
    }
}
#[test]
fn no_renderer_path_preserves_elevated_caller_depth() -> Result<(), String> {
    let c = compiled()?;
    let document = doc(&c, "article en \"T\" body cons display Math frac 1 0 nil")?;
    for base in [0, 20] {
        let run = |b: &mut Budget| {
            b.with_depth_at_least(base, |b| render(&c, &document, b).map_err(|_| DepthFailure))
        };
        let mut measured = budget();
        let expected = run(&mut measured).map_err(err)?;
        let mut limits = measured.limits();
        limits.depth = measured.usage().depth;
        assert_eq!(run(&mut Budget::new(limits)).map_err(err)?, expected);
        limits.depth -= 1;
        let mut short = Budget::new(limits);
        assert!(run(&mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::DepthLimit));
        assert_eq!(short.current_depth(), 0);
    }
    Ok(())
}
