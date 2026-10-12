use super::*;
use crate::doc::source::{budget, compiled};
fn inputs() -> Vec<(Entry, String)> {
    ["one", "two"]
        .into_iter()
        .map(|id| {
            (
                Entry {
                    id: id.into(),
                    source: format!("{id}.nepld"),
                    route: format!("{id}.html"),
                    input: None,
                },
                format!("article en \"{id}\" body cons paragraph cons \"Content.\" nil nil"),
            )
        })
        .collect()
}
fn prepared(compiled: &Compiled) -> Result<PreparedPages<'_>, String> {
    match Preparation::new(
        compiled,
        inputs(),
        budget().limits(),
        PreparationLimits::default(),
    )?
    .run()
    {
        Progress::Prepared(value) => Ok(value),
        _ => Err("expected complete preparation".into()),
    }
}
fn total(preparation: &[PageAttempt], renders: &[RenderAttempt]) -> Usage {
    preparation
        .iter()
        .flat_map(|a| &a.stages)
        .map(|a| a.usage)
        .chain(renders.iter().map(|a| a.usage))
        .fold(Usage::default(), super::super::super::add)
}
#[test]
fn output_stop_resumes_without_reparsing_or_partial_publication() -> Result<(), String> {
    let compiled = compiled()?;
    let output = OperationLimits {
        work: 0,
        ..OperationLimits::default()
    };
    let RenderProgress::Paused { job, .. } = prepared(&compiled)?
        .into_render(output, MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("zero output work should pause".into());
    };
    assert_eq!(job.reason(), StopReason::WorkLimit);
    assert_eq!(job.preparation().len(), 2);
    assert_eq!(job.usage(), total(job.preparation(), job.attempts()));
    let first_usage = job.usage();
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "resume rejected")?
        .run()
    else {
        return Err("render should complete".into());
    };
    assert_eq!(done.preparation.len(), 2);
    assert_eq!(done.renders.len(), 2);
    assert!(done.usage.work > first_usage.work);
    assert_eq!(done.usage, total(&done.preparation, &done.renders));
    let expected = pages::generate_with_renderer(&compiled, &inputs(), MathRenderer::MathMLOnly)?;
    assert_eq!(done.generated.files, expected.files);
    let manifest: serde_json::Value =
        serde_json::from_str(&done.generated.manifest).map_err(err)?;
    assert_eq!(
        manifest["job_execution"]["usage"],
        pages::resources::usage(done.usage)
    );
    assert_eq!(
        manifest["job_execution"]["render_attempts"]
            .as_array()
            .ok_or("attempts")?
            .len(),
        2
    );
    assert_eq!(
        manifest["phase_execution"]["contract"],
        "nepl3.local-doc-pages.retained-preparation/1"
    );
    Ok(())
}
#[test]
fn invalid_set_is_terminal_and_cancellation_retains_history() -> Result<(), String> {
    let compiled = compiled()?;
    let mut pages_input = inputs();
    pages_input[1].0.id = "one".into();
    let Progress::Prepared(prepared) = Preparation::new(
        &compiled,
        pages_input,
        budget().limits(),
        PreparationLimits::default(),
    )?
    .run() else {
        return Err("individual preparation".into());
    };
    let RenderProgress::Failed(failure) = prepared
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("duplicate page id must fail globally".into());
    };
    assert_eq!(failure.preparation.len(), 2);
    assert_eq!(failure.usage, total(&failure.preparation, &failure.renders));
    let RenderProgress::Paused { job, .. } = self::prepared(&compiled)?
        .into_render(
            OperationLimits {
                work: 0,
                ..OperationLimits::default()
            },
            MathRenderer::MathMLOnly,
        )
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("pause".into());
    };
    let cancelled = job.cancel();
    assert_eq!(
        cancelled.accounting.previous_stop,
        Some(StopReason::WorkLimit)
    );
    assert_eq!(
        cancelled.accounting.usage,
        total(&cancelled.preparation, &cancelled.renders)
    );
    assert_eq!(cancelled.renders.len(), 1);
    Ok(())
}
#[test]
fn partially_rendered_math_and_code_retry_preserves_owned_evidence() -> Result<(), String> {
    let compiled = compiled()?;
    let create = || -> Result<PreparedPages<'_>, String> {
        let input = vec![(
            Entry {
                id: "math".into(),
                source: "math.nepld".into(),
                route: "math.html".into(),
                input: None,
            },
            r#"article en "Math" body cons display Math frac 1 2 cons code Math frac 1 0 nil"#
                .into(),
        )];
        match Preparation::new(
            &compiled,
            input,
            budget().limits(),
            PreparationLimits::default(),
        )?
        .run()
        {
            Progress::Prepared(value) => Ok(value),
            _ => Err("Math and Code preparation".into()),
        }
    };
    let RenderProgress::Complete(baseline) = create()?
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("baseline render".into());
    };
    let reduced = OperationLimits {
        work: baseline.renders[0].usage.work / 2,
        ..OperationLimits::default()
    };
    let RenderProgress::Paused { job, .. } = create()?
        .into_render(reduced, MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("partial render should exhaust work".into());
    };
    assert!(job.attempts()[0].usage.work > 0);
    assert_eq!(job.reason(), StopReason::WorkLimit);
    assert_eq!(job.preparation().len(), 1);
    let mut smaller = budget().limits();
    smaller.work -= 1;
    let job = job
        .resume(smaller, OperationLimits::default())
        .err()
        .ok_or("shrinking job limit accepted")?
        .into_paused();
    let smaller_output = OperationLimits {
        work: reduced.work - 1,
        ..reduced
    };
    let job = job
        .resume(budget().limits(), smaller_output)
        .err()
        .ok_or("shrinking output limit accepted")?
        .into_paused();
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("retry render".into());
    };
    assert_eq!(done.generated.files, baseline.generated.files);
    assert_eq!(done.preparation.len(), 1);
    assert_eq!(done.renders.len(), 2);
    assert_eq!(done.usage, total(&done.preparation, &done.renders));
    let html = String::from_utf8(done.generated.files.get("math.html").ok_or("HTML")?.clone())
        .map_err(err)?;
    assert!(html.contains("<math"));
    assert!(html.contains("nepl-code"));
    let baseline_manifest: serde_json::Value =
        serde_json::from_str(&baseline.generated.manifest).map_err(err)?;
    let manifest: serde_json::Value =
        serde_json::from_str(&done.generated.manifest).map_err(err)?;
    assert_ne!(
        manifest["job_execution"]["identity"],
        baseline_manifest["job_execution"]["identity"]
    );
    let mut receipt = manifest["job_execution"].clone();
    let expected = receipt
        .as_object_mut()
        .ok_or("receipt")?
        .remove("identity")
        .ok_or("identity")?;
    let mut encoded = Vec::from(b"nepl3.local-doc-pages.retry-job/1\0".as_slice());
    encoded.extend_from_slice(&serde_json::to_vec(&manifest["execution_identity"]).map_err(err)?);
    encoded.extend_from_slice(&serde_json::to_vec(&manifest["phase_execution"]).map_err(err)?);
    encoded.extend_from_slice(&serde_json::to_vec(&receipt).map_err(err)?);
    assert_eq!(expected, pages::digest_hex(Digest::of(&encoded)));
    receipt["usage"]["work"] = 0.into();
    let mut altered = Vec::from(b"nepl3.local-doc-pages.retry-job/1\0".as_slice());
    altered.extend_from_slice(&serde_json::to_vec(&manifest["execution_identity"]).map_err(err)?);
    altered.extend_from_slice(&serde_json::to_vec(&manifest["phase_execution"]).map_err(err)?);
    altered.extend_from_slice(&serde_json::to_vec(&receipt).map_err(err)?);
    assert_ne!(expected, pages::digest_hex(Digest::of(&altered)));
    Ok(())
}
#[test]
fn cumulative_render_exhaustion_cannot_reset_consumed_work() -> Result<(), String> {
    let compiled = compiled()?;
    let RenderProgress::Complete(baseline) = prepared(&compiled)?
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("baseline".into());
    };
    let mut limits = budget().limits();
    limits.work = baseline.usage.work - baseline.renders[0].usage.work / 2;
    let Progress::Prepared(ready) =
        Preparation::new(&compiled, inputs(), limits, PreparationLimits::default())?.run()
    else {
        return Err("tight preparation".into());
    };
    let RenderProgress::Paused { job, .. } = ready
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .map_err(|failure| failure.error)?
        .run()
    else {
        return Err("cumulative render should pause".into());
    };
    assert_eq!(job.reason(), StopReason::WorkLimit);
    let prior = job.usage();
    let RenderProgress::Paused { job, .. } = job
        .resume(limits, OperationLimits::default())
        .map_err(|_| "same cap")?
        .run()
    else {
        return Err("same cumulative cap must still stop".into());
    };
    assert!(job.usage().work >= prior.work);
    assert!(job.usage().work <= limits.work);
    assert_eq!(job.preparation().len(), 2);
    assert_eq!(job.usage(), total(job.preparation(), job.attempts()));
    let RenderProgress::Complete(done) = job
        .resume(budget().limits(), OperationLimits::default())
        .map_err(|_| "larger cap")?
        .run()
    else {
        return Err("larger cumulative cap".into());
    };
    assert_eq!(done.preparation.len(), 2);
    assert_eq!(done.renders.len(), 3);
    assert_eq!(done.usage, total(&done.preparation, &done.renders));
    Ok(())
}
#[test]
fn conversion_failure_retains_preparation_evidence() -> Result<(), String> {
    let compiled = compiled()?;
    let mut ready = prepared(&compiled)?;
    let usage = ready.usage();
    // Private invariant fault injection exercises a fallible setup boundary.
    ready.data.inputs[0].0.input = Some("../invalid".into());
    let failure = ready
        .into_render(OperationLimits::default(), MathRenderer::MathMLOnly)
        .err()
        .ok_or("conversion must reject invalid physical input")?;
    assert_eq!(failure.usage, usage);
    assert_eq!(failure.preparation.len(), 2);
    assert_eq!(failure.usage, total(&failure.preparation, &failure.renders));
    Ok(())
}
