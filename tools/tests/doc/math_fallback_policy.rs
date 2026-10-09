//! Fault-injected decoded reply matrix; not evidence of real renderer faults.
use super::*;
use nepl3_tools::doc::math::{
    assets::PreparedAssets,
    display::{
        PreparedDisplay,
        generation::{
            Attempt, Setup,
            composite::{CompletionPolicy, Composition, FallbackReason, Representation},
        },
    },
};
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    assets: &PreparedAssets,
    base: &serde_json::Value,
    temp: &Path,
) -> Result<(), String> {
    let rows = [
        ("unavailable", None, true, true),
        ("unavailable", Some("vm-modules"), true, true),
        ("unavailable", Some("module-file"), true, true),
        ("render-error", Some("parse"), true, true),
        ("stopped", Some("cancelled"), false, false),
        ("stopped", Some("deadline"), false, false),
        ("stopped", Some("transport-deadline"), false, false),
        ("stopped", Some("expansion-limit"), true, false),
        ("stopped", Some("output-limit"), true, false),
        ("stopped", Some("node-limit"), true, false),
        ("stopped", Some("depth-limit"), true, false),
        ("stopped", Some("diagnostic-limit"), true, false),
        ("provider-violation", Some("module-identity"), true, false),
        ("provider-violation", Some("markup"), true, false),
    ];
    for (index, (kind, reason, observed, allowed)) in rows.into_iter().enumerate() {
        for failed_cleanup in [false, true] {
            if failed_cleanup && !allowed {
                continue;
            }
            let mut wire = base.clone();
            let fields = wire.as_object_mut().ok_or("wire")?;
            fields.remove("terminationFailure");
            if failed_cleanup {
                fields.insert("terminationFailure".into(), serde_json::json!(true));
            }
            if !observed {
                for key in [
                    "diagnostics",
                    "diagnosticBytes",
                    "implementation",
                    "replyBytes",
                ] {
                    fields.remove(key);
                }
            }
            let mut result = serde_json::json!({"kind":kind});
            if let Some(reason) = reason {
                result["reason"] = serde_json::json!(reason);
            }
            if matches!(reason, Some("module-file" | "module-identity")) {
                result["id"] = serde_json::json!("katex/dist/katex.mjs");
            }
            wire["result"] = result;
            recalculate(&mut wire)?;
            let path = temp.join(format!("fallback-{index}-{failed_cleanup}.cjs"));
            std::fs::write(
                &path,
                format!(
                    "process.stdin.resume();process.stdout.write(Buffer.from({}));",
                    serde_json::to_string(&serde_json::to_vec(&wire).map_err(err)?).map_err(err)?
                ),
            )
            .map_err(err)?;
            let selected = Config {
                bridge: &path,
                ..cfg
            };
            let prepared = make(Preference::KaTeXPreferred)?;
            let owner = prepared.mathml().syntax.value.nodes.as_ptr();
            let mut b = budget();
            let original = prepared
                .generate_owned(
                    Setup {
                        controls: request_controls(),
                        request_cap: 4096,
                        config: selected,
                        assets,
                        scope: "nepl-math-fallback-matrix",
                    },
                    owned_driver,
                    &mut b,
                )
                .map_err(err)?;
            let original_attempt = attempt_signature(original.attempt());
            let expected_kind = match kind {
                "render-error" => "parse",
                "provider-violation" => "violation",
                other => other,
            };
            let expected_reason = if kind == "render-error" {
                None
            } else {
                reason.map(str::to_owned)
            };
            let expected_module = matches!(reason, Some("module-file" | "module-identity"))
                .then(|| "katex/dist/katex.mjs".to_owned());
            assert_eq!(
                original_attempt,
                (expected_kind, expected_reason, expected_module)
            );
            let observations = observation_value(original.observations());
            let prior = b.usage().diagnostics;
            match original
                .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, &mut b)
                .map_err(err)?
            {
                Composition::Ready(composed) => {
                    assert!(allowed && !failed_cleanup, "{kind}/{reason:?}");
                    assert_eq!(composed.math().syntax.value.nodes.as_ptr(), owner);
                    assert_eq!(composed.representation(), Representation::MathML);
                    assert!(composed.assets().is_none());
                    assert!(composed.stylesheet().is_empty());
                    assert_eq!(observation_value(composed.observations()), observations);
                    assert_eq!(composed.termination_failure(), Some(false));
                    assert_eq!(b.usage().diagnostics, prior + 1);
                    match composed.fallback().ok_or("fallback")? {
                        FallbackReason::RenderError => assert_eq!(kind, "render-error"),
                        FallbackReason::Unavailable(cause) => {
                            assert_eq!(kind, "unavailable");
                            assert_eq!(cause.as_ref().map(|c| c.code()), reason);
                            if reason == Some("module-file") {
                                assert_eq!(
                                    cause.as_ref().and_then(|c| c.module()),
                                    Some("katex/dist/katex.mjs")
                                );
                            }
                        }
                    }
                }
                Composition::Deferred(original) => {
                    assert!(!allowed || failed_cleanup, "{kind}/{reason:?}");
                    assert_eq!(attempt_signature(original.attempt()), original_attempt);
                    assert_eq!(
                        original.prepared().mathml().syntax.value.nodes.as_ptr(),
                        owner
                    );
                    assert_eq!(observation_value(original.observations()), observations);
                    assert_eq!(original.termination_failure(), Some(failed_cleanup));
                    assert_eq!(b.usage().diagnostics, prior);
                }
            }
        }
    }
    for failure in [process::Failure::Cancelled, process::Failure::Deadline] {
        let original = make(Preference::KaTeXPreferred)?
            .generate_owned(
                Setup {
                    controls: request_controls(),
                    request_cap: 4096,
                    config: cfg,
                    assets,
                    scope: "nepl-math-driver-failure",
                },
                |_, _, _| Err(failure),
                &mut budget(),
            )
            .map_err(err)?;
        let Composition::Deferred(original) = original
            .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, &mut budget())
            .map_err(err)?
        else {
            return Err("driver failure fell back".into());
        };
        assert!(matches!(original.attempt(),Attempt::DriverFailed(f) if *f==failure));
        assert_eq!(original.termination_failure(), None);
    }
    Ok(())
}

fn attempt_signature(
    attempt: &Attempt<'_, String>,
) -> (&'static str, Option<String>, Option<String>) {
    use process::reply::Outcome;
    match attempt {
        Attempt::Other(Outcome::Unavailable(cause)) => (
            "unavailable",
            cause.as_ref().map(|c| c.code().into()),
            cause.as_ref().and_then(|c| c.module()).map(str::to_owned),
        ),
        Attempt::Other(Outcome::RenderError) => ("parse", None, None),
        Attempt::Other(Outcome::Stopped(cause)) => (
            "stopped",
            Some(cause.code().into()),
            cause.module().map(str::to_owned),
        ),
        Attempt::Other(Outcome::Violation(cause)) => (
            "violation",
            Some(cause.code().into()),
            cause.module().map(str::to_owned),
        ),
        _ => ("other", None, None),
    }
}
