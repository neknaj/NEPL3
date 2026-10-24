//! Complete-page checkpoints for explicit canonical Markdown retry jobs.
//! Input capture and retained host bookkeeping are outside logical phase usage.
use super::*;
use crate::doc::export::pages::resources::job::covers;
use crate::doc::export::pages::resources::job::preparation::{Observation, with_prepared_document};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetryLimits {
    #[serde(default)]
    pub job_limits: OperationLimits,
    #[serde(default)]
    pub parse_limits: OperationLimits,
    #[serde(default)]
    pub lower_limits: OperationLimits,
    #[serde(default)]
    pub output_limits: OperationLimits,
}
impl RetryLimits {
    pub(crate) fn covers(self, old: Self) -> bool {
        [
            self.job_limits,
            self.parse_limits,
            self.lower_limits,
            self.output_limits,
        ]
        .into_iter()
        .zip([
            old.job_limits,
            old.parse_limits,
            old.lower_limits,
            old.output_limits,
        ])
        .all(|(a, b)| covers(a.budget().limits(), b.budget().limits()))
    }
    fn phases(self) -> PhaseLimits {
        PhaseLimits {
            parse: self.parse_limits.budget().limits(),
            lower: self.lower_limits.budget().limits(),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Phase {
    Preparation { page: usize, role: Representation },
    Output,
}
/// A page may require two distinct source namespaces in a mixed registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) enum Representation {
    PageSet,
    Legacy,
}
#[derive(Debug)]
struct PreparationAttempt {
    page_index: usize,
    role: Representation,
    stages: Vec<Observation>,
}
enum PreparedRepresentation {
    PageSet(nepl3_doc_core::model::DocumentSyntax),
    Legacy(host::PreparedLegacy),
}
fn observation(o: &Observation) -> serde_json::Value {
    serde_json::json!({
        "stage":format!("{:?}",o.stage),"requested":resources::limits(o.requested),
        "effective":resources::limits(o.effective),"usage":resources::usage(o.usage),
        "stop":o.stop.map(|s|format!("{s:?}")),"failed":o.failed,
        "accounting_rejected":o.accounting_rejected,"profile":o.profile.map(hex)
    })
}

pub(crate) fn generate(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    mut limits: RetryLimits,
    mut choose: impl FnMut(RetryLimits, Phase, StopReason, Usage) -> Result<Option<RetryLimits>>,
) -> Result<Generated> {
    let Captured {
        raw,
        inputs,
        references,
        ..
    } = capture(root, manifest, raw)?;
    let compiled = crate::doc::source::compiled()?;
    let initial_phases = limits.phases();
    let footnotes = inputs.iter().all(|p| is_footnotes(&p.page.renderer));
    let svg = footnotes && !references.is_empty();
    let aliases = inputs
        .iter()
        .map(|p| serde_json::from_slice::<Vec<annotated::Alias>>(&p.aliases))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let files = references
        .into_iter()
        .map(|(entry, bytes)| PageFile {
            registration: PageRegistration {
                id: entry.id,
                source: entry.source.clone(),
                route: if svg { entry.route } else { entry.source },
            },
            content: FileBytes(bytes),
        })
        .collect();
    let needs_group = inputs.iter().any(|p| p.page.renderer != host::RENDERER);
    let mut checkpoints = Vec::new();
    checkpoints.try_reserve_exact(inputs.len() * 2)?;
    let mut legacy = Vec::new();
    legacy.try_reserve_exact(inputs.len())?;
    for (index, input) in inputs.iter().enumerate() {
        legacy.push(None);
        if needs_group {
            checkpoints.push((index, Representation::PageSet));
        }
        if input.page.renderer == host::RENDERER {
            checkpoints.push((index, Representation::Legacy));
        }
    }
    let mut completed = 0;
    let mut pages = Vec::new();
    pages.try_reserve_exact(inputs.len())?;
    let mut records = Vec::new();
    let mut account = Running::new(limits.job_limits.budget().limits());
    while completed < checkpoints.len() {
        let (index, role) = checkpoints[completed];
        let input = &inputs[index];
        let mut stages = Vec::new();
        stages.try_reserve_exact(2).map_err(|error| {
            format!(
                "HostAllocationFailure: {error}; usage={:?}; preparation={records:?}",
                account.usage()
            )
        })?;
        records.try_reserve(1).map_err(|error| {
            format!(
                "HostAllocationFailure: {error}; usage={:?}; preparation={records:?}",
                account.usage()
            )
        })?;
        let result = account.run(limits.job_limits.budget().limits(), |meter| match role {
            Representation::PageSet => with_prepared_document(
                meter,
                &compiled,
                &input.page.id,
                &input.source,
                limits.phases(),
                &mut stages,
                |document, _, _, _, _| Ok(PreparedRepresentation::PageSet(document)),
            ),
            Representation::Legacy => host::PreparedLegacy::capture_measured(
                &compiled,
                &input.page.source,
                &input.source,
                &input.aliases,
                limits.phases(),
                meter,
                &mut stages,
            )
            .map(PreparedRepresentation::Legacy),
        });
        let accounting_rejected = stages.iter().any(|s| s.accounting_rejected);
        records.push(PreparationAttempt {
            page_index: index,
            role,
            stages,
        });
        match result {
            Attempt::Complete { value, running } => {
                match value {
                    PreparedRepresentation::PageSet(document) => pages.push(PageDocument {
                        registration: PageRegistration {
                            id: input.page.id.clone(),
                            source: input.page.projection.clone(),
                            route: input.page.projection.clone(),
                        },
                        document,
                    }),
                    PreparedRepresentation::Legacy(document) => legacy[index] = Some(document),
                }
                completed += 1;
                account = running;
            }
            Attempt::Paused { mut paused, error } => {
                if accounting_rejected {
                    return Err(format!(
                        "PhaseAccountingInvariant: {error:?}; usage={:?}; preparation={records:?}",
                        paused.usage()
                    )
                    .into());
                }
                loop {
                    let selected = choose(
                        limits,
                        Phase::Preparation { page: index, role },
                        paused.reason(),
                        paused.usage(),
                    )
                    .map_err(|e| {
                        format!(
                            "{e}; usage={:?}; previous stop={:?}; preparation={records:?}",
                            paused.usage(),
                            paused.reason()
                        )
                    })?;
                    let Some(next) = selected else {
                        return Err(format!(
                            "Cancelled: {:?}; preparation={records:?}",
                            paused.cancel()
                        )
                        .into());
                    };
                    if !next.covers(limits) {
                        continue;
                    }
                    match paused.resume(next.job_limits.budget().limits()) {
                        Ok(running) => {
                            limits = next;
                            account = running;
                            break;
                        }
                        Err(rejected) => paused = rejected.into_paused(),
                    }
                }
            }
            Attempt::Failed { error, usage, .. } => {
                return Err(format!("{error}; usage={usage:?}; preparation={records:?}").into());
            }
            Attempt::Cancelled(cancelled) => {
                return Err(format!("Cancelled: {cancelled:?}; preparation={records:?}").into());
            }
        }
    }
    let batch = PreparedBatch {
        compiled,
        manifest: manifest.to_owned(),
        raw,
        inputs,
        phases: initial_phases,
        legacy: Some(legacy),
        prepared: needs_group.then_some(PreparedGroup {
            set: PageSet { pages, files },
            aliases,
            footnotes,
            svg,
        }),
    };
    let prepared_usage = account.usage();
    let mut done = render(
        &batch,
        account,
        limits.output_limits.budget().limits(),
        |reason, usage, _, _| {
            loop {
                let Some(next) = choose(limits, Phase::Output, reason, usage)? else {
                    return Ok(None);
                };
                if !next.covers(limits) {
                    continue;
                }
                limits = next;
                return Ok(Some((
                    limits.job_limits.budget().limits(),
                    limits.output_limits.budget().limits(),
                )));
            }
        },
    )
    .map_err(|e| format!("{e}; preparation={records:?}"))?;
    let mut receipt: serde_json::Value = serde_json::from_str(&done.generated.manifest)?;
    let object = receipt
        .as_object_mut()
        .ok_or("canonical receipt must be an object")?;
    // Per-page profiles may differ after a grant. Do not imply all pages used a
    // single final or registry parse/lower allowance.
    object.remove("parse_limits");
    object.remove("lower_limits");
    receipt["job_execution"] = serde_json::json!({
        "contract":"nepl3.canonical-markdown.retry-job/2",
        "limits":resources::limits(done.limits),"usage":resources::usage(done.usage),
        "prepared_usage":resources::usage(prepared_usage),
        "preparation_attempts":records.iter().enumerate().map(|(attempt_index,a)|serde_json::json!({"attempt_index":attempt_index,"page_index":a.page_index,"representation":a.role,"stages":a.stages.iter().map(observation).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "render_attempts":done.attempts.iter().map(|(l,u,s)|serde_json::json!({"limits":resources::limits(*l),"usage":resources::usage(*u),"stop":s.map(|v|format!("{v:?}"))})).collect::<Vec<_>>(),
        "scope":"Cumulative measured parse, lower and output attempts. Compiled package/profile setup, input capture, retained host metadata, job receipt serialization and I/O excluded."
    });
    done.generated.manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    Ok(done.generated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;
    fn legacy_fixture(mixed: bool) -> Result<(Fixture, Vec<u8>)> {
        let f = Fixture::new()?;
        f.write(
            "doc/legacy.nepld",
            r#"article en "Legacy" body cons paragraph cons "{word/note}" nil nil"#,
        )?;
        f.write(
            "doc/new.nepld",
            r#"article en "New" body cons paragraph cons "new" nil nil"#,
        )?;
        f.write("doc/aliases.json", "[]")?;
        f.write("doc/new-aliases.json", "[]")?;
        let mut pages = vec![
            serde_json::json!({"id":"legacy-owner","source":"doc/legacy.nepld","projection":"doc/legacy.md","route":"legacy.html","aliases":"doc/aliases.json","renderer":host::RENDERER}),
        ];
        if mixed {
            pages.push(serde_json::json!({"id":"new-owner","source":"doc/new.nepld","projection":"doc/new.md","route":"new.html","aliases":"doc/new-aliases.json","renderer":RENDERER}));
        }
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":pages}))?;
        f.write("pages.json", &raw)?;
        Ok((f, raw))
    }
    #[test]
    fn legacy_preparation_retries_measured_parse_and_lower_from_frozen_inputs() -> Result<()> {
        for lower in [false, true] {
            let (f, raw) = legacy_fixture(false)?;
            let baseline = generate_from_registry(
                f.root(),
                "pages.json",
                raw.clone(),
                &mut crate::doc::source::budget(),
            )?;
            let measured = generate(
                f.root(),
                "pages.json",
                raw.clone(),
                RetryLimits::default(),
                |_, _, _, _| Err("unexpected baseline stop".into()),
            )?;
            let measured: serde_json::Value = serde_json::from_str(&measured.manifest)?;
            let used = measured["job_execution"]["preparation_attempts"][0]["stages"]
                [usize::from(lower)]["usage"]["work"]
                .as_u64()
                .ok_or("measured work")?;
            let short = used.checked_sub(1).ok_or("empty measured work")?;
            let mut initial = RetryLimits::default();
            if lower {
                initial.lower_limits.work = short;
            } else {
                initial.parse_limits.work = short;
            }
            let mut prompts = 0;
            let result = generate(
                f.root(),
                "pages.json",
                raw,
                initial,
                |_, phase, reason, usage| {
                    assert!(matches!(
                        phase,
                        Phase::Preparation {
                            page: 0,
                            role: Representation::Legacy
                        }
                    ));
                    assert_eq!(reason, StopReason::WorkLimit);
                    assert!(usage.work > 0);
                    prompts += 1;
                    for name in [
                        "doc/legacy.nepld",
                        "doc/new.nepld",
                        "doc/aliases.json",
                        "doc/new-aliases.json",
                        "pages.json",
                    ] {
                        std::fs::remove_file(f.root().join(name))?;
                    }
                    Ok(Some(RetryLimits::default()))
                },
            )?;
            assert_eq!(prompts, 1);
            assert_eq!(result.files, baseline.files);
            let receipt: serde_json::Value = serde_json::from_str(&result.manifest)?;
            let original: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
            for key in [
                "files",
                "input_pageset",
                "input_context",
                "page_dependencies",
            ] {
                assert_eq!(receipt[key], original[key], "{key}");
            }
            let attempts = receipt["job_execution"]["preparation_attempts"]
                .as_array()
                .ok_or("attempts")?;
            assert_eq!(attempts.len(), 2);
            assert_eq!(attempts[0]["representation"], "Legacy");
            let mut max_depth = 0u64;
            for attempt in attempts {
                for stage in attempt["stages"].as_array().ok_or("stages")? {
                    max_depth = max_depth.max(stage["usage"]["depth"].as_u64().ok_or("depth")?);
                }
            }
            assert_eq!(
                receipt["job_execution"]["prepared_usage"]["depth"].as_u64(),
                Some(max_depth)
            );
            for key in [
                "work",
                "allocation_units",
                "nodes",
                "source_bytes",
                "output_bytes",
                "events",
                "diagnostics",
                "depth",
            ] {
                let mut total = receipt["job_execution"]["prepared_usage"][key]
                    .as_u64()
                    .ok_or("prepared counter")?;
                for render in receipt["job_execution"]["render_attempts"]
                    .as_array()
                    .ok_or("renders")?
                {
                    let value = render["usage"][key].as_u64().ok_or("render counter")?;
                    total = if key == "depth" {
                        total.max(value)
                    } else {
                        total + value
                    };
                }
                assert_eq!(
                    receipt["job_execution"]["usage"][key].as_u64(),
                    Some(total),
                    "total {key}"
                );
            }

            let failed = attempts[0]["stages"]
                .as_array()
                .ok_or("stages")?
                .last()
                .ok_or("failed stage")?;
            assert_eq!(failed["stage"], if lower { "Lower" } else { "Parse" });
            assert_eq!(failed["failed"], true);
            assert_eq!(failed["stop"], "WorkLimit");
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
                for a in attempts {
                    for stage in a["stages"].as_array().ok_or("stages")? {
                        sum += stage["usage"][key].as_u64().ok_or("counter")?;
                    }
                }
                assert_eq!(
                    receipt["job_execution"]["prepared_usage"][key].as_u64(),
                    Some(sum),
                    "{key}"
                );
            }
        }
        Ok(())
    }
    #[test]
    fn mixed_completed_pageset_checkpoint_survives_legacy_pause() -> Result<()> {
        let (f, raw) = legacy_fixture(true)?;
        let baseline = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            RetryLimits::default(),
            |_, _, _, _| Err("unexpected stop".into()),
        )?;
        let receipt: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
        let stages = receipt["job_execution"]["preparation_attempts"][0]["stages"]
            .as_array()
            .ok_or("stages")?;
        let first_work = stages.iter().try_fold(0u64, |sum, s| {
            s["usage"]["work"].as_u64().map(|v| sum + v).ok_or("work")
        })?;
        let mut initial = RetryLimits::default();
        initial.job_limits.work = first_work + 1;
        let mut prompts = 0;
        let result = generate(f.root(), "pages.json", raw, initial, |_, phase, _, _| {
            assert!(matches!(
                phase,
                Phase::Preparation {
                    page: 0,
                    role: Representation::Legacy
                }
            ));
            prompts += 1;
            Ok(Some(RetryLimits::default()))
        })?;
        assert_eq!(prompts, 1);
        assert_eq!(result.files, baseline.files);
        let receipt: serde_json::Value = serde_json::from_str(&result.manifest)?;
        let attempts = receipt["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?;
        assert_eq!(attempts.len(), 4);
        assert_eq!(
            attempts
                .iter()
                .filter(|a| a["page_index"] == 0 && a["representation"] == "PageSet")
                .count(),
            1
        );
        assert_eq!(attempts[1]["representation"], "Legacy");
        assert_eq!(attempts[2]["representation"], "Legacy");
        Ok(())
    }
    #[test]
    fn legacy_preparation_cancel_and_semantic_failures_do_not_resume() -> Result<()> {
        let (f, raw) = legacy_fixture(false)?;
        let mut initial = RetryLimits::default();
        initial.parse_limits.work = 0;
        let error = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            initial,
            |_, phase, _, _| {
                assert!(matches!(
                    phase,
                    Phase::Preparation {
                        role: Representation::Legacy,
                        ..
                    }
                ));
                Ok(None)
            },
        )
        .err()
        .ok_or("cancelled job completed")?;
        assert!(error.to_string().contains("Cancelled"));
        f.write(
            "doc/aliases.json",
            r#"[{"section":null,"name":"bad alias"}]"#,
        )?;
        let error = generate(
            f.root(),
            "pages.json",
            raw,
            RetryLimits::default(),
            |_, _, _, _| Err("semantic failure prompted".into()),
        )
        .err()
        .ok_or("invalid alias accepted")?;
        assert!(
            !error.to_string().contains("semantic failure prompted"),
            "{error}"
        );
        Ok(())
    }
    fn fixture() -> Result<(Fixture, Vec<u8>)> {
        let f = Fixture::new()?;
        f.write(
            "one.nepld",
            r#"article en "One" body cons paragraph cons "One" nil nil"#,
        )?;
        let second = format!(
            "article en \"Two\" body {} nil",
            "cons paragraph cons \"{Text/note}\" nil ".repeat(20)
        );
        f.write("two.nepld", second)?;
        f.write("aliases.json", "[]")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[
            {"id":"one","source":"one.nepld","projection":"one.md","route":"one.md","aliases":"aliases.json","renderer":FOOTNOTES_RENDERER},
            {"id":"two","source":"two.nepld","projection":"two.md","route":"two.md","aliases":"aliases.json","renderer":FOOTNOTES_RENDERER}
        ]}))?;
        f.write("pages.json", &raw)?;
        Ok((f, raw))
    }
    #[test]
    fn math_page_resumes_with_its_actual_profile_and_fresh_render_admission() -> Result<()> {
        let (f, raw) = fixture()?;
        f.write(
            "two.nepld",
            format!(
                "article en \"Math\" body {} nil",
                "cons display Math frac 3 7 ".repeat(20)
            ),
        )?;
        let baseline = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            RetryLimits::default(),
            |_, _, _, _| Err("unexpected baseline stop".into()),
        )?;
        let r: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
        let first = r["job_execution"]["preparation_attempts"][0]["stages"][0]["usage"]["work"]
            .as_u64()
            .ok_or("first parse")?;
        let second = r["job_execution"]["preparation_attempts"][1]["stages"][0]["usage"]["work"]
            .as_u64()
            .ok_or("Math parse")?;
        assert!(second > first + 100);
        let mut initial = RetryLimits::default();
        initial.parse_limits.work = first + 100;
        let output = generate(f.root(), "pages.json", raw, initial, |_, phase, _, _| {
            assert!(matches!(
                phase,
                Phase::Preparation {
                    page: 1,
                    role: Representation::PageSet
                }
            ));
            Ok(Some(RetryLimits::default()))
        })?;
        assert!(
            output
                .files
                .iter()
                .any(|(p, s)| p == "two.md" && s.contains("\\frac{3}{7}"))
        );
        let r: serde_json::Value = serde_json::from_str(&output.manifest)?;
        let a = r["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?;
        assert_eq!(a.len(), 3);
        let original = a[0]["stages"][0]["profile"]
            .as_str()
            .ok_or("original profile")?;
        let resumed = a[2]["stages"][0]["profile"]
            .as_str()
            .ok_or("resumed profile")?;
        assert_ne!(original, resumed);
        assert_eq!(
            a[0]["stages"][1]["profile"]
                .as_str()
                .ok_or("original lower profile")?,
            original
        );
        assert_eq!(
            a[2]["stages"][1]["profile"]
                .as_str()
                .ok_or("resumed lower profile")?,
            resumed
        );
        assert_eq!(
            r["page_dependencies"][1]["math"]
                .as_array()
                .ok_or("Math dependency")?
                .len(),
            20
        );
        Ok(())
    }

    #[test]
    fn compact_math_page_resumes_with_its_actual_profile_and_fresh_render_admission() -> Result<()>
    {
        let (f, raw) = fixture()?;
        let raw = String::from_utf8(raw)?
            .replace(FOOTNOTES_RENDERER, COMPACT_RENDERER)
            .into_bytes();
        f.write(
            "two.nepld",
            format!(
                "article en \"Math\" body {} nil",
                "cons display Math frac 3 7 ".repeat(20)
            ),
        )?;
        let baseline = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            RetryLimits::default(),
            |_, _, _, _| Err("unexpected baseline stop".into()),
        )?;
        let r: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
        let first = r["job_execution"]["preparation_attempts"][0]["stages"][0]["usage"]["work"]
            .as_u64()
            .ok_or("first parse")?;
        let second = r["job_execution"]["preparation_attempts"][1]["stages"][0]["usage"]["work"]
            .as_u64()
            .ok_or("Math parse")?;
        assert!(second > first + 100);
        let mut initial = RetryLimits::default();
        initial.parse_limits.work = first + 100;
        let output = generate(f.root(), "pages.json", raw, initial, |_, phase, _, _| {
            assert!(matches!(
                phase,
                Phase::Preparation {
                    page: 1,
                    role: Representation::PageSet
                }
            ));
            Ok(Some(RetryLimits::default()))
        })?;
        assert!(
            output
                .files
                .iter()
                .any(|(p, s)| p == "two.md" && s.contains("\\frac{3}{7}"))
        );
        let r: serde_json::Value = serde_json::from_str(&output.manifest)?;
        let baseline_receipt: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
        for key in [
            "input_pageset",
            "input_context",
            "page_dependencies",
            "files",
        ] {
            assert_eq!(r[key], baseline_receipt[key], "retry identity {key}");
        }
        assert_eq!(output.files, baseline.files);

        let a = r["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?;
        assert_eq!(a.len(), 3);
        let original = a[0]["stages"][0]["profile"]
            .as_str()
            .ok_or("original profile")?;
        let resumed = a[2]["stages"][0]["profile"]
            .as_str()
            .ok_or("resumed profile")?;
        assert_ne!(original, resumed);
        assert_eq!(
            a[0]["stages"][1]["profile"]
                .as_str()
                .ok_or("original lower profile")?,
            original
        );
        assert_eq!(
            a[2]["stages"][1]["profile"]
                .as_str()
                .ok_or("resumed lower profile")?,
            resumed
        );
        assert_eq!(
            r["page_dependencies"][1]["math"]
                .as_array()
                .ok_or("Math dependency")?
                .len(),
            20
        );
        Ok(())
    }

    #[test]
    fn passive_bytes_are_frozen_before_first_parse_attempt() -> Result<()> {
        let f = Fixture::new()?;
        f.write("doc/page.nepld",r#"article en "Page" body cons paragraph cons sentence cons link relative "guide.md" none text "Guide" cons link relative "../design/tasks.json" none text "Data" nil nil nil"#)?;
        f.write("doc/aliases.json", "[]")?;
        f.write("doc/guide.md", "# Original\n")?;
        f.write("design/tasks.json", "{\"version\":1}")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page","source":"doc/page.nepld","projection":"doc/page.md","route":"docs/page.html","aliases":"doc/aliases.json","renderer":RENDERER
        }],"files":[{"id":"guide","source":"doc/guide.md","route":"sources/guide.md"},{"id":"data","source":"design/tasks.json","route":"sources/tasks.json"}]}))?;
        let mut limits = RetryLimits::default();
        limits.parse_limits.work = 0;
        let expected = generate(f.root(), "pages.json", raw.clone(), limits, |_, _, _, _| {
            Ok(Some(RetryLimits::default()))
        })?;
        let actual = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            limits,
            |_, phase, _, _| {
                assert!(matches!(
                    phase,
                    Phase::Preparation {
                        page: 0,
                        role: Representation::PageSet
                    }
                ));
                f.write("doc/guide.md", "# Changed\n")?;
                f.write("design/tasks.json", "{\"version\":2}")?;
                Ok(Some(RetryLimits::default()))
            },
        )?;
        let changed = generate(f.root(), "pages.json", raw, limits, |_, _, _, _| {
            Ok(Some(RetryLimits::default()))
        })?;
        let expected: serde_json::Value = serde_json::from_str(&expected.manifest)?;
        let actual: serde_json::Value = serde_json::from_str(&actual.manifest)?;
        let changed: serde_json::Value = serde_json::from_str(&changed.manifest)?;
        for field in ["input_pageset", "input_context"] {
            let before = expected
                .get(field)
                .filter(|v| !v.is_null())
                .ok_or("missing original identity")?;
            assert_eq!(
                actual
                    .get(field)
                    .filter(|v| !v.is_null())
                    .ok_or("missing frozen identity")?,
                before
            );
            assert_ne!(
                changed
                    .get(field)
                    .filter(|v| !v.is_null())
                    .ok_or("missing changed identity")?,
                before
            );
        }
        Ok(())
    }

    #[test]
    fn svg_and_alias_inputs_are_frozen_before_preparation_pause() -> Result<()> {
        let f = Fixture::new()?;
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10"/></svg>"#;
        f.write(
            "page.nepld",
            r#"article en "Picture" body cons image asset "figure" none "Image" none nil"#,
        )?;
        f.write(
            "aliases.json",
            r#"[{"section":null,"name":"original-alias"}]"#,
        )?;
        f.write("image.svg", svg)?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page","source":"page.nepld","projection":"page.md","route":"page.md","aliases":"aliases.json","renderer":FOOTNOTES_RENDERER
        }],"files":[{"id":"figure","source":"image.svg","route":"assets/image.svg"}]}))?;
        let mut limits = RetryLimits::default();
        limits.parse_limits.work = 0;
        let done = generate(f.root(), "pages.json", raw, limits, |_, phase, _, _| {
            assert!(matches!(
                phase,
                Phase::Preparation {
                    page: 0,
                    role: Representation::PageSet
                }
            ));
            for name in ["page.nepld", "aliases.json", "image.svg"] {
                f.write(name, "invalid replacement")?;
            }
            Ok(Some(RetryLimits::default()))
        })?;
        assert!(
            done.files
                .iter()
                .any(|(p, s)| p == "assets/image.svg" && s == svg)
        );
        assert!(done.files.iter().any(|(p, s)| p == "page.md"
            && s.contains("original-alias")
            && s.contains("assets/image.svg")));
        Ok(())
    }

    #[test]
    fn preparation_and_output_share_all_cumulative_counters() -> Result<()> {
        let (f, raw) = fixture()?;
        let mut initial = RetryLimits::default();
        initial.output_limits.work = 10;
        let mut prompts = 0;
        let output = generate(
            f.root(),
            "pages.json",
            raw,
            initial,
            |_, phase, reason, usage| {
                assert!(matches!(phase, Phase::Output));
                assert_eq!(reason, StopReason::WorkLimit);
                assert!(usage.source_bytes > 0);
                prompts += 1;
                Ok(Some(RetryLimits::default()))
            },
        )?;
        assert_eq!(prompts, 1);
        let r: serde_json::Value = serde_json::from_str(&output.manifest)?;
        let job = &r["job_execution"];
        let prep = job["preparation_attempts"]
            .as_array()
            .ok_or("preparation attempts")?;
        let renders = job["render_attempts"].as_array().ok_or("render attempts")?;
        assert_eq!(prep.len(), 2);
        assert_eq!(renders.len(), 2);
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
            let mut prepared = 0;
            for attempt in prep {
                for stage in attempt["stages"].as_array().ok_or("stages")? {
                    let u = stage["usage"][field].as_u64().ok_or("stage counter")?;
                    prepared = if field == "depth" {
                        prepared.max(u)
                    } else {
                        prepared + u
                    };
                }
            }
            assert_eq!(
                job["prepared_usage"][field]
                    .as_u64()
                    .ok_or("prepared counter")?,
                prepared
            );
            let mut total = prepared;
            for attempt in renders {
                let u = attempt["usage"][field].as_u64().ok_or("render counter")?;
                total = if field == "depth" {
                    total.max(u)
                } else {
                    total + u
                };
            }
            assert_eq!(job["usage"][field].as_u64().ok_or("total counter")?, total);
            let cap = job["limits"][field].as_u64().ok_or("cap")?;
            let first = renders[0]["usage"][field].as_u64().ok_or("first usage")?;
            let remaining = if field == "depth" {
                cap
            } else {
                cap - prepared - first
            };
            assert_eq!(
                renders[1]["limits"][field].as_u64().ok_or("effective")?,
                remaining
            );
        }
        for attempt in prep {
            let stages = attempt["stages"].as_array().ok_or("stages")?;
            assert_eq!(stages.len(), 2);
            let profile = stages[0]["profile"].as_str().ok_or("parse profile")?;
            assert_eq!(profile.len(), 64);
            assert_eq!(
                stages[1]["profile"].as_str().ok_or("lower profile")?,
                profile
            );
        }
        Ok(())
    }

    #[test]
    fn completed_pages_survive_parse_stop_with_frozen_later_inputs() -> Result<()> {
        let (f, raw) = fixture()?;
        let baseline = generate(
            f.root(),
            "pages.json",
            raw.clone(),
            RetryLimits::default(),
            |_, _, _, _| Err("unexpected baseline stop".into()),
        )?;
        let receipt: serde_json::Value = serde_json::from_str(&baseline.manifest)?;
        let first =
            receipt["job_execution"]["preparation_attempts"][0]["stages"][0]["usage"]["work"]
                .as_u64()
                .ok_or("first parse usage")?;
        let second =
            receipt["job_execution"]["preparation_attempts"][1]["stages"][0]["usage"]["work"]
                .as_u64()
                .ok_or("second parse usage")?;
        assert!(second > first + 100);
        let mut initial = RetryLimits::default();
        initial.parse_limits.work = first + 100;
        let mut prompts = 0;
        let actual = generate(
            f.root(),
            "pages.json",
            raw,
            initial,
            |_, phase, reason, usage| {
                assert!(matches!(
                    phase,
                    Phase::Preparation {
                        page: 1,
                        role: Representation::PageSet
                    }
                ));
                assert_eq!(reason, StopReason::WorkLimit);
                assert!(usage.work > first);
                prompts += 1;
                for p in ["one.nepld", "two.nepld", "aliases.json", "pages.json"] {
                    f.write(p, "invalid changed input")?;
                }
                Ok(Some(RetryLimits::default()))
            },
        )?;
        assert_eq!(prompts, 1);
        assert_eq!(actual.files, baseline.files);
        let receipt: serde_json::Value = serde_json::from_str(&actual.manifest)?;
        let attempts = receipt["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?;
        assert_eq!(attempts.len(), 3);
        assert_eq!(attempts.iter().filter(|a| a["page_index"] == 0).count(), 1);
        assert_eq!(attempts[1]["stages"][0]["stop"], "WorkLimit");
        assert!(receipt.get("parse_limits").is_none());
        assert!(receipt.get("lower_limits").is_none());
        let mut stage_work = 0u64;
        for attempt in attempts {
            let stages = attempt["stages"].as_array().ok_or("missing stages")?;
            assert!(!stages.is_empty());
            for stage in stages {
                stage_work += stage["usage"]["work"]
                    .as_u64()
                    .ok_or("missing stage work")?;
            }
        }
        let render_work: u64 = receipt["job_execution"]["render_attempts"]
            .as_array()
            .ok_or("renders")?
            .iter()
            .map(|a| a["usage"]["work"].as_u64().ok_or("missing render work"))
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        assert_eq!(
            receipt["job_execution"]["usage"]["work"]
                .as_u64()
                .ok_or("total")?,
            stage_work + render_work
        );
        Ok(())
    }
    #[test]
    fn lower_stop_retries_only_interrupted_page_and_skips_code_stage() -> Result<()> {
        let (f, raw) = fixture()?;
        let mut initial = RetryLimits::default();
        initial.lower_limits.work = 0;
        let mut prompts = 0;
        let output = generate(
            f.root(),
            "pages.json",
            raw,
            initial,
            |_, phase, reason, _| {
                assert!(matches!(
                    phase,
                    Phase::Preparation {
                        page: 0,
                        role: Representation::PageSet
                    }
                ));
                assert_eq!(reason, StopReason::WorkLimit);
                prompts += 1;
                Ok(Some(RetryLimits::default()))
            },
        )?;
        assert_eq!(prompts, 1);
        let receipt: serde_json::Value = serde_json::from_str(&output.manifest)?;
        let attempts = receipt["job_execution"]["preparation_attempts"]
            .as_array()
            .ok_or("attempts")?;
        assert_eq!(attempts[0]["stages"][1]["stage"], "Lower");
        assert_eq!(attempts[0]["stages"][1]["stop"], "WorkLimit");
        for attempt in attempts {
            let stages = attempt["stages"].as_array().ok_or("missing stages")?;
            assert_eq!(stages.len(), 2);
            assert!(stages.iter().all(|s| s["stage"] != "Code"));
        }
        Ok(())
    }
}
