use super::*;
use crate::doc::math::{MathDisplayHost, display::Preference};
use crate::doc::source::{budget, compiled, err, with_input};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::{check::Category, lower, model::DocKind};
use nepl3_wire::foundation::FoundationCodec;
use std::{path::Path, time::Duration};

// These are deliberately synthetic policy inputs, not renderer executions.
fn owner() -> Result<PreparedDisplay, String> {
    let c = compiled()?;
    with_input(
        &c,
        "article en \"T\" body cons display Math frac 1 0 nil",
        "Article",
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let doc = lower::document(
                tree.syntax(),
                &c.doc.package.schema,
                Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let node = doc
                .value
                .nodes
                .iter()
                .position(|n| matches!(n.kind, DocKind::DisplayMath { .. }))
                .ok_or("Math")?;
            let mut host = MathDisplayHost {
                registry: profile.registry(),
                math_surface: &c.others[0].schema,
                sentence_surface: Some(&c.others[3].schema),
                doc_surface: None,
                codec: &mut codec,
            };
            host.prepare_node(&doc, node as u64, Preference::KaTeXPreferred, &mut budget())
                .map_err(err)
        },
    )
}
fn fixture(
    attempt: Attempt<'static, ()>,
    cleanup: Option<bool>,
) -> Result<OwnedGeneration<'static, 'static, ()>, String> {
    Ok(OwnedGeneration {
        prepared: owner()?,
        config: Config {
            node: Path::new("test-only-unexecuted-node"),
            bridge: Path::new("test-only-unexecuted-bridge"),
            modules_url: "file:///test-only/",
            input_cap: 1,
            output_cap: 1,
            timeout: Duration::from_secs(1),
        },
        controls: Controls {
            limits: request::RenderLimits {
                input_bytes: 64,
                output_bytes: 64,
            },
            parse_limits: request::ParseLimits {
                input_bytes: 64,
                nodes: 64,
                depth: 64,
            },
            options: request::Options {
                timeout_millis: 64,
                diagnostic_bytes: 64,
                reply_bytes: 64,
                module_bytes: 64,
            },
        },
        scope: "nepl-math-fallback".into(),
        attempt,
        observations: Some(Observations {
            diagnostics: vec![reply::Diagnostic {
                level: reply::Level::Warn,
                text: "original parse diagnostic".into(),
            }],
            diagnostic_bytes: 26,
            implementation: reply::Implementation {
                node: "test-only".into(),
                platform: "test-only".into(),
                arch: "test-only".into(),
                expected_vm_warnings: 0,
            },
            reply_bytes: 1,
        }),
        termination_failure: cleanup,
    })
}
#[test]
fn ordinary_fallback_is_explicit_and_preserves_original_owner() -> Result<(), String> {
    for parse in [false, true] {
        let attempt = || {
            Attempt::Other(if parse {
                Outcome::RenderError
            } else {
                Outcome::Unavailable(None)
            })
        };
        for explicit in [false, true] {
            let original = fixture(attempt(), Some(false))?;
            let ptr = original.prepared.mathml().syntax.value.nodes.as_ptr();
            let result = if explicit {
                original.into_composite_with_policy(CompletionPolicy::Strict, &mut budget())
            } else {
                original.into_composite(&mut budget())
            }
            .map_err(err)?;
            let Composition::Deferred(retained) = result else {
                return Err("strict changed failure".into());
            };
            assert_eq!(retained.prepared.mathml().syntax.value.nodes.as_ptr(), ptr);
        }
        let original = fixture(attempt(), Some(false))?;
        let ptr = original.prepared.mathml().syntax.value.nodes.as_ptr();
        let mut b = budget();
        let Composition::Ready(composed) = original
            .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, &mut b)
            .map_err(err)?
        else {
            return Err("fallback deferred".into());
        };
        assert_eq!(composed.math().syntax.value.nodes.as_ptr(), ptr);
        assert_eq!(composed.representation(), Representation::MathML);
        assert!(composed.generated().is_none());
        assert!(composed.stylesheet().is_empty());
        assert!(composed.assets().is_none());
        assert_eq!(
            composed.observations().ok_or("observations")?.diagnostics[0].text,
            "original parse diagnostic"
        );
        assert_eq!(composed.termination_failure(), Some(false));
        assert_eq!(b.usage().diagnostics, 1);
        if parse {
            assert!(matches!(
                composed.fallback(),
                Some(FallbackReason::RenderError)
            ));
        } else {
            assert!(matches!(
                composed.fallback(),
                Some(FallbackReason::Unavailable(None))
            ));
        }
        let parts = composed.into_import_parts();
        assert_eq!(parts.math.syntax.value.nodes.as_ptr(), ptr);
        assert!(parts.metadata.fallback.is_some());
        assert_eq!(
            parts
                .metadata
                .observations
                .ok_or("import observations")?
                .diagnostics[0]
                .text,
            "original parse diagnostic"
        );
        for cleanup in [None, Some(true)] {
            assert!(matches!(
                fixture(attempt(), cleanup)?
                    .into_composite_with_policy(
                        CompletionPolicy::OrdinaryMathmlFallback,
                        &mut budget()
                    )
                    .map_err(err)?,
                Composition::Deferred(_)
            ));
        }
    }
    for attempt in [
        Attempt::DriverFailed(()),
        Attempt::RequestRejected(request::Error::InvalidControls),
        Attempt::AssociationRejected(Mismatch::Request),
        Attempt::ValidationFailed(reply::Error::Shape),
        Attempt::Other(Outcome::InvalidRequest(None)),
    ] {
        assert!(matches!(
            fixture(attempt, Some(false))?
                .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, &mut budget())
                .map_err(err)?,
            Composition::Deferred(_)
        ));
    }
    Ok(())
}
#[test]
fn ordinary_fallback_keeps_budget_stops() -> Result<(), String> {
    let run = |b: &mut Budget| {
        fixture(Attempt::Other(Outcome::RenderError), Some(false))?
            .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, b)
            .map_err(err)
    };
    let mut measured = budget();
    assert!(matches!(run(&mut measured)?, Composition::Ready(_)));
    for reason in [
        StopReason::DiagnosticLimit,
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::OutputLimit,
    ] {
        let mut limits = measured.limits();
        let usage = measured.usage();
        let cap = match reason {
            StopReason::DiagnosticLimit => &mut limits.diagnostics,
            StopReason::WorkLimit => &mut limits.work,
            StopReason::AllocationLimit => &mut limits.allocation_units,
            StopReason::NodeLimit => &mut limits.nodes,
            _ => &mut limits.output_bytes,
        };
        *cap = match reason {
            StopReason::DiagnosticLimit => usage.diagnostics,
            StopReason::WorkLimit => usage.work,
            StopReason::AllocationLimit => usage.allocation_units,
            StopReason::NodeLimit => usage.nodes,
            _ => usage.output_bytes,
        };
        if *cap == 0 {
            continue;
        }
        let exact_limits = limits;
        let cap = match reason {
            StopReason::DiagnosticLimit => &mut limits.diagnostics,
            StopReason::WorkLimit => &mut limits.work,
            StopReason::AllocationLimit => &mut limits.allocation_units,
            StopReason::NodeLimit => &mut limits.nodes,
            _ => &mut limits.output_bytes,
        };
        *cap -= 1;
        assert!(matches!(
            run(&mut Budget::new(exact_limits))?,
            Composition::Ready(_)
        ));
        let mut short = Budget::new(limits);
        assert!(run(&mut short).is_err());
        assert_eq!(short.poll(), Err(reason));
    }
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(run(&mut cancelled).is_err());
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn explicit_mathml_constructor_rejects_other_preparations() -> Result<(), String> {
    assert!(matches!(
        owner()?.into_mathml_composite("scope", &mut budget()),
        Err(Error::Preference)
    ));
    let mut unsupported = owner()?;
    unsupported.tex = TexPreparation::Unsupported {
        node: 0,
        reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
    };
    assert!(matches!(
        unsupported.into_mathml_composite("scope", &mut budget()),
        Err(Error::Preference)
    ));
    let mut stopped = budget();
    stopped.cancel();
    assert!(matches!(
        owner()?.into_mathml_composite("scope", &mut stopped),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
