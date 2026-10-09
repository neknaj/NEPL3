use super::*;
use nepl3_tools::doc::math::display::{
    PreparedDisplay,
    generation::{Error as GenerationError, Setup},
    process::{
        CleanupPoll, driver,
        supervisor::{RunControl, RunError, StartError, Supervisor},
    },
};
fn visual(reply: &process::reply::Reply<'_>, mode: u8, stage: &str) -> Result<(), String> {
    use process::reply::Outcome;
    let (outcome, cause) = match reply.outcome() {
        Outcome::Visual(_) => return Ok(()),
        Outcome::InvalidRequest(c) => ("invalid request", c.as_ref()),
        Outcome::Unavailable(c) => ("unavailable", c.as_ref()),
        Outcome::RenderError => ("render error", None),
        Outcome::Stopped(c) => ("stopped", Some(c)),
        Outcome::Violation(c) => ("violation", Some(c)),
    };
    let diagnostics = reply.observations().map(|v| {
        v.diagnostics
            .iter()
            .map(|d| format!("{:?}: {}", d.level, d.text))
            .collect::<Vec<_>>()
    });
    Err(format!(
        "supervisor regeneration mode={mode} stage={stage}: {outcome}; cause={:?}; module={:?}; termination_failure={}; diagnostics={diagnostics:?}",
        cause.map(|c| c.code()),
        cause.and_then(|c| c.module()),
        reply.termination_failure()
    ))
}
struct Release(PathBuf);
impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "release");
    }
}
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    for mode in 0..5 {
        check_mode(make, cfg, temp, mode)?;
    }
    #[cfg(all(panic = "unwind", not(target_family = "wasm")))]
    unwind::check(make, cfg, temp)?;
    Ok(())
}
fn check_mode(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
    mode: u8,
) -> Result<(), String> {
    let stop_parent = mode == 1 || mode == 3;
    let run = mode >= 2;
    let deadline = mode == 4;
    let release = Release(temp.join(format!("supervisor-release-{mode}")));
    let ready = temp.join(format!("supervisor-ready-{mode}"));
    let bridge = temp.join(format!("supervisor-{mode}.cjs"));
    let child = "const fs=require('node:fs');setInterval(()=>{if(fs.existsSync(process.argv[1]))process.exit(0)},10);setTimeout(()=>process.exit(0),10000);";
    std::fs::write(&bridge, format!(
        "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0)}})}});process.stdin.resume();",
        serde_json::to_string(child).map_err(err)?,
        serde_json::to_string(&release.0.to_string_lossy()).map_err(err)?,
        serde_json::to_string(&ready.to_string_lossy()).map_err(err)?,
    )).map_err(err)?;
    let mut supervisor = Supervisor::default();
    assert!(supervisor.poll_cleanup().is_none());
    let mut b = budget();
    let assets = fixed_assets()?;
    let mut observed = false;
    let result = make(Preference::KaTeXPreferred)?.generate_owned(
        Setup {
            controls: request_controls(),
            request_cap: 4096,
            config: Config {
                bridge: &bridge,
                timeout: if deadline {
                    Duration::from_secs(2)
                } else {
                    cfg.timeout
                },
                ..cfg
            },
            assets: &assets,
            scope: "scoped-supervisor",
        },
        |request, config, b| {
            if run {
                let until = Instant::now() + Duration::from_secs(10);
                let result = supervisor.run(request, config, b, &mut || {
                    if ready.exists() {
                        observed = true;
                        if !deadline {
                            return if stop_parent {
                                RunControl::CancelParent
                            } else {
                                RunControl::Abort
                            };
                        }
                    }
                    if Instant::now() >= until {
                        return RunControl::Abort;
                    }
                    thread::sleep(Duration::from_millis(2));
                    RunControl::Continue
                });
                assert!(matches!(
                    (&result, mode),
                    (Err(RunError::Aborted), 2)
                        | (Err(RunError::ParentCancelled), 3)
                        | (
                            Err(RunError::PendingCleanup {
                                failure: Some(Failure::Deadline),
                                ..
                            }),
                            4
                        )
                ));
                return result.map_err(|_| ());
            }
            let mut session = supervisor.start(request, config, b).map_err(|_| ())?;
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match session.poll() {
                    driver::Poll::Pending { failure: None, .. } if ready.exists() => {
                        observed = true;
                        break;
                    }
                    driver::Poll::Pending { failure: None, .. } if Instant::now() < until => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    _ => break,
                }
            }
            if stop_parent {
                session.cancel_parent();
            }
            // No manual handoff here. Scope exit must retain the pending process.
            Err::<process::reply::Reply<'_>, ()>(())
        },
        &mut b,
    );
    assert!(observed);
    if stop_parent {
        assert!(matches!(
            result,
            Err(GenerationError::Stopped(StopReason::Cancelled))
        ));
    } else {
        assert!(matches!(
            result.map_err(err)?.attempt(),
            nepl3_tools::doc::math::display::generation::Attempt::DriverFailed(())
        ));
    }
    let expected = if deadline {
        Failure::Deadline
    } else if stop_parent {
        Failure::Budget(StopReason::Cancelled)
    } else {
        Failure::Cancelled
    };
    let parent_stop = stop_parent.then_some(StopReason::Cancelled);
    assert!(supervisor.has_pending_cleanup());
    let usage = b.usage();
    let prepared = make(Preference::KaTeXPreferred)?;
    let mut next = budget();
    let request = request::prepare(&prepared, request_controls(), 4096, &mut next)
        .map_err(err)?
        .ok_or("request")?;
    let before = next.usage();
    assert!(matches!(
        supervisor.start(
            &request,
            Config {
                node: Path::new("must-not-start"),
                ..cfg
            },
            &mut next
        ),
        Err(StartError::CleanupPending)
    ));
    assert_eq!(next.usage(), before);
    assert!(matches!(
        supervisor.run(
            &request,
            Config {
                node: Path::new("must-not-start"),
                ..cfg
            },
            &mut next,
            &mut || {
                observed = false;
                RunControl::Abort
            }
        ),
        Err(RunError::Start(StartError::CleanupPending))
    ));
    assert!(observed);
    assert_eq!(next.usage(), before);
    let status = supervisor.poll_cleanup().ok_or("missing cleanup")?;
    assert_eq!(status.parent_stop, parent_stop);
    assert!(matches!(
        status.status,
        CleanupPoll::Pending {
            failure: Some(failure),
            ..
        } if failure == expected
    ));
    std::fs::write(&release.0, "release").map_err(err)?;
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let status = supervisor.poll_cleanup().ok_or("lost cleanup")?;
        match status.status {
            CleanupPoll::Pending { .. } if Instant::now() < until => {
                thread::sleep(Duration::from_millis(2))
            }
            CleanupPoll::Reclaimed { failure, .. } => {
                assert_eq!(failure, Some(expected));
                break;
            }
            _ => return Err("supervisor reclamation failed".into()),
        }
    }
    assert!(!supervisor.has_pending_cleanup());
    assert!(supervisor.poll_cleanup().is_none());
    assert_eq!(b.usage(), usage);
    assert_eq!(b.poll(), parent_stop.map_or(Ok(()), Err));
    // A reclaimed slot permits an actual second attempt, using its own operation ledger.
    if run {
        let reply = supervisor
            .run(&request, cfg, &mut next, &mut || {
                thread::sleep(Duration::from_millis(2));
                RunControl::Continue
            })
            .map_err(err)?;
        visual(&reply, mode, "run")?;
        assert!(!supervisor.has_pending_cleanup());
    }
    let mut session = supervisor.start(&request, cfg, &mut next).map_err(err)?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match session.poll() {
            driver::Poll::Pending { .. } if Instant::now() < until => {
                thread::sleep(Duration::from_millis(2))
            }
            driver::Poll::Finished(Ok(reply)) => {
                visual(&reply, mode, "poll")?;
                break;
            }
            _ => return Err("second supervisor attempt failed".into()),
        }
    }
    assert!(matches!(session.poll(), driver::Poll::Consumed));
    drop(session);
    assert!(!supervisor.has_pending_cleanup());
    for (json, source) in [
        (false, "process.stdin.resume();"),
        (true, "process.stdin.resume();process.stdout.write('{');"),
    ] {
        let bridge = temp.join("supervisor-terminal.cjs");
        std::fs::write(&bridge, source).map_err(err)?;
        if run {
            let result = supervisor.run(
                &request,
                Config {
                    bridge: &bridge,
                    ..cfg
                },
                &mut next,
                &mut || {
                    thread::sleep(Duration::from_millis(2));
                    RunControl::Continue
                },
            );
            assert!(matches!(
                (json, result),
                (
                    false,
                    Err(RunError::Driver(driver::DriverError::Transport(
                        Failure::EmptyReply
                    )))
                ) | (
                    true,
                    Err(RunError::Driver(driver::DriverError::Decode(
                        process::reply::Error::Json
                    )))
                )
            ));
            assert!(!supervisor.has_pending_cleanup());
        }
        let mut session = supervisor
            .start(
                &request,
                Config {
                    bridge: &bridge,
                    ..cfg
                },
                &mut next,
            )
            .map_err(err)?;
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            match session.poll() {
                driver::Poll::Pending { .. } if Instant::now() < until => {
                    thread::sleep(Duration::from_millis(2))
                }
                driver::Poll::Finished(Err(error)) => {
                    assert!(matches!(
                        (json, error),
                        (false, driver::DriverError::Transport(Failure::EmptyReply))
                            | (
                                true,
                                driver::DriverError::Decode(process::reply::Error::Json)
                            )
                    ));
                    break;
                }
                _ => return Err("supervisor terminal error fixture failed".into()),
            }
        }
        assert!(matches!(session.poll(), driver::Poll::Consumed));
        drop(session);
        assert!(!supervisor.has_pending_cleanup());
    }
    Ok(())
}

#[cfg(all(panic = "unwind", not(target_family = "wasm")))]
#[path = "math_supervisor/unwind.rs"]
mod unwind;
