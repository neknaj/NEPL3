use nepl3_markup::{html::HtmlNode, mathml::Tag};
use nepl3_tools::doc::{export, source::compiled};

pub const NESTED: &str = r#"article en Sentence "Title [漢/かん]" body cons paragraph
cons Sentence sentence cons text "Ratio " cons math frac 1 2 nil
cons code Math frac 1 2
cons Sentence sentence cons math label x Sentence sentence cons math frac 1 3 nil nil
cons Sentence sentence cons document math Math frac 4 5 nil
nil cons display Math frac 2 3 nil"#;

#[test]
fn sentence_export_retains_actual_math_roots_and_nested_owners() -> Result<(), String> {
    let compiled = compiled()?;
    let result = export::generate_with_renderer(
        &compiled,
        NESTED,
        export::CssMode::Inline,
        export::MathRenderer::MathMLOnly,
    )?;
    assert!(result.html.contains("Ratio") && result.html.contains("かん"));
    assert!(!result.html.contains("<script"));
    assert_eq!(result.sentences.len(), 4);
    assert!(result.sentence_sources.is_some());
    assert_eq!(result.math.len(), 1, "only one direct Doc Math occurrence");
    assert_eq!(
        result.math_generation.len(),
        5,
        "parent and nested formula are separate occurrences"
    );
    let mut previous = None;
    for generated in &result.math_generation {
        if let Some(previous) = previous {
            assert!(previous < generated.root);
        }
        previous = Some(generated.root);
        assert!(matches!(
            result.rendered.markup.fragment.nodes[generated.root as usize],
            HtmlNode::MathElement { tag: Tag::Math, .. }
        ));
        assert!(generated.fallback.is_none());
    }
    let preferred = export::generate_with_renderer(
        &compiled,
        NESTED,
        export::CssMode::Inline,
        export::MathRenderer::KaTeXPreferred,
    )?;
    let manifest: serde_json::Value =
        serde_json::from_str(&preferred.manifest).map_err(|e| e.to_string())?;
    let diagnostics = manifest["math_diagnostics"]
        .as_array()
        .ok_or("diagnostics")?;
    assert!(
        diagnostics
            .iter()
            .any(|d| d["reason"] == "FaithfulTexUnsupported")
    );
    assert!(
        diagnostics
            .iter()
            .all(|d| d["owner_path"].as_array().is_some_and(|p| !p.is_empty()))
    );
    if manifest["katex"].is_null() {
        assert_eq!(diagnostics.len(), 5);
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d["embed"].is_number())
                .count(),
            1
        );
    }
    Ok(())
}

#[test]
fn repeated_sentence_formulas_keep_distinct_owner_paths_and_page_support_stays_explicit()
-> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "Title" body cons paragraph cons Sentence sentence cons math frac 1 2 nil cons Sentence sentence cons math frac 1 2 nil nil nil"#;
    let result = export::generate_with_renderer(
        &compiled,
        source,
        export::CssMode::Inline,
        export::MathRenderer::MathMLOnly,
    )?;
    assert_eq!(result.math_generation.len(), 2);
    assert_ne!(
        result.math_generation[0].root,
        result.math_generation[1].root
    );
    assert_ne!(
        result.math_generation[0].owner,
        result.math_generation[1].owner
    );
    let sources = result.sentence_sources.as_ref().ok_or("Sentence sources")?;
    assert_eq!(sources.sentences.len(), result.sentences.len());
    for record in &result.sentences {
        assert!(record.owner.0 < sources.owners.len());
        assert_eq!(sources.sentences[record.occurrence.0].owner, record.owner);
    }
    let pages = [(
        export::pages::Entry {
            input: None,
            id: "typed".into(),
            source: "typed.nepld".into(),
            route: "typed.html".into(),
        },
        source.into(),
    )];
    assert!(
        export::pages::generate(&compiled, &pages).is_err(),
        "standalone typed Article support does not silently replace PageSet namespace routing"
    );
    Ok(())
}

#[test]
fn default_doc_profile_exports_cross_sentence_references_and_safe_links() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en Sentence "References" body cons paragraph
cons sentence cons ref inner text "root forward" cons anchor outer text "outer" nil
cons Sentence sentence cons document anchor inner text "guest" cons math frac 1 2 nil
cons Sentence sentence cons document ref outer text "back" cons document link external "https://example.test/reference" text "link" nil nil nil"#;
    let result = export::generate_with_renderer(
        &compiled,
        source,
        export::CssMode::Inline,
        export::MathRenderer::MathMLOnly,
    )?;
    assert!(
        result.html.contains("root forward")
            && result.html.contains("guest")
            && result.html.contains("back")
    );
    assert!(
        result
            .html
            .contains("href=\"https://example.test/reference\"")
    );
    assert_eq!(result.math_generation.len(), 1);
    assert_eq!(result.sentences.len(), 3);
    Ok(())
}
