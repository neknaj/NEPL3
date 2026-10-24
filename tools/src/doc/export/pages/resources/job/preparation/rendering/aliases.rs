use super::*;
use crate::doc::source::{budget, compiled};
fn inputs() -> Vec<(Entry, String)> {
    vec![(Entry{id:"one".into(),source:"one.nepld".into(),route:"one.html".into(),input:None},r#"article en "One" body cons section s "Section" body cons paragraph cons "Text." nil nil nil"#.into())]
}
fn prepare<'a>(
    compiled: &'a Compiled,
    raw: Vec<(String, Vec<u8>)>,
) -> Result<PreparedPages<'a>, String> {
    match Preparation::with_aliases(
        compiled,
        inputs(),
        Vec::new(),
        raw,
        budget().limits(),
        PreparationLimits::default(),
    )?
    .run()
    {
        Progress::Prepared(p) => Ok(p),
        _ => Err("prepare".into()),
    }
}
fn aliases(raw: &[u8]) -> Vec<(String, Vec<u8>)> {
    vec![("one".into(), raw.to_vec())]
}
fn render(compiled: &Compiled, raw: Vec<(String, Vec<u8>)>) -> Result<RenderedJob, String> {
    match prepare(compiled, raw)?
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|e| e.error)?
        .run()
    {
        RenderProgress::Complete(done) => Ok(done),
        _ => Err("render".into()),
    }
}
#[test]
fn retained_alias_retry_matches_ordinary_and_preserves_order() -> Result<(), String> {
    let compiled = compiled()?;
    let raw = br#"[{"section":null,"name":"old-root"},{"section":"s","name":"old-section"}]"#;
    let expected = pages::generate_with_aliases(
        &compiled,
        &inputs(),
        &[],
        pages::resources::PhaseLimits::default(),
        &mut budget(),
        &[pages::aliases::PageAliases::parse(
            "one".into(),
            raw.to_vec(),
        )?],
    )?;
    let RenderProgress::Paused { job, .. } = prepare(&compiled, aliases(raw))?
        .into_render(
            OperationLimits {
                output_bytes: 1,
                ..Default::default()
            },
            MathRenderer::MathMLOnly,
        )
        .map_err(|e| e.error)?
        .run()
    else {
        return Err("pause".into());
    };
    assert_eq!(job.preparation().len(), 1);
    let used = job.usage();
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("complete".into());
    };
    assert_eq!(done.generated.files, expected.files);
    assert_eq!(done.preparation.len(), 1);
    assert_eq!(done.renders.len(), 2);
    assert!(done.usage.work > used.work);
    let html = std::str::from_utf8(&done.generated.files["one.html"]).map_err(err)?;
    assert_eq!(html.matches("id=\"old-root\"").count(), 1);
    assert_eq!(html.matches("id=\"old-section\"").count(), 1);
    Ok(())
}
#[test]
fn retained_alias_raw_bytes_bind_job_identity_not_semantic_identity() -> Result<(), String> {
    let compiled = compiled()?;
    let raw = br#"[{"section":null,"name":"old"}]"#;
    let changed = br#"[ {"section":null,"name":"old"} ]"#;
    let first = render(&compiled, aliases(raw))?;
    let second = render(&compiled, aliases(changed))?;
    assert_eq!(first.generated.files, second.generated.files);
    let a: serde_json::Value = serde_json::from_str(&first.generated.manifest).map_err(err)?;
    let b: serde_json::Value = serde_json::from_str(&second.generated.manifest).map_err(err)?;
    assert_eq!(a["identity"], b["identity"]);
    assert_ne!(
        a["job_execution"]["identity"],
        b["job_execution"]["identity"]
    );
    assert_ne!(
        a["compatibility"]["input_identity"],
        b["compatibility"]["input_identity"]
    );
    assert_eq!(
        a["job_execution"]["contract"],
        "nepl3.local-doc-pages.retry-alias-job/1"
    );
    let empty = render(&compiled, Vec::new())?;
    let m: serde_json::Value = serde_json::from_str(&empty.generated.manifest).map_err(err)?;
    assert_eq!(
        m["job_execution"]["contract"],
        "nepl3.local-doc-pages.retry-job/1"
    );
    let mut receipt = m["job_execution"].clone();
    receipt.as_object_mut().ok_or("receipt")?.remove("identity");
    let mut bytes = b"nepl3.local-doc-pages.retry-job/1\0".to_vec();
    bytes.extend_from_slice(&serde_json::to_vec(&m["execution_identity"]).map_err(err)?);
    bytes.extend_from_slice(&serde_json::to_vec(&m["phase_execution"]).map_err(err)?);
    bytes.extend_from_slice(&serde_json::to_vec(&receipt).map_err(err)?);
    assert_eq!(
        m["job_execution"]["identity"],
        pages::digest_hex(Digest::of(&bytes))
    );
    Ok(())
}
#[test]
fn retained_alias_invalid_bindings_and_collisions_fail() -> Result<(), String> {
    let compiled = compiled()?;
    for raw in [
        br#"[{"section":null,"name":"n-73"}]"#.as_slice(),
        br#"[{"section":"missing","name":"old"}]"#,
        br#"[{"section":null,"name":"same"},{"section":null,"name":"same"}]"#,
    ] {
        assert!(matches!(
            prepare(&compiled, aliases(raw))?
                .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
                .map_err(|e| e.error)?
                .run(),
            RenderProgress::Failed(_)
        ));
    }
    for raw in [
        vec![("missing".into(), b"[]".to_vec())],
        vec![
            ("one".into(), b"[]".to_vec()),
            ("one".into(), b"[]".to_vec()),
        ],
    ] {
        assert!(matches!(
            prepare(&compiled, raw)?
                .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
                .map_err(|e| e.error)?
                .run(),
            RenderProgress::Failed(_)
        ));
    }
    Ok(())
}

#[test]
fn retained_alias_binding_stop_resume_and_cancel_keep_accounting() -> Result<(), String> {
    let compiled = compiled()?;
    let raw = br#"[{"section":null,"name":"old"}]"#;
    let pause = || -> Result<PausedRender<'_>, String> {
        match prepare(&compiled, aliases(raw))?
            .into_render(
                OperationLimits {
                    work: 0,
                    ..Default::default()
                },
                MathRenderer::MathMLOnly,
            )
            .map_err(|e| e.error)?
            .run()
        {
            RenderProgress::Paused { job, .. } => Ok(job),
            _ => Err("binding pause".into()),
        }
    };
    let job = pause()?;
    assert_eq!(job.reason(), StopReason::WorkLimit);
    let before = job.usage();
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("complete".into());
    };
    assert_eq!(done.preparation.len(), 1);
    assert_eq!(done.renders.len(), 2);
    assert!(done.usage.work > before.work);
    let total = done
        .preparation
        .iter()
        .flat_map(|a| &a.stages)
        .map(|s| s.usage)
        .chain(done.renders.iter().map(|a| a.usage))
        .fold(Usage::default(), super::super::super::add);
    assert_eq!(done.usage, total);
    let cancelled = pause()?.cancel();
    assert_eq!(cancelled.renders.len(), 1);
    assert_eq!(
        cancelled.accounting.previous_stop,
        Some(StopReason::WorkLimit)
    );
    Ok(())
}
#[test]
fn retained_alias_setup_rejects_bounded_invalid_inputs() -> Result<(), String> {
    let compiled = compiled()?;
    let make = |raw| {
        Preparation::with_aliases(
            &compiled,
            inputs(),
            Vec::new(),
            raw,
            budget().limits(),
            PreparationLimits::default(),
        )
        .err()
        .ok_or("reject")
    };
    assert_eq!(
        make((0..129).map(|i| (i.to_string(), b"[]".to_vec())).collect())?,
        "AliasPageLimit"
    );
    assert_eq!(
        make(vec![("x".repeat(4097), b"[]".to_vec())])?,
        "AliasInputLimit"
    );
    assert_eq!(
        make(vec![("one".into(), vec![b' '; 1_048_577])])?,
        "AliasInputLimit"
    );
    assert!(make(vec![("one".into(), b"not json".to_vec())]).is_ok());
    let raw = serde_json::to_vec(&vec![serde_json::json!({"section":null,"name":"a"}); 4097])
        .map_err(err)?;
    assert_eq!(make(vec![("one".into(), raw)])?, "AliasCountLimit");
    Ok(())
}

#[test]
fn retained_alias_identity_formula_empty_input_and_order_are_explicit() -> Result<(), String> {
    let compiled = compiled()?;
    let raw = br#"[{"section":null,"name":"first"},{"section":null,"name":"second"}]"#;
    let done = render(&compiled, aliases(raw))?;
    let html = std::str::from_utf8(&done.generated.files["one.html"]).map_err(err)?;
    assert!(
        html.find("id=\"first\"").ok_or("first")? < html.find("id=\"second\"").ok_or("second")?
    );
    let a: serde_json::Value = serde_json::from_str(&done.generated.manifest).map_err(err)?;
    let mut receipt = a["job_execution"].clone();
    receipt.as_object_mut().ok_or("receipt")?.remove("identity");
    let mut bytes = b"nepl3.local-doc-pages.retry-alias-job/1\0".to_vec();
    bytes.extend_from_slice(&serde_json::to_vec(&a["execution_identity"]).map_err(err)?);
    bytes.extend_from_slice(&serde_json::to_vec(&a["phase_execution"]).map_err(err)?);
    bytes.extend_from_slice(
        &serde_json::to_vec(
            a["compatibility"]["execution_identity"]
                .as_str()
                .ok_or("compatibility")?,
        )
        .map_err(err)?,
    );
    bytes.extend_from_slice(&serde_json::to_vec(&receipt).map_err(err)?);
    assert_eq!(
        a["job_execution"]["identity"],
        pages::digest_hex(Digest::of(&bytes))
    );
    let no_input = render(&compiled, Vec::new())?;
    let empty_page = render(&compiled, aliases(b"[]"))?;
    assert_eq!(no_input.generated.files, empty_page.generated.files);
    let n: serde_json::Value = serde_json::from_str(&no_input.generated.manifest).map_err(err)?;
    let e: serde_json::Value = serde_json::from_str(&empty_page.generated.manifest).map_err(err)?;
    assert_eq!(
        n["job_execution"]["contract"],
        "nepl3.local-doc-pages.retry-job/1"
    );
    assert_eq!(
        e["job_execution"]["contract"],
        "nepl3.local-doc-pages.retry-alias-job/1"
    );
    assert_ne!(
        n["job_execution"]["identity"],
        e["job_execution"]["identity"]
    );
    Ok(())
}
#[test]
fn retained_alias_aggregate_exact_boundary_is_bounded() -> Result<(), String> {
    let compiled = compiled()?;
    let make = |extra: bool| {
        let mut values: Vec<_> = (0..10)
            .map(|_| {
                let mut raw = b"[]".to_vec();
                raw.resize(1_000_000, b' ');
                ("one".to_string(), raw)
            })
            .collect();
        if extra {
            values.push(("one".to_string(), vec![b' ']));
        }
        Preparation::with_aliases(
            &compiled,
            inputs(),
            Vec::new(),
            values,
            budget().limits(),
            PreparationLimits::default(),
        )
    };
    make(false)?;
    assert_eq!(
        make(true).err().ok_or("aggregate limit")?,
        "AliasInputLimit"
    );
    Ok(())
}

#[test]
fn retained_alias_mixed_sentence_pages_keep_files_and_cross_page_links() -> Result<(), String> {
    let compiled = compiled()?;
    let input = || {
        vec![
        (Entry{id:"one".into(),source:"one.nepld".into(),route:"first/one.html".into(),input:None},r#"article en Sentence "Title" body cons paragraph cons sentence cons link relative "two.nepld" some "s" text "Two" cons text " " cons link relative "ref.txt" none text "File" nil nil nil"#.into()),
        (Entry{id:"two".into(),source:"two.nepld".into(),route:"second/two.html".into(),input:None},r#"article en "Two" body cons section s "Section" body nil nil"#.into())]
    };
    let files = || {
        vec![(
            Entry {
                id: "ref".into(),
                source: "ref.txt".into(),
                route: "resources/ref.txt".into(),
                input: None,
            },
            b"reference bytes".to_vec(),
        )]
    };
    let raw_one = br#"[{"section":null,"name":"old-one"}]"#;
    let raw_two = br#"[{"section":"s","name":"old-section"}]"#;
    let expected = pages::generate_with_aliases(
        &compiled,
        &input(),
        &files(),
        pages::resources::PhaseLimits::default(),
        &mut budget(),
        &[
            pages::aliases::PageAliases::parse("one".into(), raw_one.to_vec())?,
            pages::aliases::PageAliases::parse("two".into(), raw_two.to_vec())?,
        ],
    )?;
    let Progress::Prepared(ready) = Preparation::with_aliases(
        &compiled,
        input(),
        files(),
        vec![
            ("one".into(), raw_one.to_vec()),
            ("two".into(), raw_two.to_vec()),
        ],
        budget().limits(),
        PreparationLimits::default(),
    )?
    .run() else {
        return Err("prepare".into());
    };
    let RenderProgress::Complete(done) = ready
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|e| e.error)?
        .run()
    else {
        return Err("render".into());
    };
    assert_eq!(done.generated.files, expected.files);
    assert_eq!(
        done.generated.files["resources/ref.txt"],
        b"reference bytes"
    );
    let html = std::str::from_utf8(&done.generated.files["first/one.html"]).map_err(err)?;
    assert!(html.contains("../second/two.html#n-73"));
    assert!(html.contains("../resources/ref.txt"));
    assert_eq!(html.matches("id=\"old-one\"").count(), 1);
    Ok(())
}
