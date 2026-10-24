//! Output-attempt ownership over a frozen canonical batch. No admission proofs
//! survive an attempt. The ordinary canonical entry remains single-attempt.
#[path = "retry/preparation.rs"]
pub(crate) mod preparation;
use super::*;
use crate::doc::export::pages::resources::job::{Attempt, Running};
use nepl3_core::budget::{Limits, StopReason, Usage};

use crate::doc::export::pages::resources::{self, OperationLimits};
use serde::{Deserialize, Serialize};

/// Cumulative rendering allowance, independent of registry preparation caps.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OutputRetryLimits {
    #[serde(default)]
    pub render_job_limits: OperationLimits,
    #[serde(default)]
    pub output_limits: OperationLimits,
}

pub(crate) fn generate(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    selected: OutputRetryLimits,
    choose: impl FnMut(StopReason, Usage, Limits, Limits) -> Result<Option<(Limits, Limits)>>,
) -> Result<Generated> {
    let registry = parse_registry(&raw)?;
    let mut preparation = registry.output_limits.budget();
    let mut batch = prepare_batch(root, manifest, raw, &mut preparation)?;
    batch.prepare_legacy(&mut preparation)?;
    let mut done = render(
        &batch,
        Running::new(selected.render_job_limits.budget().limits()),
        selected.output_limits.budget().limits(),
        choose,
    )?;
    let mut receipt: serde_json::Value = serde_json::from_str(&done.generated.manifest)?;
    receipt["render_job"] = serde_json::json!({
        "contract":"nepl3.canonical-markdown.output-retry/1",
        "usage":resources::usage(done.usage),
        "limits":resources::limits(done.limits),
        "attempts":done.attempts.iter().map(|(limits,usage,stop)| serde_json::json!({
            "limits":resources::limits(*limits),"usage":resources::usage(*usage),"stop":stop.map(|r|format!("{r:?}"))
        })).collect::<Vec<_>>(),
        "scope":"Cumulative render attempts only; preparation, host bookkeeping, job receipt serialization and I/O excluded.",
        "preparation_output_budget":{"limits":resources::limits(preparation.limits()),"usage":resources::usage(preparation.usage())}
    });
    done.generated.manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    Ok(done.generated)
}

struct Rendered {
    generated: Generated,
    usage: Usage,
    limits: Limits,
    attempts: Vec<(Limits, Usage, Option<StopReason>)>,
}

fn render(
    batch: &PreparedBatch,
    mut running: Running,
    mut phase: Limits,
    mut choose: impl FnMut(StopReason, Usage, Limits, Limits) -> Result<Option<(Limits, Limits)>>,
) -> Result<Rendered> {
    let mut attempts = Vec::new();
    loop {
        attempts.try_reserve(1).map_err(|error| {
            format!(
                "HostAllocationFailure: {error}; cumulative job usage={:?}; attempts={attempts:?}",
                running.usage()
            )
        })?;
        let attempt = running.run_budget(phase, |budget| {
            let generated = batch.render(budget.usage(), budget);
            attempts.push((budget.limits(), budget.usage(), budget.poll().err()));
            generated
        });
        match attempt {
            Attempt::Complete { value, running } => {
                return Ok(Rendered {
                    generated: value,
                    usage: running.usage(),
                    limits: running.limits(),
                    attempts,
                });
            }
            Attempt::Paused { mut paused, .. } => loop {
                let Some((job_limits, output_limits)) =
                    choose(paused.reason(), paused.usage(), paused.limits(), phase).map_err(|error| format!("{error}; cumulative job usage={:?}; previous stop={:?}; attempts={attempts:?}", paused.usage(), paused.reason()))?
                else {
                    let cancelled = paused.cancel();
                    return Err(format!("Cancelled: {cancelled:?}; attempts={attempts:?}").into());
                };
                if !crate::doc::export::pages::resources::job::covers(output_limits, phase) {
                    continue;
                }
                match paused.resume(job_limits) {
                    Ok(next) => {
                        running = next;
                        phase = output_limits;
                        break;
                    }
                    Err(rejected) => paused = rejected.into_paused(),
                }
            },
            Attempt::Failed { error, usage, .. } => {
                return Err(format!(
                    "{error}; cumulative job usage={usage:?}; attempts={attempts:?}"
                )
                .into());
            }
            Attempt::Cancelled(cancelled) => {
                return Err(format!("Cancelled: {cancelled:?}; attempts={attempts:?}").into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;
    #[test]
    fn passive_files_remain_frozen_across_repeated_stops() -> Result<()> {
        let f = Fixture::new()?;
        f.write("doc/page.nepld", r#"article en "Page" body cons paragraph cons sentence cons link relative "guide.md" none text "Guide" cons link relative "../design/tasks.json" none text "Data" nil nil nil"#)?;
        f.write("doc/aliases.json", "[]")?;
        f.write("doc/guide.md", "# Original guide\n")?;
        f.write("design/tasks.json", "{\"version\":1}")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page","source":"doc/page.nepld","projection":"doc/page.md","route":"docs/page.html",
            "aliases":"doc/aliases.json","renderer":RENDERER
        }],"files":[
            {"id":"guide","source":"doc/guide.md","route":"sources/guide.md"},
            {"id":"data","source":"design/tasks.json","route":"sources/tasks.json"}
        ]}))?;
        let expected = generate_from_registry(
            f.root(),
            "pages.json",
            raw.clone(),
            &mut crate::doc::source::budget(),
        )?;
        let large = crate::doc::source::budget().limits();
        let mut selected = OutputRetryLimits::default();
        selected.output_limits.work = 10;
        let mut prompts = 0;
        let done = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            selected,
            |_, _, _, phase| {
                prompts += 1;
                f.write("doc/guide.md", "# Replaced\n")?;
                f.write("design/tasks.json", "{\"version\":999}")?;
                Ok(Some((large, if prompts == 1 { phase } else { large })))
            },
        )?;
        let changed = generate_from_registry(
            f.root(),
            "pages.json",
            raw,
            &mut crate::doc::source::budget(),
        )?;
        let changed: serde_json::Value = serde_json::from_str(&changed.manifest)?;
        assert_eq!(prompts, 2);
        assert_eq!(done.files, expected.files);
        let actual: serde_json::Value = serde_json::from_str(&done.manifest)?;
        let expected: serde_json::Value = serde_json::from_str(&expected.manifest)?;
        for field in ["input_pageset", "input_context"] {
            let before = expected
                .get(field)
                .filter(|v| !v.is_null())
                .ok_or("missing canonical identity")?;
            let after = actual
                .get(field)
                .filter(|v| !v.is_null())
                .ok_or("missing retry identity")?;
            assert_eq!(after, before);
            assert_ne!(
                changed
                    .get(field)
                    .filter(|v| !v.is_null())
                    .ok_or("missing changed identity")?,
                before
            );
        }
        assert_eq!(
            actual["render_job"]["attempts"]
                .as_array()
                .ok_or("attempts")?
                .len(),
            3
        );
        Ok(())
    }

    #[test]
    fn late_output_stop_recharges_resolution_and_preserves_cumulative_usage() -> Result<()> {
        let f = Fixture::new()?;
        f.write(
            "page.nepld",
            r#"article en "Page" body cons paragraph cons "{Text/note}" nil nil"#,
        )?;
        f.write("aliases.json", "[]")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page","source":"page.nepld","projection":"page.md","route":"page.md",
            "aliases":"aliases.json","renderer":FOOTNOTES_RENDERER
        }]}))?;
        let large = crate::doc::source::budget().limits();
        let batch = prepare_batch(f.root(), "pages.json", raw, &mut Budget::new(large))?;
        let mut full = Budget::new(large);
        let expected = batch.render(Usage::default(), &mut full)?;
        let phase = Limits {
            work: full
                .usage()
                .work
                .checked_sub(256)
                .ok_or("fixture too small")?,
            ..large
        };
        let done = render(&batch, Running::new(large), phase, |reason, usage, _, _| {
            assert_eq!(reason, StopReason::WorkLimit);
            assert!(usage.nodes > 0);
            assert!(usage.output_bytes > 0);
            assert!(usage.work > full.usage().work / 2);
            Ok(Some((large, large)))
        })?;
        assert_eq!(done.generated.files, expected.files);
        assert_eq!(done.attempts.len(), 2);
        assert!(done.usage.work > full.usage().work);
        assert!(done.attempts[1].1.nodes >= done.attempts[0].1.nodes);
        Ok(())
    }

    #[test]
    fn generated_retry_freezes_svg_alias_registry_and_records_attempts() -> Result<()> {
        let f = Fixture::new()?;
        let source =
            r#"article en "Original" body cons image asset "figure" none "Picture" none nil"#;
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10"/></svg>"#;
        f.write("page.nepld", source)?;
        f.write(
            "aliases.json",
            r#"[{"section":null,"name":"legacy-title"}]"#,
        )?;
        f.write("figure.svg", svg)?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page", "source":"page.nepld", "projection":"page.md", "route":"page.md",
            "aliases":"aliases.json", "renderer":FOOTNOTES_RENDERER
        }], "files":[{"id":"figure","source":"figure.svg","route":"assets/figure.svg"}]}))?;
        f.write("pages.json", &raw)?;
        let expected = generate_from_registry(
            f.root(),
            "pages.json",
            raw.clone(),
            &mut crate::doc::source::budget(),
        )?;
        let large = crate::doc::source::budget().limits();
        let mut selected = OutputRetryLimits::default();
        selected.output_limits.work = 10;
        let actual = generate(f.root(), "pages.json", raw, selected, |_, _, _, _| {
            for path in ["page.nepld", "aliases.json", "figure.svg", "pages.json"] {
                std::fs::remove_file(f.root().join(path))?;
            }
            Ok(Some((large, large)))
        })?;
        assert_eq!(actual.files, expected.files);
        assert!(
            actual
                .files
                .iter()
                .any(|(path, text)| path == "assets/figure.svg" && text == svg)
        );
        let receipt: serde_json::Value = serde_json::from_str(&actual.manifest)?;
        assert_eq!(
            receipt["render_job"]["attempts"]
                .as_array()
                .ok_or("attempts")?
                .len(),
            2
        );
        assert_eq!(
            receipt["render_job"]["contract"],
            "nepl3.canonical-markdown.output-retry/1"
        );
        Ok(())
    }

    #[test]
    fn output_retry_retains_failed_work_and_frozen_source() -> Result<()> {
        let f = Fixture::new()?;
        f.write(
            "page.nepld",
            r#"article en "Original" body cons paragraph cons "{Text/note}" nil nil"#,
        )?;
        f.write("aliases.json", "[]")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page", "source":"page.nepld", "projection":"page.md", "route":"page.md",
            "aliases":"aliases.json", "renderer":FOOTNOTES_RENDERER
        }]}))?;
        let large = crate::doc::source::budget().limits();
        let batch = prepare_batch(f.root(), "pages.json", raw, &mut Budget::new(large))?;
        let expected = batch.render(Usage::default(), &mut Budget::new(large))?;
        let short = Limits { work: 10, ..large };
        let mut prompts = 0;
        let done = render(&batch, Running::new(large), short, |reason, usage, _, _| {
            assert_eq!(reason, StopReason::WorkLimit);
            assert!(usage.work > 0);
            prompts += 1;
            f.write("page.nepld", "invalid replacement")?;
            f.write("aliases.json", "invalid replacement")?;
            Ok(Some((large, large)))
        })?;
        assert_eq!(prompts, 1);
        assert_eq!(done.generated.files, expected.files);
        assert_eq!(done.attempts.len(), 2);
        assert_eq!(done.attempts[0].2, Some(StopReason::WorkLimit));
        assert_eq!(done.attempts[1].2, None);
        assert_eq!(
            done.usage.work,
            done.attempts.iter().map(|v| v.1.work).sum::<u64>()
        );
        assert_eq!(
            done.usage.depth,
            done.attempts
                .iter()
                .map(|v| v.1.depth)
                .max()
                .ok_or("attempts missing")?
        );
        assert_eq!(
            done.attempts[1].0.work,
            large.work - done.attempts[0].1.work
        );
        Ok(())
    }
    #[test]
    fn compact_math_svg_receipts_match_across_normal_and_retry_entries() -> Result<()> {
        let f = Fixture::new()?;
        f.write("page.nepld", r#"article en "Compact" body cons paragraph cons "{Text/note}" nil cons display Math frac 3 7 cons image asset "figure" none "Image" none nil"#)?;
        f.write("aliases.json", "[]")?;
        f.write("figure.svg", r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10"/></svg>"#)?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page","source":"page.nepld","projection":"page.md","route":"page.md",
            "aliases":"aliases.json","renderer":COMPACT_RENDERER
        }],"files":[{"id":"figure","source":"figure.svg","route":"assets/figure.svg"}]}))?;
        let expected = generate_from_registry(
            f.root(),
            "pages.json",
            raw.clone(),
            &mut crate::doc::source::budget(),
        )?;
        let batch = prepare_batch(
            f.root(),
            "pages.json",
            raw.clone(),
            &mut crate::doc::source::budget(),
        )?;
        let mut measured = crate::doc::source::budget();
        batch.render(Usage::default(), &mut measured)?;
        let short_work = measured
            .usage()
            .work
            .checked_sub(256)
            .ok_or("fixture too small")?;
        let large = crate::doc::source::budget().limits();
        let mut selected = OutputRetryLimits::default();
        selected.output_limits.work = short_work;
        let mut stops = 0;
        let retried = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            selected,
            |reason, used, _, _| {
                assert_eq!(reason, StopReason::WorkLimit);
                assert!(used.work > 0 && used.output_bytes > 0);
                stops += 1;
                Ok(Some((large, large)))
            },
        )?;
        assert_eq!(stops, 1);
        let mut selected = preparation::RetryLimits::default();
        selected.output_limits.work = short_work;
        let mut stops = 0;
        let prepared = preparation::generate(
            f.root(),
            "pages.json",
            raw,
            selected,
            |_, phase, reason, used| {
                assert!(matches!(phase, preparation::Phase::Output));
                assert_eq!(reason, StopReason::WorkLimit);
                assert!(used.work > 0);
                stops += 1;
                Ok(Some(preparation::RetryLimits::default()))
            },
        )?;
        assert_eq!(stops, 1);
        let baseline: serde_json::Value = serde_json::from_str(&expected.manifest)?;
        for actual in [&retried, &prepared] {
            assert_eq!(actual.files, expected.files);
            let receipt: serde_json::Value = serde_json::from_str(&actual.manifest)?;
            for key in [
                "input_pageset",
                "input_context",
                "page_dependencies",
                "files",
            ] {
                assert_eq!(receipt[key], baseline[key], "{key}");
            }
        }
        let output: serde_json::Value = serde_json::from_str(&retried.manifest)?;
        let attempts = output["render_job"]["attempts"]
            .as_array()
            .ok_or("output attempts")?;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["stop"], "WorkLimit");
        assert!(attempts[1]["stop"].is_null());
        let first = attempts[0]["usage"]["work"].as_u64().ok_or("failed work")?;
        let total = output["render_job"]["usage"]["work"]
            .as_u64()
            .ok_or("total work")?;
        let success = baseline["output_budget"]["usage_before_receipt_serialization"]["work"]
            .as_u64()
            .ok_or("baseline work")?;
        assert!(first > 0 && total >= first + success);
        let prepared: serde_json::Value = serde_json::from_str(&prepared.manifest)?;
        let attempts = prepared["job_execution"]["render_attempts"]
            .as_array()
            .ok_or("preparation entry output attempts")?;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["stop"], "WorkLimit");
        assert!(attempts[1]["stop"].is_null());
        Ok(())
    }
    #[test]
    fn legacy_output_retry_preserves_namespace_bytes_and_frozen_inputs() -> Result<()> {
        for mixed in [None, Some(RENDERER), Some(FOOTNOTES_RENDERER)] {
            let f = Fixture::new()?;
            let source = r#"article en "Legacy" body cons paragraph cons sentence cons anno text "term" cons text "note" nil nil nil nil"#;
            f.write("doc/legacy.nepld", source)?;
            f.write(
                "doc/new.nepld",
                r#"article en "New" body cons paragraph cons "Sentence" nil nil"#,
            )?;
            f.write(
                "doc/aliases.json",
                r#"[{"section":null,"name":"legacy-title"}]"#,
            )?;
            f.write("doc/new-aliases.json", "[]")?;
            let mut pages = vec![
                serde_json::json!({"id":"legacy-owner","source":"doc/legacy.nepld","projection":"doc/legacy.md","route":"legacy.html","aliases":"doc/aliases.json","renderer":host::RENDERER}),
            ];
            if let Some(renderer) = mixed {
                pages.push(serde_json::json!({"id":"new-owner","source":"doc/new.nepld","projection":"doc/new.md","route":if is_footnotes(renderer) { "doc/new.md" } else { "new.html" },"aliases":"doc/new-aliases.json","renderer":renderer}));
            }
            let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":pages}))?;
            f.write("pages.json", std::str::from_utf8(&raw)?)?;
            let expected = generate_from_registry(
                f.root(),
                "pages.json",
                raw.clone(),
                &mut crate::doc::source::budget(),
            )?;
            let baseline: serde_json::Value = serde_json::from_str(&expected.manifest)?;
            let full = preparation::generate(
                f.root(),
                "pages.json",
                raw.clone(),
                preparation::RetryLimits::default(),
                |_, _, _, _| Err("unexpected full preparation stop".into()),
            )?;
            assert_eq!(full.files, expected.files);
            let full_receipt: serde_json::Value = serde_json::from_str(&full.manifest)?;
            for key in [
                "files",
                "input_pageset",
                "input_context",
                "page_dependencies",
            ] {
                assert_eq!(full_receipt[key], baseline[key], "full preparation {key}");
            }
            assert_eq!(
                full_receipt["job_execution"]["contract"],
                "nepl3.canonical-markdown.retry-job/2"
            );
            let roles = full_receipt["job_execution"]["preparation_attempts"]
                .as_array()
                .ok_or("attempts")?;
            assert_eq!(roles.len(), if mixed.is_some() { 3 } else { 1 });
            assert_eq!(
                roles[usize::from(mixed.is_some())]["representation"],
                "Legacy"
            );

            let mut batch = prepare_batch(
                f.root(),
                "pages.json",
                raw.clone(),
                &mut crate::doc::source::budget(),
            )?;
            batch.prepare_legacy(&mut crate::doc::source::budget())?;
            // If a render accidentally uses the old parsing path, zero phase
            // caps fail. The output-only retained AST must not reparse or lower.
            batch.phases.parse.work = 0;
            batch.phases.lower.work = 0;
            let mut full_budget = crate::doc::source::budget();
            let direct = batch.render(Default::default(), &mut full_budget)?;
            assert_eq!(direct.files, expected.files);
            let receipt: serde_json::Value = serde_json::from_str(&direct.manifest)?;
            assert_eq!(receipt["files"], baseline["files"]);
            let large = crate::doc::source::budget().limits();
            let short = Limits {
                work: receipt["output_budget"]["usage_before_receipt_serialization"]["work"]
                    .as_u64()
                    .ok_or("render work")?
                    .checked_sub(1)
                    .ok_or("empty render")?,
                ..large
            };
            let mut late_stops = 0;
            let late = render(&batch, Running::new(large), short, |reason, used, _, _| {
                assert_eq!(reason, StopReason::WorkLimit);
                assert!(used.work > 0 && used.output_bytes > 0 && used.allocation_units > 0);
                late_stops += 1;
                Ok(Some((large, large)))
            })?;
            assert_eq!(late_stops, 1);
            assert_eq!(late.generated.files, expected.files);
            assert_eq!(late.attempts.len(), 2);
            let total = resources::usage(late.usage);
            for key in [
                "work",
                "allocation_units",
                "nodes",
                "source_bytes",
                "output_bytes",
                "events",
                "diagnostics",
            ] {
                let mut sum = 0u64;
                for (_, u, _) in &late.attempts {
                    sum += resources::usage(*u)[key]
                        .as_u64()
                        .ok_or("missing usage counter")?;
                }
                assert_eq!(total[key].as_u64(), Some(sum), "{key}");
            }
            assert_eq!(
                late.usage.depth,
                late.attempts
                    .iter()
                    .map(|(_, u, _)| u.depth)
                    .max()
                    .ok_or("no attempts")?
            );

            let selected = OutputRetryLimits {
                output_limits: OperationLimits {
                    work: 0,
                    ..OperationLimits::default()
                },
                ..OutputRetryLimits::default()
            };
            let large = crate::doc::source::budget().limits();
            let mut prompts = 0;
            let resumed = generate(f.root(), "pages.json", raw, selected, |reason, _, _, _| {
                assert_eq!(reason, StopReason::WorkLimit);
                prompts += 1;
                for path in [
                    "doc/legacy.nepld",
                    "doc/new.nepld",
                    "doc/aliases.json",
                    "doc/new-aliases.json",
                    "pages.json",
                ] {
                    std::fs::remove_file(f.root().join(path))?;
                }
                Ok(Some((large, large)))
            })?;
            assert_eq!(prompts, 1);
            assert_eq!(resumed.files, expected.files);
            let receipt: serde_json::Value = serde_json::from_str(&resumed.manifest)?;
            for key in [
                "files",
                "input_pageset",
                "input_context",
                "page_dependencies",
            ] {
                assert_eq!(receipt[key], baseline[key], "{key}");
            }
            assert_eq!(
                receipt["render_job"]["attempts"]
                    .as_array()
                    .ok_or("attempts")?
                    .len(),
                2
            );
        }
        Ok(())
    }

    #[test]
    fn legacy_output_retry_cancel_and_preparation_stop_are_terminal() -> Result<()> {
        let f = Fixture::new()?;
        f.write("doc/p.nepld", "article en \"Page\" body nil")?;
        f.write("doc/a.json", "[]")?;
        let mut registry = serde_json::json!({"version":1,"pages":[{"id":"p","source":"doc/p.nepld","projection":"doc/p.md","route":"p.html","aliases":"doc/a.json","renderer":host::RENDERER}]});
        let selected = OutputRetryLimits {
            output_limits: OperationLimits {
                work: 0,
                ..OperationLimits::default()
            },
            ..OutputRetryLimits::default()
        };
        let e = generate(
            f.root(),
            "pages.json",
            serde_json::to_vec(&registry)?,
            selected,
            |_, _, _, _| Ok(None),
        )
        .err()
        .ok_or("cancelled job completed")?;
        assert!(e.to_string().contains("Cancelled"), "{e}");
        for stage in ["parse_limits", "lower_limits"] {
            registry
                .as_object_mut()
                .ok_or("registry")?
                .remove("parse_limits");
            registry[stage] = serde_json::to_value(OperationLimits {
                work: 0,
                ..OperationLimits::default()
            })?;
            let e = generate(
                f.root(),
                "pages.json",
                serde_json::to_vec(&registry)?,
                OutputRetryLimits::default(),
                |_, _, _, _| Err("unexpected prompt".into()),
            )
            .err()
            .ok_or("preparation stop ignored")?;
            assert!(!e.to_string().contains("unexpected prompt"), "{e}");
            assert!(e.to_string().contains("WorkLimit"), "{e}");
        }
        Ok(())
    }
}
