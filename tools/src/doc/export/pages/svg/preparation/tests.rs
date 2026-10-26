use super::*;
use crate::{
    doc::source::{budget, compiled},
    testing::Fixture,
};
use serde_json::json;
const ONE: &str = "article en \"One\" body nil";
const TWO: &str = "article en \"Two\" body cons paragraph cons code Math frac 1 2 cons \"Longer frozen second page\" nil nil";
const RAW: &str = "<svg intentionally not validated during preparation>";
fn fixture() -> crate::Result<Fixture> {
    let f = Fixture::new()?;
    f.write("one.nepld", ONE)?;
    f.write("two.nepld", TWO)?;
    f.write("image.svg", RAW)?;
    f.write("passive.txt", "immutable passive bytes")?;
    f.json("manifest.json", &manifest())?;
    Ok(f)
}
fn manifest() -> serde_json::Value {
    json!({"version":1,"pages":[
        {"id":"one","source":"one.nepld","route":"one.html"},
        {"id":"two","source":"two.nepld","route":"two.html"}],
        "files":[{"id":"passive","source":"passive.txt","route":"passive.txt"}],
        "assets":[{"page":"two","assets":[{"id":"logo","source":"image.svg","mime":"image/svg+xml"}]},{"page":"one","assets":[]}],
        "display":{"mode":"single","language":"fr","fallbacks":["ja"]}})
}
fn check_frozen(value: &Frozen, mode: SvgMode) {
    assert_eq!(value.svg_mode(), mode);
    assert!(matches!(value.renderer(), MathRenderer::KaTeXPreferred));
    assert!(
        matches!(value.options().parallel, nepl3_doc_html::ParallelMode::Single { ref language, ref fallbacks } if language == "fr" && fallbacks == &["ja"])
    );
    assert_eq!(value.output_limits(), OperationLimits::default());
    let bindings: Vec<_> = value.bindings().collect();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].page_index(), 1);
    assert_eq!(bindings[1].page_index(), 0);
    assert_eq!(bindings[1].assets().count(), 0);
    let assets: Vec<_> = bindings[0].assets().collect();
    assert_eq!(assets.len(), 1);
    assert_eq!(
        (assets[0].id, assets[0].source, assets[0].svg),
        ("logo", "image.svg", RAW)
    );
}
fn total(attempts: &[base::PageAttempt]) -> Usage {
    attempts
        .iter()
        .flat_map(|a| &a.stages)
        .fold(Usage::default(), |a, s| {
            let b = s.usage;
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
        })
}
#[test]
fn svg_retained_preparation_keeps_inputs_selection_and_completed_pages() -> crate::Result<()> {
    for mode in [SvgMode::Embedded, SvgMode::External] {
        let c = compiled()?;
        let f = fixture()?;
        let capture = || {
            (match mode {
                SvgMode::Embedded => Preparation::capture,
                SvgMode::External => Preparation::capture_external,
            })(
                &c,
                &f.root().join("manifest.json"),
                MathRenderer::KaTeXPreferred,
                budget().limits(),
                OperationLimits::default(),
            )
        };
        let Progress::Prepared(baseline) = capture()?.run() else {
            return Err("baseline".into());
        };
        check_frozen(baseline.frozen(), mode);
        let first = baseline.attempts()[0].stages[0].usage.source_bytes;
        let second = baseline.attempts()[1].stages[0].usage.source_bytes;
        assert!(second > first);
        let mut phases = base::PreparationLimits::default();
        phases.parse_limits.source_bytes = first;
        let mut value = manifest();
        value["parse_limits"] = serde_json::to_value(phases.parse_limits)?;
        f.json("manifest.json", &value)?;
        let Progress::Paused { job, .. } = capture()?.run() else {
            return Err("pause second".into());
        };
        assert_eq!(job.prepared_count(), 1);
        assert_eq!(job.reason(), StopReason::SourceLimit);
        let profile = job.attempts()[0].stages[0].profile;
        let consumed = job.usage();
        assert_eq!(consumed, total(job.attempts()));
        check_frozen(job.frozen(), mode);
        // No subsequent attempt may reread any captured source, including page 1.
        for file in [
            "manifest.json",
            "one.nepld",
            "two.nepld",
            "image.svg",
            "passive.txt",
        ] {
            std::fs::remove_file(f.root().join(file))?;
        }
        let Progress::Paused { job, .. } = job
            .resume(budget().limits(), phases)
            .map_err(|_| "same grant")?
            .run()
        else {
            return Err("same pause".into());
        };
        // Source admission can stop before any measured Work; no refund is allowed.
        assert!(job.usage().work >= consumed.work);
        check_frozen(job.frozen(), mode);
        let prior = job.usage();
        let mut smaller = phases;
        smaller.code_limits.work -= 1;
        let job = job
            .resume(budget().limits(), smaller)
            .err()
            .ok_or("reject shrink")?
            .into_paused();
        assert_eq!(job.usage(), prior);
        check_frozen(job.frozen(), mode);
        phases.parse_limits.source_bytes = second;
        let Progress::Prepared(ready) = job
            .resume(budget().limits(), phases)
            .map_err(|_| "grow")?
            .run()
        else {
            return Err("ready".into());
        };
        assert_eq!(ready.prepared_count(), 2);
        assert_eq!(
            ready
                .attempts()
                .iter()
                .map(|a| a.page_index)
                .collect::<Vec<_>>(),
            [0, 1, 1, 1]
        );
        assert_eq!(ready.attempts()[0].stages[0].profile, profile);
        assert_eq!(
            ready.documents().collect::<Vec<_>>(),
            baseline.documents().collect::<Vec<_>>()
        );
        assert_eq!(ready.usage(), total(ready.attempts()));
        check_frozen(ready.frozen(), mode);
        // The malformed raw SVG is deliberately retained: Prepared is not admission.
    }
    Ok(())
}
#[test]
fn svg_retained_preparation_preserves_stages_failure_and_cancellation() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture()?;
    for stage in [base::Stage::Parse, base::Stage::Lower, base::Stage::Code] {
        let mut phases = base::PreparationLimits::default();
        match stage {
            base::Stage::Parse => phases.parse_limits.work = 0,
            base::Stage::Lower => phases.lower_limits.work = 0,
            base::Stage::Code => phases.code_limits.work = 0,
        }
        let mut value = manifest();
        value["parse_limits"] = serde_json::to_value(phases.parse_limits)?;
        value["lower_limits"] = serde_json::to_value(phases.lower_limits)?;
        f.json("manifest.json", &value)?;
        let Progress::Paused { job, .. } = Preparation::capture(
            &c,
            &f.root().join("manifest.json"),
            MathRenderer::MathMLOnly,
            budget().limits(),
            phases.code_limits,
        )?
        .run() else {
            return Err("stage pause".into());
        };
        assert_eq!(job.reason(), StopReason::WorkLimit);
        assert_eq!(
            job.attempts()[0].stages.last().map(|v| v.stage),
            Some(stage)
        );
        assert_eq!(job.usage(), total(job.attempts()));
        assert!(matches!(job.frozen().renderer(), MathRenderer::MathMLOnly));
        let usage = job.usage();
        let cancelled = job.cancel();
        assert_eq!(cancelled.accounting.usage, usage);
        assert_eq!(
            cancelled.accounting.previous_stop,
            Some(StopReason::WorkLimit)
        );
    }
    f.json("manifest.json", &manifest())?;
    let cancelled = Preparation::capture(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::MathMLOnly,
        budget().limits(),
        OperationLimits::default(),
    )?
    .cancel();
    assert_eq!(cancelled.accounting.usage, Usage::default());
    assert!(cancelled.attempts.is_empty());
    f.write("one.nepld", "invalid malformed document")?;
    let Progress::Failed {
        usage, attempts, ..
    } = Preparation::capture(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::MathMLOnly,
        budget().limits(),
        OperationLimits::default(),
    )?
    .run()
    else {
        return Err("invalid syntax is terminal".into());
    };
    assert_eq!(usage, total(&attempts));
    assert!(attempts.iter().flat_map(|a| &a.stages).any(|s| s.failed));
    Ok(())
}
fn shrink(v: &mut Limits, i: usize) -> Result<(), String> {
    let field = match i {
        0 => &mut v.source_bytes,
        1 => &mut v.work,
        2 => &mut v.depth,
        3 => &mut v.nodes,
        4 => &mut v.allocation_units,
        5 => &mut v.output_bytes,
        6 => &mut v.diagnostics,
        7 => &mut v.events,
        _ => return Err("invalid resource index".into()),
    };
    *field -= 1;
    Ok(())
}

#[test]
fn svg_retained_preparation_rejects_every_shrunk_dimension_without_loss() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture()?;
    let mut limits = budget().limits();
    limits.source_bytes = 1;
    let phases = base::PreparationLimits::default();
    let Progress::Paused { mut job, .. } = Preparation::capture(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::KaTeXPreferred,
        limits,
        phases.code_limits,
    )?
    .run() else {
        return Err("initial source pause".into());
    };
    let usage = job.usage();
    let attempts = format!("{:?}", job.attempts());
    for group in 0..4 {
        for resource in 0..8 {
            let mut candidate = limits;
            let mut selected = phases;
            if group == 0 {
                shrink(&mut candidate, resource)?;
            } else {
                let value = match group {
                    1 => &mut selected.parse_limits,
                    2 => &mut selected.lower_limits,
                    _ => &mut selected.code_limits,
                };
                let mut cap = value.budget().limits();
                shrink(&mut cap, resource)?;
                *value = operation(cap);
            }
            job = job
                .resume(candidate, selected)
                .err()
                .ok_or("shrunk grant accepted")?
                .into_paused();
            assert_eq!(job.usage(), usage);
            assert_eq!(format!("{:?}", job.attempts()), attempts);
            assert_eq!(job.reason(), StopReason::SourceLimit);
            check_frozen(job.frozen(), SvgMode::Embedded);
        }
    }
    let Progress::Prepared(ready) = job
        .resume(budget().limits(), phases)
        .map_err(|_| "final grant")?
        .run()
    else {
        return Err("ready after rejection".into());
    };
    check_frozen(ready.frozen(), SvgMode::Embedded);
    assert_eq!(ready.usage(), total(ready.attempts()));
    Ok(())
}
#[test]
fn svg_retained_capture_keeps_finite_asset_and_source_boundaries() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture()?;
    let capture = || {
        Preparation::capture(
            &c,
            &f.root().join("manifest.json"),
            MathRenderer::MathMLOnly,
            budget().limits(),
            OperationLimits::default(),
        )
    };
    for count in [32, 33] {
        let mut value = manifest();
        value["assets"] = json!([{"page":"one","assets":(0..count).map(|i|json!({"id":format!("svg{i}"),"source":"image.svg","mime":"image/svg+xml"})).collect::<Vec<_>>()}]);
        f.json("manifest.json", &value)?;
        assert_eq!(capture().is_ok(), count == 32);
    }
    let maximum = nepl3_markup::html::svg::MAX_BYTES;
    f.write("image.svg", "x".repeat(maximum))?;
    f.json("manifest.json", &manifest())?;
    assert!(capture().is_ok());
    f.write("image.svg", "x".repeat(maximum + 1))?;
    assert!(matches!(capture(), Err(e) if e=="CapturedInputLimit"));
    f.write("image.svg", "x".repeat(maximum))?;
    f.write("extra.svg", "x")?;
    let mut value = manifest();
    value["assets"] = json!([{"page":"one","assets":(0..4).map(|i|json!({"id":format!("svg{i}"),"source":"image.svg","mime":"image/svg+xml"})).collect::<Vec<_>>()}]);
    f.json("manifest.json", &value)?;
    assert!(capture().is_ok());
    value["assets"][0]["assets"]
        .as_array_mut()
        .ok_or("assets")?
        .push(json!({"id":"extra","source":"extra.svg","mime":"image/svg+xml"}));
    f.json("manifest.json", &value)?;
    assert!(matches!(capture(), Err(e) if e=="SvgByteLimit"));
    f.write("image.svg", RAW)?;
    for count in [128, 129] {
        let mut value = manifest();
        value["pages"] = json!((0..count).map(|i|json!({"id":format!("p{i}"),"source":"one.nepld","route":format!("p{i}.html")})).collect::<Vec<_>>());
        value["assets"] = json!(
            (0..count)
                .map(|i| json!({"page":format!("p{i}"),"assets":[]}))
                .collect::<Vec<_>>()
        );
        f.json("manifest.json", &value)?;
        assert_eq!(capture().is_ok(), count == 128);
    }
    f.json("manifest.json", &manifest())?;
    let available = crate::doc::export::MAX_SOURCE_BYTES as usize - ONE.len() - TWO.len();
    f.write("passive.txt", "x".repeat(available))?;
    assert!(capture().is_ok());
    f.write("passive.txt", "x".repeat(available + 1))?;
    assert!(matches!(capture(), Err(e) if e=="SourceLimit"));
    Ok(())
}
#[test]
fn svg_retained_output_allowance_is_frozen_but_not_preparation_budget() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture()?;
    let zero = OperationLimits {
        source_bytes: 0,
        work: 0,
        depth: 0,
        nodes: 0,
        allocation_units: 0,
        output_bytes: 0,
        diagnostics: 0,
        events: 0,
    };
    let mut value = manifest();
    value["output_limits"] = serde_json::to_value(zero)?;
    f.json("manifest.json", &value)?;
    let mut phases = base::PreparationLimits::default();
    phases.code_limits.work = 0;
    let Progress::Paused { job, .. } = Preparation::capture(
        &c,
        &f.root().join("manifest.json"),
        MathRenderer::MathMLOnly,
        budget().limits(),
        phases.code_limits,
    )?
    .run() else {
        return Err("code pause".into());
    };
    assert!(job.usage().work > 0);
    assert_eq!(job.frozen().output_limits(), zero);
    let before = job.usage();
    let records = format!("{:?}", job.attempts());
    let mut smaller = phases;
    smaller.parse_limits.source_bytes -= 1;
    let job = job
        .resume(budget().limits(), smaller)
        .err()
        .ok_or("shrink")?
        .into_paused();
    assert_eq!(job.usage(), before);
    assert_eq!(format!("{:?}", job.attempts()), records);
    assert_eq!(job.frozen().output_limits(), zero);
    let Progress::Prepared(ready) = job
        .resume(budget().limits(), base::PreparationLimits::default())
        .map_err(|_| "resume")?
        .run()
    else {
        return Err("output zero does not cap preparation".into());
    };
    assert!(ready.usage().work > before.work);
    assert_eq!(ready.frozen().output_limits(), zero);
    assert_eq!(ready.usage(), total(ready.attempts()));
    Ok(())
}
#[test]
fn svg_retained_capture_rejects_invalid_binding_metadata_and_bytes() -> crate::Result<()> {
    let c = compiled()?;
    let f = fixture()?;
    let capture = || {
        Preparation::capture(
            &c,
            &f.root().join("manifest.json"),
            MathRenderer::MathMLOnly,
            budget().limits(),
            OperationLimits::default(),
        )
    };
    let mut values = Vec::new();
    let mut v = manifest();
    v["assets"][0]["page"] = json!("unknown");
    values.push(v);
    let mut v = manifest();
    v["assets"][1]["page"] = json!("two");
    values.push(v);
    let mut v = manifest();
    v["assets"][0]["assets"] = json!([{"id":"logo","source":"image.svg","mime":"image/svg+xml"},{"id":"logo","source":"image.svg","mime":"image/svg+xml"}]);
    values.push(v);
    let mut v = manifest();
    v["assets"][0]["assets"][0]["mime"] = json!("text/plain");
    values.push(v);
    let mut v = manifest();
    v["assets"][0]["assets"][0]["source"] = json!("../image.svg");
    values.push(v);
    for v in values {
        f.json("manifest.json", &v)?;
        assert!(capture().is_err());
    }
    f.json("manifest.json", &manifest())?;
    std::fs::write(f.root().join("image.svg"), [0xff])?;
    assert!(capture().is_err());
    f.write("image.svg", RAW)?;
    let text = serde_json::to_string(&manifest())?;
    f.write(
        "manifest.json",
        format!("{text}{}", " ".repeat(65536 - text.len())),
    )?;
    assert!(capture().is_ok());
    f.write(
        "manifest.json",
        format!("{text}{}", " ".repeat(65537 - text.len())),
    )?;
    assert!(matches!(capture(),Err(e) if e=="CapturedInputLimit"));
    Ok(())
}
