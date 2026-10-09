//! Fault-injected reply markers test launch routing, not renderer qualification.
use super::*;
use nepl3_tools::doc::math::display::{PreparedDisplay, process::owned};
struct Release(PathBuf);
impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "release");
    }
}
fn finish<'c>(launch: &mut owned::Launch<'c>, b: &mut Budget) -> Result<owned::Poll<'c>, String> {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let result = launch.poll(b);
        if matches!(result, owned::Poll::Pending { .. }) {
            if Instant::now() >= until {
                return Err("owned launch test watchdog".into());
            }
            thread::sleep(Duration::from_millis(2));
        } else {
            return Ok(result);
        }
    }
}
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    fixture: &serde_json::Value,
    temp: &Path,
) -> Result<(), String> {
    for mode in 0..5 {
        let cancel_first = mode == 1;
        let releases = [
            Release(temp.join(format!("owned-a-{mode}-release"))),
            Release(temp.join(format!("owned-b-{mode}-release"))),
        ];
        let bridges = [
            temp.join(format!("owned-a-{mode}.cjs")),
            temp.join(format!("owned-b-{mode}.cjs")),
        ];
        for i in 0..2 {
            let mut wire = fixture.clone();
            let marker = format!("launch-{i}");
            wire["diagnostics"] = serde_json::json!([{"level":"warn","text":marker}]);
            wire["diagnosticBytes"] = serde_json::json!(marker.len() + 1);
            recalculate(&mut wire)?;
            std::fs::write(&bridges[i], format!(
                "process.stdin.on('end',()=>{{const t=setInterval(()=>{{if(require('node:fs').existsSync({})){{clearInterval(t);process.stdout.write({});}}}},5);const timer=setTimeout(()=>process.exit(9),10000);timer.unref();}});process.stdin.resume();",
                serde_json::to_string(&releases[i].0.to_string_lossy()).map_err(err)?,
                serde_json::to_string(&serde_json::to_string(&wire).map_err(err)?).map_err(err)?,
            )).map_err(err)?;
        }
        let owners = [
            make(Preference::KaTeXPreferred)?,
            make(Preference::KaTeXPreferred)?,
        ];
        let pointers = owners
            .each_ref()
            .map(|owner| owner.mathml().syntax.value.nodes.as_ptr().cast::<()>());
        assert_ne!(pointers[0], pointers[1]);
        let nodes = owners.each_ref().map(|owner| owner.doc_node());
        let displays = owners.each_ref().map(|owner| owner.display());
        let a = request::prepare(&owners[0], request_controls(), 4096, &mut budget())
            .map_err(err)?
            .ok_or("wire a")?;
        let b = request::prepare(&owners[1], request_controls(), 4096, &mut budget())
            .map_err(err)?
            .ok_or("wire b")?;
        assert_eq!(a.bytes(), b.bytes());
        drop((a, b));
        let configs = [
            Config {
                bridge: &bridges[0],
                ..cfg
            },
            Config {
                bridge: &bridges[1],
                ..cfg
            },
        ];
        let mut budgets = [budget(), budget()];
        let [a, b] = owners;
        let owned::Start::Running(a) =
            owned::start(a, request_controls(), 4096, configs[0], &mut budgets[0])
        else {
            return Err("start a".into());
        };
        let owned::Start::Running(b) =
            owned::start(b, request_controls(), 4096, configs[1], &mut budgets[1])
        else {
            return Err("start b".into());
        };
        // Move and interleave owners; release B first so arrival order differs.
        let mut launches = [b, a];
        if cancel_first {
            launches[1].cancel();
        }
        std::fs::write(&releases[1].0, "release").map_err(err)?;
        let owned::Poll::Complete(completed) = finish(&mut launches[0], &mut budgets[1])? else {
            return Err("completion b".into());
        };
        if mode == 2 || mode == 3 {
            let reason = if mode == 2 {
                budgets[1].cancel();
                StopReason::Cancelled
            } else {
                budgets[1]
                    .charge(
                        Resource::Work,
                        budgets[1].limits().work - budgets[1].usage().work,
                    )
                    .map_err(err)?;
                StopReason::WorkLimit
            };
            let decoded = completed.decode(&mut budgets[1]);
            assert!(
                matches!(decoded.result(), Err(process::reply::Error::Stopped(s)) if *s == reason)
            );
            assert_eq!(
                decoded
                    .prepared()
                    .mathml()
                    .syntax
                    .value
                    .nodes
                    .as_ptr()
                    .cast::<()>(),
                pointers[1]
            );
            assert_eq!(budgets[1].poll(), Err(reason));
            let assets = fixed_assets()?;
            assert!(
                matches!(decoded.into_generation(&assets, "nepl-math-owned-stopped", &mut budgets[1]), Err(nepl3_tools::doc::math::display::generation::Error::Stopped(s)) if s == reason)
            );
        } else {
            verify(
                completed.decode(&mut budgets[1]),
                pointers[1],
                nodes[1],
                displays[1],
                (configs[1], &mut budgets[1]),
                "launch-1",
            )?;
        }
        assert!(matches!(
            launches[0].poll(&mut budgets[1]),
            owned::Poll::Consumed
        ));
        if mode == 4 {
            assert!(matches!(
                launches[1].poll(&mut budgets[0]),
                owned::Poll::Pending { failure: None, .. }
            ));
            let [_b, a] = launches;
            let mut cleanup = a
                .into_cleanup(&budgets[0])
                .ok_or("pending cleanup missing")?;
            assert_eq!(cleanup.parent_stop(), None);
            let usage = budgets[0].usage();
            let until = Instant::now() + Duration::from_secs(10);
            loop {
                match cleanup.poll() {
                    process::CleanupPoll::Pending { .. } if Instant::now() < until => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    process::CleanupPoll::Reclaimed {
                        failure: Some(Failure::Cancelled),
                        ..
                    } => break,
                    _ => return Err("owned pending cleanup failed".into()),
                }
            }
            assert_eq!(budgets[0].usage(), usage);
            assert!(matches!(cleanup.poll(), process::CleanupPoll::Consumed));
            continue;
        }
        if cancel_first {
            let owned::Poll::Failed(failed) = finish(&mut launches[1], &mut budgets[0])? else {
                return Err("cancel a".into());
            };
            assert_eq!(failed.failure(), Failure::Cancelled);
            assert_eq!(
                failed
                    .prepared()
                    .mathml()
                    .syntax
                    .value
                    .nodes
                    .as_ptr()
                    .cast::<()>(),
                pointers[0]
            );
            assert_eq!(failed.config(), configs[0]);
            assert_eq!(failed.controls(), request_controls());
            let old_exit = failed.exit();
            let native = failed
                .into_generation("nepl-math-owned-failed", &mut budgets[0])
                .map_err(err)?;
            assert_eq!(native.exit(), old_exit);
            let (generation, _) = native.into_parts();
            assert!(matches!(
                generation.attempt(),
                nepl3_tools::doc::math::display::generation::Attempt::DriverFailed(
                    process::driver::DriverError::Transport(Failure::Cancelled)
                )
            ));
            assert!(matches!(
                generation.into_composite(&mut budgets[0]).map_err(err)?,
                nepl3_tools::doc::math::display::generation::composite::Composition::Deferred(_)
            ));
        } else {
            assert!(matches!(
                launches[1].poll(&mut budgets[0]),
                owned::Poll::Pending { failure: None, .. }
            ));
            std::fs::write(&releases[0].0, "release").map_err(err)?;
            let owned::Poll::Complete(completed) = finish(&mut launches[1], &mut budgets[0])?
            else {
                return Err("completion a".into());
            };
            verify(
                completed.decode(&mut budgets[0]),
                pointers[0],
                nodes[0],
                displays[0],
                (configs[0], &mut budgets[0]),
                "launch-0",
            )?;
        }
        assert!(matches!(
            launches[1].poll(&mut budgets[0]),
            owned::Poll::Consumed
        ));
        let [_b, a] = launches;
        assert!(a.into_cleanup(&budgets[0]).is_none());
    }
    classifications(make, cfg)?;
    continuation_boundaries(make, cfg, fixture, temp)?;
    Ok(())
}
fn classifications(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
) -> Result<(), String> {
    for mode in 0..8 {
        let owner = make(if mode == 0 {
            Preference::MathMLOnly
        } else {
            Preference::KaTeXPreferred
        })?;
        let pointer = owner.mathml().syntax.value.nodes.as_ptr();
        let mut controls = request_controls();
        if mode == 1 {
            controls.options.timeout_millis = 0;
        }
        let mut b = budget();
        if mode == 6 {
            let mut limits = b.limits();
            limits.allocation_units = 4096;
            b = Budget::new(limits);
        }
        if mode == 7 {
            let mut reference = budget();
            let _ = request::prepare(&owner, controls, 4096, &mut reference).map_err(err)?;
            let mut limits = b.limits();
            limits.work = reference.usage().work;
            b = Budget::new(limits);
        }
        if mode == 2 {
            b.cancel();
        }
        let config = Config {
            node: Path::new("missing-owned-node"),
            input_cap: if mode == 4 { 0 } else { cfg.input_cap },
            ..cfg
        };
        let result = owned::start(
            owner,
            controls,
            if mode == 5 { 0 } else { 4096 },
            config,
            &mut b,
        );
        match (mode, result) {
            (0, owned::Start::NotRequested(prepared))
            | (
                1,
                owned::Start::Rejected {
                    prepared,
                    error: request::Error::InvalidControls,
                },
            )
            | (
                3,
                owned::Start::Failed {
                    prepared,
                    failure: Failure::Spawn(_),
                },
            )
            | (
                4,
                owned::Start::Failed {
                    prepared,
                    failure: Failure::InputLimit,
                },
            ) => {
                assert_eq!(prepared.mathml().syntax.value.nodes.as_ptr(), pointer);
            }
            (2, owned::Start::Stopped(StopReason::Cancelled)) => {
                assert_eq!(b.poll(), Err(StopReason::Cancelled))
            }
            (5, owned::Start::Stopped(StopReason::OutputLimit)) => {
                assert_eq!(b.poll(), Err(StopReason::OutputLimit))
            }
            (6, owned::Start::Stopped(StopReason::AllocationLimit)) => {
                assert_eq!(b.poll(), Err(StopReason::AllocationLimit))
            }
            (7, owned::Start::Stopped(StopReason::WorkLimit)) => {
                assert_eq!(b.poll(), Err(StopReason::WorkLimit))
            }
            _ => return Err("owned start classification changed".into()),
        }
    }
    Ok(())
}
fn verify(
    decoded: owned::Decoded<'_>,
    pointer: *const (),
    node: u64,
    display: nepl3_markup::mathml::Display,
    config: (Config<'_>, &mut Budget),
    marker: &str,
) -> Result<(), String> {
    let (config, b) = config;
    assert_eq!(
        decoded
            .prepared()
            .mathml()
            .syntax
            .value
            .nodes
            .as_ptr()
            .cast::<()>(),
        pointer
    );
    assert_eq!(decoded.prepared().doc_node(), node);
    assert_eq!(decoded.prepared().display(), display);
    assert_eq!(decoded.config(), config);
    assert_eq!(decoded.controls(), request_controls());
    assert!(decoded.exit().success());
    let (outcome, observations, termination_failure) = decoded.result().map_err(err)?;
    assert!(matches!(outcome, process::reply::Outcome::Visual(_)));
    assert!(termination_failure); // Preserved injected field, not CLI success.
    assert_eq!(
        observations.ok_or("observations")?.diagnostics[0].text,
        marker
    );
    let assets = fixed_assets()?;
    let native = decoded
        .into_generation(&assets, "nepl-math-owned-native", b)
        .map_err(err)?;
    assert!(native.exit().success());
    let (generation, exit) = native.into_parts();
    assert!(exit.success());
    assert_eq!(
        generation
            .prepared()
            .mathml()
            .syntax
            .value
            .nodes
            .as_ptr()
            .cast::<()>(),
        pointer
    );
    assert_eq!(generation.selected_config(), config);
    assert_eq!(generation.controls(), request_controls());
    assert_eq!(generation.termination_failure(), Some(true));
    assert_eq!(
        generation
            .observations()
            .ok_or("generation observations")?
            .diagnostics[0]
            .text,
        marker
    );
    assert!(matches!(
        generation.attempt(),
        nepl3_tools::doc::math::display::generation::Attempt::Visual(_)
    ));
    let nepl3_tools::doc::math::display::generation::composite::Composition::Ready(composed) =
        generation.into_composite(b).map_err(err)?
    else {
        return Err("native composition deferred".into());
    };
    assert_eq!(composed.termination_failure(), Some(true));
    Ok(())
}

fn continuation_boundaries(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    fixture: &serde_json::Value,
    temp: &Path,
) -> Result<(), String> {
    use nepl3_tools::doc::math::display::generation::{
        Attempt, Error as GenerationError,
        composite::{CompletionPolicy, Composition},
    };
    let assets = fixed_assets()?;
    for mode in 0..5 {
        let bridge = temp.join(format!("owned-generation-boundary-{mode}.cjs"));
        let wire = if mode == 0 {
            "{".to_owned()
        } else {
            serde_json::to_string(fixture).map_err(err)?
        };
        std::fs::write(
            &bridge,
            format!(
                "process.stdin.on('end',()=>process.stdout.write({}));process.stdin.resume();",
                serde_json::to_string(&wire).map_err(err)?
            ),
        )
        .map_err(err)?;
        let owner = make(Preference::KaTeXPreferred)?;
        let pointer = owner.mathml().syntax.value.nodes.as_ptr();
        let mut b = budget();
        let config = Config {
            bridge: &bridge,
            ..cfg
        };
        let owned::Start::Running(mut launch) =
            owned::start(owner, request_controls(), 4096, config, &mut b)
        else {
            return Err("boundary launch".into());
        };
        let owned::Poll::Complete(completed) = finish(&mut launch, &mut b)? else {
            return Err("boundary completion".into());
        };
        let decoded = completed.decode(&mut b);
        let scope = if mode == 1 {
            "invalid scope"
        } else {
            "nepl-math-native-boundary"
        };
        let expected_stop = match mode {
            2 => {
                b.cancel();
                Some(StopReason::Cancelled)
            }
            3 => {
                b.charge(
                    Resource::AllocationUnits,
                    b.limits().allocation_units - b.usage().allocation_units,
                )
                .map_err(err)?;
                Some(StopReason::AllocationLimit)
            }
            4 => {
                b.charge(
                    Resource::Work,
                    b.limits().work - b.usage().work - scope.len() as u64,
                )
                .map_err(err)?;
                Some(StopReason::WorkLimit)
            }
            _ => None,
        };
        let result = decoded.into_generation(&assets, scope, &mut b);
        if let Some(expected) = expected_stop {
            assert!(matches!(result, Err(GenerationError::Stopped(s)) if s == expected));
            assert_eq!(b.poll(), Err(expected));
            continue;
        }
        let native = result.map_err(err)?;
        assert!(native.exit().success());
        let (generation, _) = native.into_parts();
        assert_eq!(
            generation.prepared().mathml().syntax.value.nodes.as_ptr(),
            pointer
        );
        assert_eq!(generation.selected_config(), config);
        if mode == 0 {
            assert!(matches!(
                generation.attempt(),
                Attempt::DriverFailed(process::driver::DriverError::Decode(
                    process::reply::Error::Json
                ))
            ));
            assert!(generation.observations().is_none());
            assert_eq!(generation.termination_failure(), None);
        } else {
            assert!(matches!(generation.attempt(), Attempt::ValidationFailed(_)));
            assert!(generation.observations().is_some());
            assert_eq!(generation.termination_failure(), Some(true));
        }
        let Composition::Deferred(generation) = generation.into_composite(&mut b).map_err(err)?
        else {
            return Err("unexpected strict fallback".into());
        };
        assert!(matches!(
            generation
                .into_composite_with_policy(CompletionPolicy::OrdinaryMathmlFallback, &mut b)
                .map_err(err)?,
            Composition::Deferred(_)
        ));
    }
    Ok(())
}
