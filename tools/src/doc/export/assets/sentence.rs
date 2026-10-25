//! Bounded SVG dispatch by occurrence role. Preserve the legacy alt-only scan's
//! accounting; mixed owners use explicit planned display rather than fallback.
use crate::doc::source::err;
use nepl3_core::budget::Budget;
use nepl3_doc_core::model::DocumentSyntax;
use nepl3_doc_html::{ParallelMode, RenderOptions, visibility};
pub(crate) fn requires_planned_display(
    document: &DocumentSyntax,
    budget: &mut Budget,
) -> Result<bool, String> {
    let shape = document.value.validate_shape(budget).map_err(err)?;
    // Rows checks every variant. Display selection must never hide an unsupported
    // role, and this inventory is not guest validation or namespace authority.
    let occurrences = visibility::sentence_occurrences(
        &shape,
        &RenderOptions {
            parallel: ParallelMode::Rows,
        },
        budget,
    )
    .map_err(err)?;
    Ok(!occurrences.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::source::{budget, compiled, with_input_route};
    use nepl3_core::source::{SourceAdmission, SourceStore};
    use nepl3_doc_core::{check::Category, model::*};
    use nepl3_wire::foundation::FoundationCodec;
    #[test]
    fn shared_alt_owner_in_body_requires_planned_display() -> Result<(), String> {
        let compiled = compiled()?;
        let mut doc = with_input_route(
            true,
            &compiled,
            r#"article en "Title" body cons image asset "logo" none Sentence "Alt" none nil"#,
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)
            },
        )?;
        assert!(!requires_planned_display(&doc, &mut budget())?);
        let alt = doc
            .value
            .nodes
            .iter()
            .find_map(|n| match n.kind {
                DocKind::Image { alt, .. } => Some(alt),
                _ => None,
            })
            .ok_or("alt")?;
        let index = doc.value.nodes.len() as u64;
        doc.value.nodes.push(DocNode {
            kind: DocKind::Paragraph {
                items: vec![FlowRef(alt.0)],
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        let DocRoot::Article(root) = doc.value.root else {
            return Err("article".into());
        };
        let DocKind::Article { body, .. } = doc.value.nodes[root.0 as usize].kind else {
            return Err("article".into());
        };
        let DocKind::Body { blocks } = &mut doc.value.nodes[body.0 as usize].kind else {
            return Err("body".into());
        };
        blocks.push(BlockRef(index));
        assert_eq!(requires_planned_display(&doc, &mut budget()), Ok(true));
        let mut stopped = budget();
        stopped.cancel();
        assert!(requires_planned_display(&doc, &mut stopped).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod composition_tests {
    use crate::doc::{
        export::{
            self, CssMode, DisplayOptions, MathRenderer, Stage, pages::resources::ExecutionLimits,
        },
        source::compiled,
    };
    use nepl3_doc_html::assets::{SvgInput, SvgMode};
    #[test]
    fn composed_svg_alt_uses_one_cumulative_output_allowance() -> Result<(), String> {
        let compiled = compiled()?;
        let source = r#"article en "Title" body cons image asset "logo" none Sentence "{[漢/かん]/note}" none nil"#;
        let inputs = [SvgInput {
            id: "logo",
            svg: "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>",
        }];
        for mode in [SvgMode::Embedded, SvgMode::External] {
            let run = |limits, observed: &mut Vec<_>| {
                export::generate_impl(
                    &compiled,
                    source,
                    CssMode::Inline,
                    Some((&inputs, mode)),
                    DisplayOptions {
                        math_renderer: MathRenderer::MathMLOnly,
                        ..Default::default()
                    },
                    limits,
                    &mut |stage| observed.push(stage),
                )
            };
            let mut observed = Vec::new();
            let output = run(ExecutionLimits::default(), &mut observed)?;
            let owners = output.sentence_alt_sources.as_ref().ok_or("alt owners")?;
            assert_eq!(owners.sentences.len(), 1);
            assert!(!owners.sentences[0].1.sources.is_empty());
            assert!(output.sentence_sources.is_none() && output.sentences.is_empty());
            let prepare = observed
                .iter()
                .find(|s| s.stage == Stage::Prepare)
                .ok_or("prepare")?
                .usage
                .work;
            let render = observed
                .iter()
                .find(|s| s.stage == Stage::RenderAndSerialize)
                .ok_or("render")?
                .usage
                .work;
            assert!(render > prepare && prepare > 0);
            let mut limits = ExecutionLimits::default();
            limits.output_limits.work = render;
            assert_eq!(run(limits, &mut Vec::new())?.html, output.html);
            limits.output_limits.work -= 1;
            assert!(run(limits, &mut Vec::new()).is_err());
        }
        Ok(())
    }
}

#[cfg(test)]
mod mixed;
