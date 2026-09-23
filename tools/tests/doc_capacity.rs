//! Desktop Doc host capacity against an authored sequence of short sentences.
use nepl3_tools::doc::{export, source::compiled};

#[test]
fn observing_export_preserves_artifacts_and_stage_order() -> Result<(), String> {
    let compiled = compiled()?;
    let source = "article ja \"T\" body cons paragraph cons \"[文/ぶん]。\" nil nil";
    let expected = export::generate(&compiled, source)?;
    let mut measurements = Vec::new();
    let actual = export::generate_observed(&compiled, source, &mut |m| measurements.push(m))?;
    assert_eq!(actual.html, expected.html);
    assert_eq!(actual.manifest, expected.manifest);
    assert_eq!(
        measurements.iter().map(|m| m.stage).collect::<Vec<_>>(),
        [
            export::Stage::ParseAndValidate,
            export::Stage::Lower,
            export::Stage::Prepare,
            export::Stage::RenderAndSerialize,
        ]
    );
    assert!(measurements[3].usage.work >= measurements[2].usage.work);
    Ok(())
}

/// Explicit host measurement: no wall-clock threshold and no production Budget
/// increase. Source construction is parser test input, outside measured stages.
#[test]
#[ignore = "explicit pipeline scaling measurement"]
fn measure_annotated_document_scaling() -> Result<(), String> {
    let compiled = compiled()?;
    for (paragraphs, sentences, annotated, text_repeats) in [
        (8, 4, true, 1),
        (32, 4, true, 1),
        (128, 4, true, 1),
        (32, 1, true, 1),
        (32, 4, false, 1),
        (32, 4, true, 8),
    ] {
        let mut source = String::from("article ja \"T\" body ");
        let text = if annotated {
            "[文/ぶん]。"
        } else {
            "文。"
        }
        .repeat(text_repeats);
        for _ in 0..paragraphs {
            source.push_str("cons paragraph ");
            for _ in 0..sentences {
                source.push_str("cons \"");
                source.push_str(&text);
                source.push_str("\" ");
            }
            source.push_str("nil ");
        }
        source.push_str("nil");
        println!(
            "input bytes={} paragraphs={paragraphs} sentences={sentences} annotated={annotated} text_repeats={text_repeats}",
            source.len()
        );
        let output = export::generate_observed(&compiled, &source, &mut |m| {
            println!(
                "stage={:?} elapsed_ns={} usage={:?}",
                m.stage,
                m.elapsed.as_nanos(),
                m.usage
            );
        })?;
        let count = paragraphs * sentences * text_repeats;
        assert_eq!(output.html.matches('。').count(), count);
        assert_eq!(
            output.html.matches("ぶん").count(),
            if annotated { count } else { 0 }
        );
    }
    Ok(())
}

#[test]
fn large_annotated_document_preserves_every_sentence() -> Result<(), String> {
    let mut source = String::from("article ja \"T\" body ");
    for _ in 0..128 {
        source.push_str("cons paragraph ");
        for _ in 0..4 {
            source.push_str("cons \"[文/ぶん]。\" ");
        }
        source.push_str("nil ");
    }
    source.push_str("nil");
    let output = export::generate(&compiled()?, &source)?;
    assert_eq!(output.html.matches("class=\"nepl-ruby\"").count(), 512);
    assert_eq!(output.html.matches("ぶん").count(), 512);
    assert_eq!(output.html.matches('。').count(), 512);
    assert!(!output.html.contains("<script"));
    let manifest: serde_json::Value =
        serde_json::from_str(&output.manifest).map_err(|e| e.to_string())?;
    // Record usage for observation. This capacity test checks content under the
    // bounded host pipeline; reducing validation visits must remain successful.
    // Budget rejection and unpublished output are checked independently by
    // canonical::tests::html_output_allowance_is_explicit_independent_and_recorded.
    println!("{}", manifest["operations"]);
    Ok(())
}
