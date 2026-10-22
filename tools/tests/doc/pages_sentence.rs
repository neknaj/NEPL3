use nepl3_tools::doc::{
    export::{self, pages},
    source::compiled,
};
const ONE: &str = r#"article en Sentence "One" body cons paragraph
cons sentence cons anchor outer text "outer" nil
cons Sentence sentence cons document link page "two" some "guest" text "next" cons math frac 1 2 nil nil nil"#;
const TWO: &str = r#"article en Sentence "Two" body cons paragraph
cons Sentence sentence cons document anchor guest text "guest target" cons document link relative "../one.nepld" some "outer" text "back" nil nil nil"#;
fn entries(one: &str, two: &str) -> Vec<(pages::Entry, String)> {
    vec![
        (
            pages::Entry {
                input: None,
                id: "one".into(),
                source: "docs/one.nepld".into(),
                route: "first/index.html".into(),
            },
            one.into(),
        ),
        (
            pages::Entry {
                input: None,
                id: "two".into(),
                source: "docs/nested/two.nepld".into(),
                route: "other/deep.html".into(),
            },
            two.into(),
        ),
    ]
}
#[test]
fn typed_pages_emit_cross_member_routes_and_retain_sentence_math() -> Result<(), String> {
    let compiled = compiled()?;
    let generated = pages::generate_with_renderer(
        &compiled,
        &entries(ONE, TWO),
        export::MathRenderer::MathMLOnly,
    )?;
    let first = String::from_utf8(generated.files["first/index.html"].clone())
        .map_err(|e| e.to_string())?;
    let second =
        String::from_utf8(generated.files["other/deep.html"].clone()).map_err(|e| e.to_string())?;
    assert!(first.contains("../other/deep.html#"), "{first}");
    assert!(second.contains("../first/index.html#"), "{second}");
    assert!(second.contains("guest target"));
    assert!(first.contains("<math"));
    assert!(!first.contains("<script"));
    let provenance = generated.provenance.ok_or("provenance")?;
    assert_eq!(provenance.sentence_sources.len(), 2);
    assert_eq!(provenance.sentences.len(), 2);
    assert_eq!(provenance.math_generation[0].len(), 1);
    for (sources, records) in provenance
        .sentence_sources
        .iter()
        .zip(&provenance.sentences)
    {
        assert_eq!(sources.sentences.len(), records.len());
        for record in records {
            assert_eq!(sources.sentences[record.occurrence.0].owner, record.owner);
        }
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&generated.manifest).map_err(|e| e.to_string())?;
    assert_eq!(manifest["renderer"], "nepl3-doc-html sentence-pages/1");
    Ok(())
}
#[test]
fn typed_pages_reject_missing_guest_fragment_and_unsafe_guest_uri() -> Result<(), String> {
    let compiled = compiled()?;
    let result = pages::generate(
        &compiled,
        &entries(&ONE.replace("some \"guest\"", "some \"absent\""), TWO),
    );
    assert!(result.is_err());
    let bad = TWO.replace(
        "link relative \"../one.nepld\" some \"outer\"",
        "link external \"javascript:bad\"",
    );
    assert!(pages::generate(&compiled, &entries(ONE, &bad)).is_err());
    Ok(())
}

#[test]
fn typed_page_operation_preserves_shared_limits_and_deeper_caller() -> Result<(), String> {
    use nepl3_core::budget::{Budget, Resource};
    use nepl3_tools::doc::source::budget;
    let compiled = compiled()?;
    let input = entries(ONE, TWO);
    let mut measured = budget();
    measured
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    pages::generate_with_output_budget(&compiled, &input, &mut measured)?;
    let mut limits = measured.limits();
    limits.work = measured.usage().work;
    let mut exact = Budget::new(limits);
    exact
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    pages::generate_with_output_budget(&compiled, &input, &mut exact)?;
    limits.work -= 1;
    let mut short = Budget::new(limits);
    short
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    assert!(pages::generate_with_output_budget(&compiled, &input, &mut short).is_err());
    assert!(short.poll().is_err());
    let mut deeper = budget();
    let depth = deeper.current_depth();
    deeper
        .with_depth_at_least(3, |b| {
            Ok::<_, nepl3_core::budget::StopReason>(pages::generate_with_output_budget(
                &compiled, &input, b,
            ))
        })
        .map_err(|e| format!("{e:?}"))??;
    assert_eq!(deeper.current_depth(), depth);
    Ok(())
}

#[test]
fn typed_pages_and_code_preserve_cumulative_source_limits() -> Result<(), String> {
    let compiled = compiled()?;
    let one = ONE.replace(
        "cons Sentence sentence",
        "cons code Math frac 3 4 cons Sentence sentence",
    );
    let inputs = entries(&one, TWO);
    let mut budget = nepl3_tools::doc::source::budget();
    let generated = pages::generate_with_output_budget(&compiled, &inputs, &mut budget)?;
    // Code preparation also performs bounded syntax analysis; those earlier
    // admissions remain charged rather than being erased by display dispatch.
    assert!(budget.usage().source_bytes >= (one.len() + TWO.len()) as u64);
    let mut limits = budget.limits();
    limits.source_bytes = budget.usage().source_bytes;
    pages::generate_with_output_budget(
        &compiled,
        &inputs,
        &mut nepl3_core::budget::Budget::new(limits),
    )?;
    limits.source_bytes -= 1;
    let mut short = nepl3_core::budget::Budget::new(limits);
    assert!(pages::generate_with_output_budget(&compiled, &inputs, &mut short).is_err());
    assert!(short.poll().is_err());
    let provenance = generated.provenance.ok_or("provenance")?;
    assert_eq!(
        provenance.math_generation[0].len(),
        1,
        "Code must not become an evaluated formula"
    );
    Ok(())
}

#[test]
fn later_page_namespace_errors_keep_the_owning_page() -> Result<(), String> {
    let compiled = compiled()?;
    let duplicate = TWO.replace(
        "cons document link relative",
        "cons document anchor guest text \"duplicate\" cons document link relative",
    );
    let unresolved = TWO.replace("anchor guest", "ref missing");
    for source in [duplicate, unresolved] {
        let failure = pages::generate(&compiled, &entries(ONE, &source))
            .err()
            .ok_or("unexpected success")?;
        assert!(failure.contains("page 1 (two): namespace:"), "{failure}");
    }
    Ok(())
}
