use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_tools::doc::{
    export::{self, pages},
    source::{Compiled, budget, compiled},
};
const ONE: &str = r#"article en Sentence "First" body cons paragraph
cons code Math frac 1 2 cons parallel
cons variant en Sentence sentence cons document link page "two" some "english" text "English link" nil
cons variant ja Sentence sentence cons document link page "two" some "japanese" text "Japanese link" nil nil nil nil"#;
const TWO: &str = r#"article en Sentence "Second" body cons paragraph cons parallel
cons variant en Sentence sentence cons document anchor english text "English target" cons math frac 1 2 nil
cons variant ja Sentence sentence cons document anchor japanese text "Japanese target" cons math frac 3 4 nil nil nil nil"#;
fn entries(one: &str, two: &str) -> Vec<(pages::Entry, String)> {
    [
        ("one", "docs/one.nepld", "first/index.html", one),
        ("two", "docs/nested/two.nepld", "other/deep.html", two),
    ]
    .into_iter()
    .map(|(id, source, route, text)| {
        (
            pages::Entry {
                id: id.into(),
                source: source.into(),
                route: route.into(),
                input: None,
            },
            text.into(),
        )
    })
    .collect()
}
fn mode(language: &str, fallbacks: &[&str]) -> ParallelMode {
    ParallelMode::Single {
        language: language.into(),
        fallbacks: fallbacks.iter().map(|s| (*s).into()).collect(),
    }
}
fn generate(
    c: &Compiled,
    inputs: &[(pages::Entry, String)],
    parallel: ParallelMode,
    b: &mut Budget,
) -> Result<pages::GeneratedPages, String> {
    pages::generate_with_display_options(
        c,
        inputs,
        &[],
        Default::default(),
        b,
        pages::DisplayOptions {
            document: RenderOptions { parallel },
            math_renderer: export::MathRenderer::MathMLOnly,
        },
    )
}
#[test]
fn single_pages_keep_routes_structural_owners_math_and_option_identity() -> Result<(), String> {
    let c = compiled()?;
    let inputs = entries(ONE, TWO);
    let mut identities = Vec::new();
    for (language, fallbacks, english) in [
        ("en", vec![], true),
        ("ja", vec![], false),
        ("fr", vec!["ja", "en"], false),
        ("fr", vec!["en", "ja"], true),
    ] {
        let options = mode(language, &fallbacks);
        let result = generate(&c, &inputs, options.clone(), &mut budget())?;
        let first =
            std::str::from_utf8(&result.files["first/index.html"]).map_err(|e| e.to_string())?;
        let second =
            std::str::from_utf8(&result.files["other/deep.html"]).map_err(|e| e.to_string())?;
        assert_eq!(first.contains("English link"), english);
        assert_eq!(first.contains("Japanese link"), !english);
        assert_eq!(second.contains("English target"), english);
        assert_eq!(second.contains("Japanese target"), !english);
        let href = first
            .split("href=\"")
            .find(|s| s.starts_with("../other/deep.html#"))
            .ok_or("routed link")?
            .split('"')
            .next()
            .ok_or("href")?;
        let fragment = href.strip_prefix("../other/deep.html#").ok_or("fragment")?;
        assert!(second.contains(&format!("id=\"{fragment}\"")));
        let p = result.provenance.as_ref().ok_or("provenance")?;
        assert_eq!(p.math_generation[1].len(), 1);
        for (sources, records) in p.sentence_sources.iter().zip(&p.sentences) {
            assert_eq!(sources.sentences.len(), 3);
            assert_eq!(
                records.iter().map(|r| r.occurrence.0).collect::<Vec<_>>(),
                [0, if english { 1 } else { 2 }]
            );
            for record in records {
                assert_eq!(record.owner, sources.sentences[record.occurrence.0].owner);
            }
        }
        assert_eq!(
            p.sentences[0]
                .iter()
                .map(|r| r.placement)
                .collect::<Vec<_>>(),
            [0, 2]
        );
        let m: serde_json::Value =
            serde_json::from_str(&result.manifest).map_err(|e| e.to_string())?;
        assert_eq!(m["options"]["parallel"], "Single");
        assert_eq!(m["options"]["language"], language);
        assert_eq!(m["options"]["fallbacks"], serde_json::json!(fallbacks));
        assert!(
            m["identity_contract"]
                .as_str()
                .ok_or("identity contract")?
                .contains("sentence-single/1")
        );
        let repeated = generate(&c, &inputs, options, &mut budget())?;
        assert_eq!(
            p.rendered.identity,
            repeated.provenance.ok_or("repeat")?.rendered.identity
        );
        assert!(!identities.contains(&p.rendered.identity));
        identities.push(p.rendered.identity);
    }
    let old = pages::generate_with_renderer(&c, &inputs, export::MathRenderer::MathMLOnly)?;
    let rows = generate(&c, &inputs, ParallelMode::Rows, &mut budget())?;
    assert_eq!(old.files, rows.files);
    assert_eq!(
        old.provenance.ok_or("old")?.rendered.identity,
        rows.provenance.as_ref().ok_or("rows")?.rendered.identity
    );
    let columns = generate(&c, &inputs, ParallelMode::Columns, &mut budget())?;
    assert_ne!(
        rows.provenance.ok_or("rows")?.rendered.identity,
        columns.provenance.ok_or("columns")?.rendered.identity
    );
    Ok(())
}
#[test]
fn single_pages_reject_hidden_targets_and_invalid_selection_and_accept_legacy_peer()
-> Result<(), String> {
    let c = compiled()?;
    let bad = ONE.replace("some \"japanese\"", "some \"english\"");
    let error = generate(&c, &entries(&bad, TWO), mode("ja", &[]), &mut budget())
        .err()
        .ok_or("hidden target accepted")?;
    assert!(error.contains("MissingAnchor"), "{error}");
    for options in [mode("fr", &[]), mode("en", &["EN"]), mode("", &[])] {
        assert!(generate(&c, &entries(ONE, TWO), options, &mut budget()).is_err());
    }
    let legacy = r#"article en "Legacy" body cons paragraph cons parallel cons variant en sentence cons anchor english text "English target" nil cons variant ja sentence cons anchor japanese text "Japanese target" nil nil nil nil"#;
    let result = generate(&c, &entries(ONE, legacy), mode("ja", &[]), &mut budget())?;
    let p = result.provenance.ok_or("legacy provenance")?;
    assert!(p.sentence_sources[1].sentences.is_empty());
    assert!(p.sentences[1].is_empty());
    Ok(())
}
#[test]
fn single_pages_share_budget_through_selection_identity_and_output() -> Result<(), String> {
    let c = compiled()?;
    let inputs = entries(ONE, TWO);
    let options = mode("fr", &["ja", "en"]);
    let run = |b: &mut Budget| generate(&c, &inputs, options.clone(), b);
    let mut measured = budget();
    measured
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    run(&mut measured)?;
    let mut limits = measured.limits();
    limits.work = measured.usage().work;
    let mut exact = Budget::new(limits);
    exact
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    run(&mut exact)?;
    limits.work -= 1;
    let mut short = Budget::new(limits);
    short
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    assert!(run(&mut short).is_err());
    assert_eq!(short.poll(), Err(StopReason::WorkLimit));
    let mut source_limits = measured.limits();
    source_limits.source_bytes = measured.usage().source_bytes;
    run(&mut Budget::new(source_limits))?;
    source_limits.source_bytes -= 1;
    let mut short = Budget::new(source_limits);
    assert!(run(&mut short).is_err());
    assert_eq!(short.poll(), Err(StopReason::SourceLimit));
    let mut deeper = budget();
    deeper
        .with_depth_at_least(3, |b| Ok::<_, StopReason>(run(b)))
        .map_err(|e| format!("{e:?}"))??;
    assert_eq!(deeper.current_depth(), 0);
    let mut stopped = budget();
    stopped.cancel();
    assert!(run(&mut stopped).is_err());
    assert_eq!(stopped.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn legacy_only_pages_keep_input_identity_but_distinguish_display_execution() -> Result<(), String> {
    let c = compiled()?;
    let source = r#"article en "Legacy" body cons paragraph cons parallel cons variant en sentence cons text "English" nil cons variant ja sentence cons text "Japanese" nil nil nil nil"#;
    let inputs = entries(source, source);
    let mut input_identity = None;
    let mut executions = Vec::new();
    let mut phases = Vec::new();
    for selected in [
        ParallelMode::Rows,
        ParallelMode::Columns,
        mode("en", &[]),
        mode("ja", &[]),
        mode("fr", &["en", "ja"]),
        mode("fr", &["ja", "en"]),
    ] {
        let result = generate(&c, &inputs, selected.clone(), &mut budget())?;
        let identity = result
            .provenance
            .as_ref()
            .ok_or("provenance")?
            .rendered
            .identity;
        if let Some(original) = input_identity {
            assert_eq!(identity, original);
        } else {
            input_identity = Some(identity);
        }
        let m: serde_json::Value =
            serde_json::from_str(&result.manifest).map_err(|e| e.to_string())?;
        let mut base = b"nepl3.local-doc-pages.execution/2\0".to_vec();
        base.extend_from_slice(&identity.0);
        base.push(1); // MathMLOnly.
        for section in ["limits", "initial_usage"] {
            for field in [
                "source_bytes",
                "work",
                "depth",
                "nodes",
                "allocation_units",
                "output_bytes",
                "diagnostics",
                "events",
            ] {
                base.extend_from_slice(
                    &m["output_budget"][section][field]
                        .as_u64()
                        .ok_or("budget field")?
                        .to_be_bytes(),
                );
            }
        }
        let base = nepl3_core::source::Digest::of(&base);
        let expected = if matches!(selected, ParallelMode::Rows) {
            assert_eq!(
                m["output_budget"]["contract"],
                "nepl3.local-doc-pages.execution/2"
            );
            base
        } else {
            assert_eq!(
                m["output_budget"]["contract"],
                "nepl3.local-doc-pages.display-execution/1"
            );
            let mut bytes = b"nepl3.local-doc-pages.display-execution/1\0".to_vec();
            bytes.extend_from_slice(&base.0);
            bytes.push(if matches!(selected, ParallelMode::Columns) {
                1
            } else {
                2
            });
            if let ParallelMode::Single {
                language,
                fallbacks,
            } = &selected
            {
                bytes.extend_from_slice(&(language.len() as u64).to_le_bytes());
                bytes.extend_from_slice(language.as_bytes());
                bytes.extend_from_slice(&(fallbacks.len() as u64).to_le_bytes());
                for value in fallbacks {
                    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
                    bytes.extend_from_slice(value.as_bytes());
                }
            }
            nepl3_core::source::Digest::of(&bytes)
        };
        let hex = expected
            .0
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        assert_eq!(m["execution_identity"], hex);
        assert!(!executions.contains(&m["execution_identity"]));
        executions.push(m["execution_identity"].clone());
        assert!(!phases.contains(&m["phase_execution"]["identity"]));
        phases.push(m["phase_execution"]["identity"].clone());
        if matches!(selected, ParallelMode::Rows) {
            let old = pages::generate_with_renderer(&c, &inputs, export::MathRenderer::MathMLOnly)?;
            assert_eq!(old.files, result.files);
            let old: serde_json::Value =
                serde_json::from_str(&old.manifest).map_err(|e| e.to_string())?;
            assert_eq!(old["execution_identity"], m["execution_identity"]);
        }
    }
    Ok(())
}
