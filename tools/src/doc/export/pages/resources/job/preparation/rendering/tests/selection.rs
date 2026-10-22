use super::*;
use nepl3_doc_html::{ParallelMode, RenderOptions};
fn selected() -> pages::DisplayOptions {
    pages::DisplayOptions {
        document: RenderOptions {
            parallel: ParallelMode::Single {
                language: "fr".into(),
                fallbacks: vec!["ja".into(), "en".into()],
            },
        },
        math_renderer: MathRenderer::MathMLOnly,
    }
}
fn selected_inputs() -> Vec<(Entry, String)> {
    let mut source = inputs();
    source[0].1=r#"article en Sentence "First" body cons paragraph cons code Math frac 1 2 cons parallel
cons variant en Sentence sentence cons document link page "two" some "english" text "English link" nil
cons variant ja Sentence sentence cons document link page "two" some "japanese" text "Japanese link" nil nil nil nil"#.into();
    source[1].1=r#"article en Sentence "Second" body cons paragraph cons parallel
cons variant en Sentence sentence cons document anchor english text "English target" cons math frac 1 2 nil
cons variant ja Sentence sentence cons document anchor japanese text "Japanese target" cons math frac 3 4 nil nil nil nil"#.into();
    source
}
fn prepare<'a>(c: &'a Compiled, source: Vec<(Entry, String)>) -> Result<PreparedPages<'a>, String> {
    match Preparation::new(c, source, budget().limits(), PreparationLimits::default())?.run() {
        Progress::Prepared(p) => Ok(p),
        _ => Err("preparation failed".into()),
    }
}
#[test]
fn retained_single_freezes_selection_across_repeated_output_stops() -> Result<(), String> {
    let c = compiled()?;
    let RenderProgress::Complete(baseline) = prepare(&c, selected_inputs())?
        .into_render_with_display(OperationLimits::default(), selected())
        .map_err(|e| e.error)?
        .run()
    else {
        return Err("baseline".into());
    };
    let full_work = baseline
        .renders
        .first()
        .ok_or("baseline render")?
        .usage
        .work;
    let late_work = full_work.checked_sub(1).ok_or("baseline work")?;
    let mut caller_options = selected();
    let job = prepare(&c, selected_inputs())?
        .into_render_with_display(
            OperationLimits {
                work: 0,
                ..Default::default()
            },
            caller_options.clone(),
        )
        .map_err(|e| e.error)?;
    caller_options.document.parallel = ParallelMode::Rows; // Caller mutation cannot change the owned request.
    assert!(matches!(
        caller_options.document.parallel,
        ParallelMode::Rows
    ));
    let RenderProgress::Paused { job, .. } = job.run() else {
        return Err("first pause".into());
    };
    assert_eq!(job.reason(), StopReason::WorkLimit);
    assert_eq!(job.preparation().len(), 2);
    let RenderProgress::Paused { job, .. } = job
        .resume(
            budget().limits(),
            OperationLimits {
                work: late_work,
                ..Default::default()
            },
        )
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("second pause".into());
    };
    assert_eq!(job.attempts().len(), 2);
    assert!(job.attempts()[1].usage.work > full_work / 2); // A late retry, not another entry-boundary stop.
    assert_eq!(job.usage(), total(job.preparation(), job.attempts()));
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("completion".into());
    };
    assert_eq!(done.preparation.len(), 2);
    assert_eq!(done.renders.len(), 3);
    assert_eq!(done.usage, total(&done.preparation, &done.renders));
    let direct = pages::generate_with_display_options(
        &c,
        &selected_inputs(),
        &[],
        pages::resources::PhaseLimits::default(),
        &mut budget(),
        selected(),
    )?;
    assert_eq!(done.generated.files, direct.files);
    let first = std::str::from_utf8(&done.generated.files["one.html"]).map_err(err)?;
    assert!(first.contains("Japanese link"));
    assert!(!first.contains("English link"));
    let p = done.generated.provenance.as_ref().ok_or("provenance")?;
    assert_eq!(
        p.sentences[0]
            .iter()
            .map(|r| r.occurrence.0)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(
        p.sentences[0]
            .iter()
            .map(|r| r.placement)
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(p.math_generation[1].len(), 1);
    for (sources, records) in p.sentence_sources.iter().zip(&p.sentences) {
        for record in records {
            assert_eq!(record.owner, sources.sentences[record.occurrence.0].owner);
        }
    }

    let m: serde_json::Value = serde_json::from_str(&done.generated.manifest).map_err(err)?;
    assert_eq!(m["options"]["parallel"], "Single");
    assert_eq!(m["options"]["language"], "fr");
    assert_eq!(m["options"]["fallbacks"], serde_json::json!(["ja", "en"]));
    assert_eq!(
        m["phase_execution"]["contract"],
        "nepl3.local-doc-pages.retained-preparation/1"
    );
    assert_eq!(
        m["job_execution"]["usage"],
        pages::resources::usage(done.usage)
    );
    Ok(())
}
#[test]
fn retained_single_selection_and_hidden_targets_fail_without_pause() -> Result<(), String> {
    let c = compiled()?;
    for case in 0..3 {
        let mut options = selected();
        let mut source = selected_inputs();
        match case {
            0 => {
                options.document.parallel = ParallelMode::Single {
                    language: "missing".into(),
                    fallbacks: vec![],
                }
            }
            1 => {
                options.document.parallel = ParallelMode::Single {
                    language: "fr".into(),
                    fallbacks: vec!["ja".into(), "ja".into()],
                }
            }
            _ => source[0].1 = source[0].1.replace("some \"japanese\"", "some \"english\""),
        }
        let RenderProgress::Failed(failure) = prepare(&c, source)?
            .into_render_with_display(OperationLimits::default(), options)
            .map_err(|e| e.error)?
            .run()
        else {
            return Err("semantic failure must not pause or complete".into());
        };
        assert_eq!(failure.preparation.len(), 2);
        assert_eq!(failure.renders.len(), 1);
        assert_eq!(failure.usage, total(&failure.preparation, &failure.renders));
        if case == 2 {
            assert!(failure.error.contains("MissingAnchor"), "{}", failure.error);
        }
    }
    Ok(())
}
#[test]
fn retained_legacy_single_and_cancellation_keep_owned_selection_and_history() -> Result<(), String>
{
    let c = compiled()?;
    let source = || {
        let mut source = inputs();
        for (_, text) in &mut source {
            *text=r#"article en "Legacy" body cons paragraph cons parallel cons variant en sentence cons text "English" nil cons variant ja sentence cons text "Japanese" nil nil nil nil"#.into();
        }
        source
    };
    let mut identities: Vec<[serde_json::Value; 3]> = Vec::new();
    let mut displayed = None;
    for (language, fallbacks) in [
        ("en", vec![]),
        ("EN", vec![]),
        ("fr", vec!["en"]),
        ("fr", vec!["en", "ja"]),
    ] {
        let mut options = selected();
        options.document.parallel = ParallelMode::Single {
            language: language.into(),
            fallbacks: fallbacks.into_iter().map(str::to_owned).collect(),
        };
        let RenderProgress::Complete(done) = prepare(&c, source())?
            .into_render_with_display(OperationLimits::default(), options.clone())
            .map_err(|e| e.error)?
            .run()
        else {
            return Err("legacy completion".into());
        };
        let direct = pages::generate_with_display_options(
            &c,
            &source(),
            &[],
            pages::resources::PhaseLimits::default(),
            &mut budget(),
            options,
        )?;
        assert_eq!(done.generated.files, direct.files);
        let m: serde_json::Value = serde_json::from_str(&done.generated.manifest).map_err(err)?;
        if let Some(previous) = &displayed {
            assert_eq!(&done.generated.files, previous);
        } else {
            displayed = Some(done.generated.files.clone());
        }
        let ids = [
            m["execution_identity"].clone(),
            m["phase_execution"]["identity"].clone(),
            m["job_execution"]["identity"].clone(),
        ];
        for old in &identities {
            for (left, right) in old.iter().zip(&ids) {
                assert_ne!(left, right);
            }
        }
        identities.push(ids);
    }
    let cancelled = prepare(&c, selected_inputs())?
        .into_render_with_display(OperationLimits::default(), selected())
        .map_err(|e| e.error)?
        .cancel();
    assert_eq!(cancelled.preparation.len(), 2);
    assert!(cancelled.renders.is_empty());
    assert_eq!(
        cancelled.accounting.usage,
        total(&cancelled.preparation, &cancelled.renders)
    );
    Ok(())
}
