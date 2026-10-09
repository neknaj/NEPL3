use super::*;
use nepl3_tools::doc::math::display::{
    PreparedDisplay,
    generation::Attempt,
    process::supervisor::{OwnedFinished, OwnedPoll, OwnedSession, OwnedStart, Supervisor},
};
struct Release(PathBuf);
impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "release");
    }
}
fn finish<'c>(session: &mut OwnedSession<'_, 'c>) -> Result<OwnedFinished<'c>, String> {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        match session.poll() {
            OwnedPoll::Finished(result) => return Ok(result),
            OwnedPoll::Pending { .. } if Instant::now() < until => {
                thread::sleep(Duration::from_millis(2))
            }
            _ => return Err("owned session test watchdog".into()),
        }
    }
}
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    for stop_parent in [false, true] {
        let release = Release(temp.join(format!("owned-session-release-{stop_parent}")));
        let ready = temp.join(format!("owned-session-ready-{stop_parent}"));
        let bridge = temp.join(format!("owned-session-{stop_parent}.cjs"));
        let child = "const fs=require('node:fs');setInterval(()=>{if(fs.existsSync(process.argv[1]))process.exit(0)},10);setTimeout(()=>process.exit(0),10000);";
        std::fs::write(&bridge, format!("process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0)}})}});process.stdin.resume();", serde_json::to_string(child).map_err(err)?, serde_json::to_string(&release.0.to_string_lossy()).map_err(err)?, serde_json::to_string(&ready.to_string_lossy()).map_err(err)?)).map_err(err)?;
        let mut supervisor = Supervisor::default();
        let mut b = budget();
        {
            let owner = make(Preference::KaTeXPreferred)?;
            let OwnedStart::Running(mut session) = supervisor.start_owned(
                owner,
                request_controls(),
                4096,
                Config {
                    bridge: &bridge,
                    ..cfg
                },
                &mut b,
            ) else {
                return Err("owned session start".into());
            };
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match session.poll() {
                    OwnedPoll::Pending { failure: None, .. } if ready.exists() => break,
                    OwnedPoll::Pending { failure: None, .. } if Instant::now() < until => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    _ => return Err("owned session readiness".into()),
                }
            }
            if stop_parent {
                session.cancel_parent();
            }
        }
        assert!(supervisor.has_pending_cleanup());
        let owner = make(Preference::KaTeXPreferred)?;
        let pointer = owner.mathml().syntax.value.nodes.as_ptr();
        let mut retry_budget = budget();
        let usage = retry_budget.usage();
        let mut bad = request_controls();
        bad.options.timeout_millis = 0;
        let OwnedStart::Busy(owner) = supervisor.start_owned(
            owner,
            bad,
            0,
            Config {
                node: Path::new("must-not-launch"),
                ..cfg
            },
            &mut retry_budget,
        ) else {
            return Err("occupied slot admitted source".into());
        };
        assert_eq!(owner.mathml().syntax.value.nodes.as_ptr(), pointer);
        assert_eq!(retry_budget.usage(), usage);
        assert_eq!(retry_budget.poll(), Ok(()));
        let mut stopped = budget();
        stopped.cancel();
        let stopped_source = make(Preference::KaTeXPreferred)?;
        let stopped_pointer = stopped_source.mathml().syntax.value.nodes.as_ptr();
        let OwnedStart::Busy(stopped_source) =
            supervisor.start_owned(stopped_source, bad, 0, cfg, &mut stopped)
        else {
            return Err("busy stop precedence".into());
        };
        assert_eq!(
            stopped_source.mathml().syntax.value.nodes.as_ptr(),
            stopped_pointer
        );
        assert_eq!(stopped.poll(), Err(StopReason::Cancelled));
        assert_eq!(stopped.usage(), budget().usage());
        let expected = if stop_parent {
            Failure::Budget(StopReason::Cancelled)
        } else {
            Failure::Cancelled
        };
        let pending = supervisor.poll_cleanup().ok_or("lost pending cleanup")?;
        assert!(
            matches!(pending.status, process::CleanupPoll::Pending { failure: Some(f), .. } if f == expected)
        );
        assert_eq!(
            pending.parent_stop,
            stop_parent.then_some(StopReason::Cancelled)
        );
        std::fs::write(&release.0, "release").map_err(err)?;
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            match supervisor.poll_cleanup().ok_or("cleanup missing")?.status {
                process::CleanupPoll::Pending { .. } if Instant::now() < until => {
                    thread::sleep(Duration::from_millis(2))
                }
                process::CleanupPoll::Reclaimed {
                    failure: Some(f), ..
                } if f == expected => break,
                _ => return Err("owned session reclamation".into()),
            }
        }
        // Retry exactly the unadmitted source with its unchanged operation ledger.
        let decoded = {
            let OwnedStart::Running(mut session) =
                supervisor.start_owned(owner, request_controls(), 4096, cfg, &mut retry_budget)
            else {
                return Err("owned session retry".into());
            };
            let OwnedFinished::Decoded(decoded) = finish(&mut session)? else {
                return Err("owned retry transport failure".into());
            };
            assert!(matches!(session.poll(), OwnedPoll::Consumed));
            decoded
        };
        assert!(!supervisor.has_pending_cleanup());
        assert_eq!(
            decoded.prepared().mathml().syntax.value.nodes.as_ptr(),
            pointer
        );
        let assets = fixed_assets()?;
        let native = decoded
            .into_generation(&assets, "nepl-math-owned-session", &mut retry_budget)
            .map_err(err)?;
        assert!(matches!(native.generation().attempt(), Attempt::Visual(_)));
        assert_eq!(
            native
                .generation()
                .prepared()
                .mathml()
                .syntax
                .value
                .nodes
                .as_ptr(),
            pointer
        );
    }
    for json in [true, false] {
        let mut supervisor = Supervisor::default();
        let owner = make(Preference::KaTeXPreferred)?;
        let pointer = owner.mathml().syntax.value.nodes.as_ptr();
        let bridge = temp.join("owned-session-json.cjs");
        std::fs::write(
            &bridge,
            "process.stdin.on('end',()=>process.stdout.write('{'));process.stdin.resume();",
        )
        .map_err(err)?;
        let config = if json {
            Config {
                bridge: &bridge,
                ..cfg
            }
        } else {
            cfg
        };
        let mut b = budget();
        if !json {
            let mut reference = budget();
            let request = request::prepare(&owner, request_controls(), 4096, &mut reference)
                .map_err(err)?
                .ok_or("decode budget request")?;
            let mut limits = b.limits();
            limits.work = reference.usage().work
                + request.bytes().len() as u64
                + config.output_cap as u64
                + 2;
            b = Budget::new(limits);
        }
        let decoded = {
            let OwnedStart::Running(mut session) =
                supervisor.start_owned(owner, request_controls(), 4096, config, &mut b)
            else {
                return Err("terminal session start".into());
            };
            let OwnedFinished::Decoded(decoded) = finish(&mut session)? else {
                return Err("terminal session transport failure".into());
            };
            decoded
        };
        assert!(!supervisor.has_pending_cleanup());
        assert_eq!(
            decoded.prepared().mathml().syntax.value.nodes.as_ptr(),
            pointer
        );
        assert!(matches!(
            (json, decoded.result()),
            (true, Err(process::reply::Error::Json))
                | (
                    false,
                    Err(process::reply::Error::Stopped(StopReason::WorkLimit))
                )
        ));
        if !json {
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
    }
    Ok(())
}
