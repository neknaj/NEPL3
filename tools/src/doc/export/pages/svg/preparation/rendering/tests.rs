use super::*;
use crate::{
    doc::{
        export::pages::svg::{
            generation, input,
            preparation::{Preparation, Progress},
        },
        source::{Compiled, budget, compiled},
    },
    testing::Fixture,
};
use nepl3_core::source::Digest;
use serde_json::{Value, json};
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const ONE: &str = r#"article en Sentence "One" body cons image asset "logo" none Sentence "[漢/かん]" none cons paragraph cons Sentence sentence cons math frac 1 2 cons document math Math frac 3 4 cons document link page "two" some "target" text "Next" nil cons code Math frac 5 6 nil cons display Math frac 7 8 nil"#;
const TWO: &str = r#"article en "Two" body cons image asset "logo" none "Two image" none cons section target "Target" body cons paragraph cons sentence cons math Math frac 9 10 nil nil nil nil"#;
fn manifest(output: OperationLimits) -> Value {
    json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"deep/a.html"},{"id":"two","source":"two.nepld","route":"deep/b.html"}],"files":[{"id":"manual","source":"manual.txt","route":"manual.txt"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"one.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"two.svg","mime":"image/svg+xml"}]}],"output_limits":output})
}
fn fixture(output: OperationLimits) -> crate::Result<Fixture> {
    let f = Fixture::new()?;
    f.write("one.nepld", ONE)?;
    f.write("two.nepld", TWO)?;
    f.write("one.svg", SVG)?;
    f.write("two.svg", SVG)?;
    f.write("manual.txt", "frozen passive bytes")?;
    f.json("manifest.json", &manifest(output))?;
    Ok(f)
}
fn prepared<'c>(
    c: &'c Compiled,
    f: &Fixture,
    renderer: MathRenderer,
) -> Result<Prepared<'c>, String> {
    match Preparation::capture(
        c,
        &f.root().join("manifest.json"),
        renderer,
        budget().limits(),
        OperationLimits::default(),
    )?
    .run()
    {
        Progress::Prepared(value) => Ok(value),
        Progress::Paused { job, error } => {
            Err(format!("preparation {:?}: {error:?}", job.reason()))
        }
        Progress::Failed { error, .. } => Err(error),
        Progress::Cancelled(_) => Err("cancelled preparation".into()),
    }
}
fn complete(job: RenderJob<'_>) -> Result<Rendered, String> {
    match job.run() {
        RenderProgress::Complete(value) => Ok(value),
        RenderProgress::Paused { job, error } => {
            Err(format!("render {:?}: {error:?}", job.reason()))
        }
        RenderProgress::Failed(value) => Err(value.error),
        RenderProgress::Cancelled(_) => Err("cancelled render".into()),
    }
}
fn add(a: Usage, b: Usage) -> Usage {
    Usage {
        source_bytes: a.source_bytes + b.source_bytes,
        work: a.work + b.work,
        depth: a.depth.max(b.depth),
        nodes: a.nodes + b.nodes,
        allocation_units: a.allocation_units + b.allocation_units,
        output_bytes: a.output_bytes + b.output_bytes,
        diagnostics: a.diagnostics + b.diagnostics,
        events: a.events + b.events,
    }
}
fn total(prep: &[PageAttempt], renders: &[RenderAttempt]) -> Usage {
    let initial = prep
        .iter()
        .flat_map(|a| &a.stages)
        .fold(Usage::default(), |sum, s| add(sum, s.usage));
    renders.iter().fold(initial, |sum, a| add(sum, a.usage))
}
fn identity(receipt: &Value) -> Result<String, String> {
    let payload = serde_json::to_vec(&(&receipt["package"], &receipt["job_execution"]))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::from(b"nepl3.local-doc-pages.svg-render-job/1\0".as_slice());
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&payload);
    Ok(crate::doc::export::digest_hex(Digest::of(&bytes)))
}
fn check_output(value: &Rendered) -> crate::Result<Value> {
    assert_eq!(value.usage, total(&value.preparation, &value.renders));
    let receipt: Value = serde_json::from_str(&value.manifest)?;
    assert_eq!(receipt["format"], "nepl3.local-doc-pages.svg-render-job/1");
    assert_eq!(receipt["identity"], identity(&receipt)?);
    assert_eq!(
        receipt["job_execution"]["usage"],
        resources::usage(value.usage)
    );
    assert_eq!(
        receipt["job_execution"]["limits"],
        resources::limits(value.limits)
    );
    let rows = receipt["job_execution"]["render_attempts"]
        .as_array()
        .ok_or("render rows")?;
    assert_eq!(rows.len(), value.renders.len());
    for (row, actual) in rows.iter().zip(&value.renders) {
        assert_eq!(row["requested"], resources::limits(actual.requested));
        assert_eq!(row["effective"], resources::limits(actual.effective));
        assert_eq!(row["usage"], resources::usage(actual.usage));
        assert_eq!(row["stop"], json!(actual.stop.map(|v| format!("{v:?}"))));
        assert_eq!(row["failed"], actual.failed);
    }
    let prep = &receipt["job_execution"]["preparation"];
    assert_eq!(
        prep["cumulative_usage"],
        resources::usage(total(&value.preparation, &[]))
    );
    let rows = prep["attempts"].as_array().ok_or("preparation rows")?;
    assert_eq!(rows.len(), value.preparation.len());
    for (row, actual) in rows.iter().zip(&value.preparation) {
        assert_eq!(row["page_index"], actual.page_index);
        let stages = row["stages"].as_array().ok_or("stages")?;
        assert_eq!(stages.len(), actual.stages.len());
        for (stage, actual) in stages.iter().zip(&actual.stages) {
            assert_eq!(stage["stage"], format!("{:?}", actual.stage));
            assert_eq!(stage["requested"], resources::limits(actual.requested));
            assert_eq!(stage["effective"], resources::limits(actual.effective));
            assert_eq!(stage["usage"], resources::usage(actual.usage));
            assert_eq!(stage["stop"], json!(actual.stop.map(|v| format!("{v:?}"))));
            assert_eq!(stage["failed"], actual.failed);
            assert_eq!(stage["accounting_rejected"], actual.accounting_rejected);
            assert_eq!(
                stage["profile"],
                json!(actual.profile.map(crate::doc::export::digest_hex))
            );
        }
    }
    assert_eq!(
        receipt["job_execution"]["successful_render_attempt"],
        value.renders.len() - 1
    );
    assert!(
        receipt["job_execution"]["native_evidence_scope"]
            .as_str()
            .ok_or("native scope")?
            .contains("not retained")
    );
    for file in receipt["package"]["files"].as_array().ok_or("files")? {
        let path = file["path"].as_str().ok_or("path")?;
        let bytes = value.files.get(path).ok_or("payload")?;
        assert_eq!(file["bytes"], bytes.len());
        assert_eq!(
            file["sha256"],
            crate::doc::export::digest_hex(Digest::of(bytes))
        );
    }
    assert!(value.files.values().any(|b| b == b"frozen passive bytes"));
    let first = String::from_utf8(value.files.get("deep/a.html").ok_or("one")?.clone())?;
    assert!(first.contains("<math"));
    assert!(first.contains("<code"));
    assert!(first.contains("Next"));
    assert!(
        receipt["package"]["source_input"]["scope"]
            .as_str()
            .ok_or("scope")?
            .contains("not source preparation")
    );
    Ok(receipt)
}
#[test]
fn retained_svg_render_success_preserves_legacy_payload_and_job_identity() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    let captured = input::capture(&f.root().join("manifest.json"))?;
    let old = generation::run_selected(&c, &captured, MathRenderer::MathMLOnly, &mut budget())?;
    let ready = prepared(&c, &f, MathRenderer::MathMLOnly)?;
    let result = complete(ready.into_render().map_err(|e| format!("{e:?}"))?)?;
    assert_eq!(result.files, old.files);
    let receipt = check_output(&result)?;
    let old_receipt: Value = serde_json::from_str(&old.manifest)?;
    assert_eq!(
        receipt["package"]["input_identity"],
        old_receipt["input_identity"]
    );
    assert_eq!(receipt["package"]["katex_generation"], false);
    assert_eq!(result.renders.len(), 1);
    let mut altered = receipt.clone();
    altered["job_execution"]["usage"]["work"] = json!(0);
    assert_ne!(identity(&altered)?, identity(&receipt)?);
    altered = receipt.clone();
    altered["package"]["source_input"]["scope"] = json!("different");
    assert_ne!(identity(&altered)?, identity(&receipt)?);
    Ok(())
}
#[test]
fn retained_svg_render_repeats_output_only_and_preserves_failed_consumption() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    let baseline = complete(
        prepared(&c, &f, MathRenderer::MathMLOnly)?
            .into_render()
            .map_err(|e| format!("{e:?}"))?,
    )?;
    let low = OperationLimits {
        work: baseline.renders[0].usage.work / 2,
        ..Default::default()
    };
    assert!(low.work > 0);
    f.json("manifest.json", &manifest(low))?;
    let job = prepared(&c, &f, MathRenderer::MathMLOnly)?
        .into_render()
        .map_err(|e| format!("{e:?}"))?;
    let preparation = format!("{:?}", job.data.owned.preparation);
    let code = job.data.owned.code.as_ptr();
    let pages = job.data.owned.request.set.pages.as_ptr();
    for name in [
        "manifest.json",
        "one.nepld",
        "two.nepld",
        "one.svg",
        "two.svg",
        "manual.txt",
    ] {
        std::fs::remove_file(f.root().join(name))?;
    }
    let RenderProgress::Paused { job, .. } = job.run() else {
        return Err("first render pause".into());
    };
    assert_eq!(job.reason(), StopReason::WorkLimit);
    assert!(job.attempts()[0].usage.work > 0);
    assert_eq!(job.usage(), total(job.preparation(), job.attempts()));
    assert_eq!(job.data.owned.code.as_ptr(), code);
    assert_eq!(job.data.owned.request.set.pages.as_ptr(), pages);
    let first = job.usage();
    let RenderProgress::Paused { job, .. } = job
        .resume(budget().limits(), low)
        .map_err(|_| "same phase")?
        .run()
    else {
        return Err("second render pause".into());
    };
    assert!(job.usage().work > first.work);
    assert_eq!(job.attempts().len(), 2);
    assert_eq!(format!("{:?}", job.preparation()), preparation);
    assert_eq!(job.data.owned.code.as_ptr(), code);
    assert_eq!(job.data.owned.request.set.pages.as_ptr(), pages);
    let resumed = complete(
        job.resume(budget().limits(), OperationLimits::default())
            .map_err(|_| "larger grant")?,
    )?;
    assert_eq!(resumed.files, baseline.files);
    assert_eq!(resumed.renders.len(), 3);
    assert_eq!(format!("{:?}", resumed.preparation), preparation);
    assert!(resumed.usage.work > baseline.usage.work);
    let receipt = check_output(&resumed)?;
    let initial = check_output(&baseline)?;
    assert_ne!(receipt["identity"], initial["identity"]);
    Ok(())
}
#[test]
fn retained_svg_render_semantic_failure_and_cancellation_are_terminal() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    f.write("one.svg", "<invalid unadmitted SVG>")?;
    let ready = prepared(&c, &f, MathRenderer::MathMLOnly)?;
    let RenderProgress::Failed(failed) = ready.into_render().map_err(|e| format!("{e:?}"))?.run()
    else {
        return Err("SVG admission must fail".into());
    };
    assert_eq!(failed.renders.len(), 1);
    assert!(failed.renders[0].stop.is_none());
    assert!(failed.renders[0].failed);
    assert_eq!(failed.usage, total(&failed.preparation, &failed.renders));
    f.write("one.svg", SVG)?;
    let job = prepared(&c, &f, MathRenderer::MathMLOnly)?
        .into_render()
        .map_err(|e| format!("{e:?}"))?;
    let cancelled = job.cancel();
    assert!(cancelled.renders.is_empty());
    assert!(!cancelled.preparation.is_empty());
    let low = OperationLimits {
        work: 0,
        ..Default::default()
    };
    f.json("manifest.json", &manifest(low))?;
    let RenderProgress::Paused { job, .. } = prepared(&c, &f, MathRenderer::MathMLOnly)?
        .into_render()
        .map_err(|e| format!("{e:?}"))?
        .run()
    else {
        return Err("pause to cancel".into());
    };
    let usage = job.usage();
    let cancelled = job.cancel();
    assert_eq!(cancelled.accounting.usage, usage);
    assert_eq!(
        cancelled.accounting.previous_stop,
        Some(StopReason::WorkLimit)
    );
    Ok(())
}
fn set_cap(v: &mut OperationLimits, field: usize, value: u64) -> Result<(), String> {
    match field {
        0 => v.source_bytes = value,
        1 => v.work = value,
        2 => v.depth = value,
        3 => v.nodes = value,
        4 => v.allocation_units = value,
        5 => v.output_bytes = value,
        6 => v.diagnostics = value,
        7 => v.events = value,
        _ => return Err("resource field".into()),
    };
    Ok(())
}
fn amount(u: Usage, field: usize) -> Result<u64, String> {
    match field {
        0 => Ok(u.source_bytes),
        1 => Ok(u.work),
        2 => Ok(u.depth),
        3 => Ok(u.nodes),
        4 => Ok(u.allocation_units),
        5 => Ok(u.output_bytes),
        6 => Ok(u.diagnostics),
        7 => Ok(u.events),
        _ => Err("resource field".into()),
    }
}
fn as_operation(v: Limits) -> OperationLimits {
    OperationLimits {
        source_bytes: v.source_bytes,
        work: v.work,
        depth: v.depth,
        nodes: v.nodes,
        allocation_units: v.allocation_units,
        output_bytes: v.output_bytes,
        diagnostics: v.diagnostics,
        events: v.events,
    }
}
#[test]
fn retained_svg_render_rejects_all_shrinking_grants_without_replacing_authority()
-> crate::Result<()> {
    let c = compiled()?;
    let low = OperationLimits {
        work: 1,
        ..Default::default()
    };
    let f = fixture(low)?;
    let RenderProgress::Paused { mut job, .. } = prepared(&c, &f, MathRenderer::MathMLOnly)?
        .into_render()
        .map_err(|e| format!("{e:?}"))?
        .run()
    else {
        return Err("render pause".into());
    };
    let usage = job.usage();
    let preparation = format!("{:?}", job.preparation());
    let attempts = format!("{:?}", job.attempts());
    let code = job.data.owned.code.as_ptr();
    let pages = job.data.owned.request.set.pages.as_ptr();
    for group in 0..2 {
        for field in 0..8 {
            let mut total = as_operation(job.limits());
            let mut output = low;
            let selected = if group == 0 { &mut total } else { &mut output };
            let current = match field {
                0 => selected.source_bytes,
                1 => selected.work,
                2 => selected.depth,
                3 => selected.nodes,
                4 => selected.allocation_units,
                5 => selected.output_bytes,
                6 => selected.diagnostics,
                _ => selected.events,
            };
            set_cap(selected, field, current - 1)?;
            job = job
                .resume(total.budget().limits(), output)
                .err()
                .ok_or("shrinking grant accepted")?
                .into_paused();
            assert_eq!(job.usage(), usage);
            assert_eq!(format!("{:?}", job.preparation()), preparation);
            assert_eq!(format!("{:?}", job.attempts()), attempts);
            assert_eq!(job.data.owned.code.as_ptr(), code);
            assert_eq!(job.data.owned.request.set.pages.as_ptr(), pages);
            assert_eq!(job.reason(), StopReason::WorkLimit);
        }
    }
    let ready = complete(
        job.resume(budget().limits(), OperationLimits::default())
            .map_err(|_| "valid grant")?,
    )?;
    let _ = check_output(&ready)?;
    Ok(())
}
#[test]
fn retained_svg_render_measures_exact_and_one_short_phase_boundaries() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    let baseline = complete(
        prepared(&c, &f, MathRenderer::MathMLOnly)?
            .into_render()
            .map_err(|e| format!("{e:?}"))?,
    )?;
    for field in 0..8 {
        let mut cap = amount(baseline.renders[0].usage, field)?;
        let mut settled = false;
        for _ in 0..16 {
            let mut limits = OperationLimits::default();
            set_cap(&mut limits, field, cap)?;
            f.json("manifest.json", &manifest(limits))?;
            let result = complete(
                prepared(&c, &f, MathRenderer::MathMLOnly)?
                    .into_render()
                    .map_err(|e| format!("{e:?}"))?,
            )?;
            let used = amount(result.renders[0].usage, field)?;
            assert_eq!(result.files, baseline.files);
            if used == cap {
                settled = true;
                break;
            }
            cap = used;
        }
        assert!(settled, "fixed point for resource {field}");
        if cap == 0 {
            continue;
        }
        // Work/allocation/output also meter the encoded cap's decimal digits.
        if matches!(field, 1 | 4 | 5) {
            assert_eq!(
                cap.to_string().len(),
                (cap - 1).to_string().len(),
                "fixture cap digit width"
            );
        }
        let mut short = OperationLimits::default();
        set_cap(&mut short, field, cap - 1)?;
        f.json("manifest.json", &manifest(short))?;
        let RenderProgress::Paused { job, .. } = prepared(&c, &f, MathRenderer::MathMLOnly)?
            .into_render()
            .map_err(|e| format!("{e:?}"))?
            .run()
        else {
            return Err(format!("one short resource {field} must pause").into());
        };
        let expected = match field {
            0 => StopReason::SourceLimit,
            1 => StopReason::WorkLimit,
            2 => StopReason::DepthLimit,
            3 => StopReason::NodeLimit,
            4 => StopReason::AllocationLimit,
            5 => StopReason::OutputLimit,
            6 => StopReason::DiagnosticLimit,
            _ => StopReason::EventLimit,
        };
        assert_eq!(job.reason(), expected);
        assert_eq!(job.usage(), total(job.preparation(), job.attempts()));
    }
    Ok(())
}
#[test]
#[ignore = "requires NEPL3_KATEX_NODE_MODULES and Node >=24 with locked npm packages"]
fn retained_svg_render_pinned_native_survives_package_receipt_stop() -> crate::Result<()> {
    let packages = std::fs::canonicalize(std::env::var("NEPL3_KATEX_NODE_MODULES")?)?;
    assert!(packages.join("katex/package.json").is_file());
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    let baseline = complete(
        prepared(&c, &f, MathRenderer::KaTeXPreferred)?
            .into_render()
            .map_err(|e| format!("{e:?}"))?,
    )?;
    let receipt = check_output(&baseline)?;
    assert_eq!(
        receipt["package"]["katex_generation"]["successful_visual_pages"],
        2
    );
    assert_eq!(
        receipt["package"]["katex_generation"]["successful_visuals"],
        4
    );
    let before = &receipt["package"]["pre_receipt_usage"];
    let low = OperationLimits {
        output_bytes: before["output_bytes"].as_u64().ok_or("output snapshot")?,
        ..Default::default()
    };
    assert!(low.output_bytes > 0);
    f.json("manifest.json", &manifest(low))?;
    let RenderProgress::Paused { job, .. } = prepared(&c, &f, MathRenderer::KaTeXPreferred)?
        .into_render()
        .map_err(|e| format!("{e:?}"))?
        .run()
    else {
        return Err("package receipt stop".into());
    };
    assert_eq!(job.reason(), StopReason::OutputLimit);
    assert_eq!(job.attempts()[0].usage.output_bytes, low.output_bytes);
    assert!(job.attempts()[0].usage.work >= before["work"].as_u64().ok_or("work snapshot")?);
    let prep = format!("{:?}", job.preparation());
    for name in [
        "manifest.json",
        "one.nepld",
        "two.nepld",
        "one.svg",
        "two.svg",
        "manual.txt",
    ] {
        std::fs::remove_file(f.root().join(name))?;
    }
    let final_result = complete(
        job.resume(budget().limits(), OperationLimits::default())
            .map_err(|_| "output grant")?,
    )?;
    assert_eq!(final_result.files, baseline.files);
    assert_eq!(format!("{:?}", final_result.preparation), prep);
    assert_eq!(final_result.renders.len(), 2);
    let final_receipt = check_output(&final_result)?;
    assert_eq!(
        final_receipt["package"]["katex_generation"]["successful_visuals"],
        4
    );
    assert_eq!(
        final_receipt["package"]["katex_generation"]["host_invocation_recorded"],
        false
    );
    assert!(
        final_receipt["job_execution"]["native_evidence_scope"]
            .as_str()
            .ok_or("scope")?
            .contains("earlier interrupted attempts are not retained")
    );
    Ok(())
}
#[test]
fn retained_svg_render_after_preparation_resume_keeps_actual_page_profiles() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture(OperationLimits::default())?;
    f.write(
        "one.nepld",
        "article en \"One\" body cons image asset \"logo\" none \"image\" none nil",
    )?;
    let baseline = prepared(&c, &f, MathRenderer::MathMLOnly)?;
    let first = baseline.attempts()[0].stages[0].usage.source_bytes;
    let second = baseline.attempts()[1].stages[0].usage.source_bytes;
    assert!(first < second);
    let mut phases = job::preparation::PreparationLimits::default();
    phases.parse_limits.source_bytes = first;
    let mut value = manifest(OperationLimits::default());
    value["parse_limits"] = serde_json::to_value(phases.parse_limits)?;
    f.json("manifest.json", &value)?;
    let Progress::Paused { job, .. } = Preparation::capture(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::MathMLOnly,
        budget().limits(),
        OperationLimits::default(),
    )?
    .run() else {
        return Err("second page preparation pause".into());
    };
    assert_eq!(job.prepared_count(), 1);
    let first_profile = job.attempts()[0].stages[0].profile.ok_or("first profile")?;
    for name in [
        "manifest.json",
        "one.nepld",
        "two.nepld",
        "one.svg",
        "two.svg",
        "manual.txt",
    ] {
        std::fs::remove_file(f.root().join(name))?;
    }
    phases.parse_limits.source_bytes = second;
    let Progress::Prepared(ready) = job
        .resume(budget().limits(), phases)
        .map_err(|_| "source grant")?
        .run()
    else {
        return Err("completed preparation".into());
    };
    assert_eq!(
        ready
            .attempts()
            .iter()
            .map(|a| a.page_index)
            .collect::<Vec<_>>(),
        [0, 1, 1]
    );
    let second_profile = ready.attempts()[2].stages[0]
        .profile
        .ok_or("second profile")?;
    let snapshots = format!("{:?}", ready.attempts());
    let result = complete(ready.into_render().map_err(|e| format!("{e:?}"))?)?;
    assert_eq!(format!("{:?}", result.preparation), snapshots);
    assert_eq!(result.usage, total(&result.preparation, &result.renders));
    let receipt: Value = serde_json::from_str(&result.manifest)?;
    let origins = &receipt["package"]["source_input"]["sources"];
    assert_eq!(
        origins[0]["operations"][0]["requested"]["source_bytes"],
        first
    );
    assert_eq!(
        origins[1]["operations"][0]["requested"]["source_bytes"],
        second
    );
    assert_eq!(
        origins[0]["operations"][0]["profile"],
        crate::doc::export::digest_hex(first_profile)
    );
    assert_eq!(
        origins[1]["operations"][0]["profile"],
        crate::doc::export::digest_hex(second_profile)
    );
    assert_eq!(receipt["identity"], identity(&receipt)?);
    let mut changed = receipt.clone();
    changed["job_execution"]["preparation"]["attempts"][0]["stages"][0]["profile"] =
        json!("different");
    assert_ne!(identity(&changed)?, identity(&receipt)?);
    Ok(())
}
