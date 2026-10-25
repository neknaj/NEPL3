use super::*;
use nepl3_doc_core::{check::Category, model::*};
use nepl3_doc_html::{ParallelMode, RenderOptions, assets::*};
use nepl3_tools::doc::annotations::{SentenceAnnotationRenderer, alt};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const SOURCE: &str = r#"article en "Title" body cons image asset "logo" none Sentence "{[漢/かん]/note}" none cons paragraph cons sentence cons image asset "logo" none Sentence "Inline & <alt>" nil nil nil"#;
fn lower(compiled: &Compiled, source: &str) -> Result<DocumentSyntax, String> {
    with_input_route(true, compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)
    })
}
fn project(
    compiled: &Compiled,
    doc: &DocumentSyntax,
    b: &mut Budget,
) -> Result<alt::Projection, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    alt::prepare(
        doc,
        &mut SentenceAnnotationRenderer {
            registry: &compiled.doc.registry,
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        },
        b,
    )
    .map_err(err)
}
#[test]
fn sentence_alt_projects_real_meaning_without_html_callbacks() -> Result<(), String> {
    let compiled = compiled()?;
    let doc = lower(&compiled, SOURCE)?;
    let before = doc.clone();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    let projection = alt::prepare(
        &doc,
        &mut SentenceAnnotationRenderer {
            registry: &compiled.doc.registry,
            surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
            math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
            doc_surface: Some(&compiled.doc.package.schema),
            codec: &mut codec,
        },
        &mut b,
    )
    .map_err(err)?;
    assert_eq!(
        projection
            .resolved
            .iter()
            .map(|x| x.text.as_str())
            .collect::<Vec<_>>(),
        vec!["漢", "Inline & <alt>"]
    );
    assert_eq!(projection.sentences.len(), 2);
    assert!(
        projection
            .sentences
            .iter()
            .all(|(_, s)| !s.sources.is_empty() && !s.locations.is_empty())
    );
    assert_eq!(doc, before);
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let inputs = [SvgInput {
        id: "logo",
        svg: SVG,
    }];
    assert!(matches!(
        prepare_svg_display(
            &doc,
            &options,
            &inputs,
            SvgMode::Embedded,
            &compiled.doc.registry,
            &mut codec,
            &mut b
        ),
        Err(AssetError::Alt)
    ));
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let prepared = prepare_svg_display_with_text(
            &doc,
            &options,
            &inputs,
            mode,
            &projection.resolved,
            &compiled.doc.registry,
            &mut codec,
            &mut b,
        )
        .map_err(err)?;
        let output = render_svg_display(
            &prepared,
            &mut |_, _, _| Err::<_, String>("alt must not call HTML".into()),
            &mut b,
        )
        .map_err(err)?;
        let markup = &output.fragment.markup;
        let checked =
            nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, &mut b)
                .map_err(err)?;
        let html = nepl3_markup::html::serialize(&checked, &mut b).map_err(err)?;
        assert!(html.contains("alt=\"漢\""));
        assert!(html.contains("alt=\"Inline &amp; &lt;alt&gt;\""));
        assert!(!html.contains("かん") && !html.contains("note"));
        assert!(output.foreign.is_empty());
    }
    {
        let mut empty_admission = SourceAdmission::default();
        let mut empty_codec =
            FoundationCodec::new(&compiled.doc.registry, &store, &mut empty_admission)
                .map_err(err)?;
        let empty = lower(&compiled, r#"article en "Title" body nil"#)?;
        assert!(matches!(
            prepare_svg_display_with_text(
                &empty,
                &options,
                &[],
                SvgMode::Embedded,
                &projection.resolved,
                &compiled.doc.registry,
                &mut empty_codec,
                &mut b
            ),
            Err(AssetError::Alt)
        ));
    }
    for mismatch in 0..4 {
        let mut wrong = projection.resolved.clone();
        match mismatch {
            0 => wrong[0].document_digest = Digest::of(b"wrong"),
            1 => wrong[0].guest_digest = Digest::of(b"wrong"),
            2 => wrong[0].embed = EmbedRef(u64::MAX),
            _ => wrong.push(wrong[0].clone()),
        }
        assert!(matches!(
            prepare_svg_display_with_text(
                &doc,
                &options,
                &inputs,
                SvgMode::Embedded,
                &wrong,
                &compiled.doc.registry,
                &mut codec,
                &mut b
            ),
            Err(AssetError::Alt)
        ));
    }
    Ok(())
}
#[test]
fn sentence_alt_budget_is_cumulative_and_foreign_text_is_not_invented() -> Result<(), String> {
    let compiled = compiled()?;
    let doc = lower(&compiled, SOURCE)?;
    let mut b = budget();
    b.charge(Resource::Work, 123).map_err(err)?;
    let projection = project(&compiled, &doc, &mut b)?;
    let used = b.usage().work;
    let mut limits = budget().limits();
    limits.work = used;
    let mut exact = Budget::new(limits);
    exact.charge(Resource::Work, 123).map_err(err)?;
    assert_eq!(
        project(&compiled, &doc, &mut exact)?.resolved,
        projection.resolved
    );
    limits.work -= 1;
    let mut short = Budget::new(limits);
    short.charge(Resource::Work, 123).map_err(err)?;
    assert!(project(&compiled, &doc, &mut short).is_err());
    let mut shared = doc.clone();
    let image = shared
        .value
        .nodes
        .iter()
        .position(|n| matches!(n.kind, DocKind::InlineImage { .. }))
        .ok_or("inline image")? as u64;
    let sentence = shared.value.nodes.iter().position(|n| matches!(&n.kind, DocKind::Sentence { inlines } if inlines.contains(&InlineRef(image)))).ok_or("image sentence")?;
    let mut child = image;
    for _ in 0..12 {
        let next = shared.value.nodes.len() as u64;
        shared.value.nodes.push(DocNode {
            kind: DocKind::Concat {
                inlines: vec![InlineRef(child)],
            },
            locations: vec![],
            origin: None,
            span: None,
        });
        child = next;
    }
    let DocKind::Sentence { inlines } = &mut shared.value.nodes[sentence].kind else {
        return Err("sentence".into());
    };
    inlines.push(InlineRef(child));
    let mut shared_budget = budget();
    let shared_projection = project(&compiled, &shared, &mut shared_budget)?;
    assert_eq!(
        shared_projection.sentences.len(),
        2,
        "shared alt owner retained once"
    );
    assert!(shared_budget.usage().depth > b.usage().depth);
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(project(&compiled, &doc, &mut cancelled).is_err());
    let mut nested = budget();
    nested
        .with_depth_at_least(7, |b| Ok::<_, StopReason>(project(&compiled, &doc, b)))
        .map_err(err)??;
    let depth = nested.usage().depth;
    assert!(depth >= b.usage().depth + 7);
    let mut depth_limits = budget().limits();
    depth_limits.depth = depth;
    let mut exact_depth = Budget::new(depth_limits);
    exact_depth
        .with_depth_at_least(7, |b| Ok::<_, StopReason>(project(&compiled, &doc, b)))
        .map_err(err)??;
    depth_limits.depth -= 1;
    let mut short_depth = Budget::new(depth_limits);
    assert!(
        short_depth
            .with_depth_at_least(7, |b| Ok::<_, StopReason>(project(&compiled, &doc, b)))
            .map_err(err)?
            .is_err()
    );
    let source = r#"article en "Title" body cons image asset "logo" none Sentence sentence cons math frac 1 2 nil none nil"#;
    let foreign = lower(&compiled, source)?;
    let failure = match project(&compiled, &foreign, &mut budget()) {
        Err(error) => error,
        Ok(_) => return Err("unresolved foreign text was accepted".into()),
    };
    assert!(failure.contains("Text(Unresolved"), "{failure}");
    let hidden = lower(
        &compiled,
        r#"article en "Title" body cons image asset "logo" none Sentence sentence cons anno text "base" cons math frac 1 2 nil nil none nil"#,
    )?;
    assert_eq!(
        project(&compiled, &hidden, &mut budget())?.resolved[0].text,
        "base"
    );
    Ok(())
}
