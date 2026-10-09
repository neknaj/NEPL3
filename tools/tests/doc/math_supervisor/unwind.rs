//! Real native-resource retention across a caught host callback unwind. This
//! abandons generation, then starts a new attempt; it does not resume Doc state.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

struct Fixture {
    supervisor: Supervisor,
    release: PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // This runs before the enclosing Temp can remove the marker. The child
        // also has a 10s watchdog. Do not panic again while handling a failing
        // assertion; this is bounded failure-path cleanup, not an OS guarantee.
        let _ = std::fs::write(&self.release, "release");
        let until = Instant::now() + Duration::from_secs(30);
        while self.supervisor.has_pending_cleanup() && Instant::now() < until {
            let _ = self.supervisor.poll_cleanup();
            if self.supervisor.has_pending_cleanup() {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}

pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    let mut fixture = Fixture {
        supervisor: Supervisor::default(),
        release: temp.join("unwind-release"),
    };
    let release = &fixture.release;
    let ready = temp.join("unwind-ready");
    let bridge = temp.join("unwind-held-pipe.cjs");
    let child = "const fs=require('node:fs');setInterval(()=>{if(fs.existsSync(process.argv[1]))process.exit(0)},10);setTimeout(()=>process.exit(0),10000);";
    std::fs::write(&bridge, format!(
        "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{detached:true,windowsHide:true,stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0)}})}});process.stdin.resume();",
        serde_json::to_string(child).map_err(err)?,
        serde_json::to_string(&release.to_string_lossy()).map_err(err)?,
        serde_json::to_string(&ready.to_string_lossy()).map_err(err)?,
    )).map_err(err)?;
    let prepared = make(Preference::KaTeXPreferred)?;
    let assets = fixed_assets()?;
    let supervisor = &mut fixture.supervisor;
    let mut b = budget();
    let original_limits = b.limits();
    let mut ceiling = original_limits;
    ceiling.depth = 50;
    let mut observed = false;
    let caught = catch_unwind(AssertUnwindSafe(|| -> Result<(), StopReason> {
        b.with_depth_at_least(9, |b| {
            b.with_ceiling(ceiling, |b| {
                assert_eq!(b.current_depth(), 9);
                assert_eq!(b.limits(), ceiling);
                let until = Instant::now() + Duration::from_secs(5);
                let _ = prepared
                    .generate_owned(
                        Setup {
                            controls: request_controls(),
                            request_cap: 4096,
                            config: Config {
                                bridge: &bridge,
                                ..cfg
                            },
                            assets: &assets,
                            scope: "nepl-math-before-unwind",
                        },
                        |request, config, b| {
                            supervisor.run(request, config, b, &mut || {
                                if ready.exists() {
                                    observed = true;
                                    // Unwind without a panic hook; the scoped native Session
                                    // must transfer its actual resources before its request dies.
                                    resume_unwind(Box::new("host wait callback unwind"));
                                }
                                if Instant::now() >= until {
                                    return RunControl::Abort;
                                }
                                thread::sleep(Duration::from_millis(2));
                                RunControl::Continue
                            })
                        },
                        b,
                    )
                    .map_err(|GenerationError::Stopped(s)| s)?;
                Ok(())
            })
        })
    }));
    assert!(observed);
    assert!(caught.is_err());
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.limits(), original_limits);
    assert_eq!(b.poll(), Ok(()));
    assert!(b.usage().work > 0);
    assert!(b.usage().allocation_units > 0);
    assert!(b.usage().depth >= 9);
    let usage = b.usage();
    assert!(supervisor.has_pending_cleanup());
    let retained = supervisor.poll_cleanup().ok_or("lost unwind cleanup")?;
    assert_eq!(retained.parent_stop, None);
    assert!(matches!(
        retained.status,
        CleanupPoll::Pending {
            failure: Some(Failure::Cancelled),
            ..
        }
    ));
    std::fs::write(release, "release").map_err(err)?;
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        let reclaimed = supervisor.poll_cleanup().ok_or("unwind cleanup vanished")?;
        assert_eq!(reclaimed.parent_stop, None);
        match reclaimed.status {
            CleanupPoll::Pending { .. } if Instant::now() < until => {
                thread::sleep(Duration::from_millis(2))
            }
            CleanupPoll::Reclaimed {
                failure: Some(Failure::Cancelled),
                ..
            } => break,
            _ => return Err("unwind cleanup did not reclaim the cancelled attempt".into()),
        }
    }
    assert_eq!(b.usage(), usage);
    assert_eq!(b.poll(), Ok(()));
    assert!(!supervisor.has_pending_cleanup());
    // A distinct new attempt uses the surviving cumulative ledger. Completion
    // here does not relabel the abandoned attempt as a successful generation.
    let generation = make(Preference::KaTeXPreferred)?
        .generate_owned(
            Setup {
                controls: request_controls(),
                request_cap: 4096,
                config: cfg,
                assets: &assets,
                scope: "nepl-math-after-unwind",
            },
            |request, config, b| {
                supervisor.run(request, config, b, &mut || {
                    thread::sleep(Duration::from_millis(2));
                    RunControl::Continue
                })
            },
            &mut b,
        )
        .map_err(err)?;
    assert!(matches!(
        generation.attempt(),
        nepl3_tools::doc::math::display::generation::Attempt::Visual(_)
    ));
    assert_eq!(generation.termination_failure(), Some(false));
    assert!(!supervisor.has_pending_cleanup());
    assert!(b.usage().work > usage.work);
    assert_eq!(b.current_depth(), 0);
    assert_eq!(b.limits(), original_limits);
    Ok(())
}
