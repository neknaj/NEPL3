//! Test-only scheduling for lifetime-free native reclamation.
use super::*;
use nepl3_tools::doc::math::display::{
    PreparedDisplay,
    generation::{Error as GenerationError, Setup},
    process::{
        Cleanup, CleanupPoll,
        driver::{self, Driver},
    },
};
struct Release(PathBuf);
impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "release");
    }
}
fn reclaim(c: &mut Cleanup, expected: Failure) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        match c.poll() {
            CleanupPoll::Pending { .. } => {
                assert!(Instant::now() < until, "test-only reclamation watchdog");
                thread::sleep(Duration::from_millis(2));
            }
            CleanupPoll::Reclaimed { failure, .. } => {
                assert_eq!(failure, Some(expected));
                break;
            }
            CleanupPoll::Consumed => return Err("reclamation lost".into()),
        }
    }
    assert!(matches!(c.poll(), CleanupPoll::Consumed));
    Ok(())
}
fn ready(d: &mut Driver<'_>, b: &mut Budget, path: &Path) -> Result<(), ()> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match d.poll(b) {
            driver::Poll::Pending { failure: None, .. } if path.exists() => return Ok(()),
            driver::Poll::Pending { failure: None, .. } if Instant::now() < until => {
                thread::sleep(Duration::from_millis(2))
            }
            _ => return Err(()),
        }
    }
}
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    for mode in 0..4 {
        let callback = mode == 1;
        let expected = match mode {
            2 => Failure::Cancelled,
            3 => Failure::Deadline,
            _ => Failure::Budget(StopReason::Cancelled),
        };
        let release = Release(temp.join(format!("cleanup-release-{mode}")));
        let marker = temp.join(format!("cleanup-ready-{mode}"));
        let bridge = temp.join(format!("cleanup-{mode}.cjs"));
        let descendant = "const fs=require('node:fs');const t=setInterval(()=>{if(fs.existsSync(process.argv[1]))process.exit(0)},10);setTimeout(()=>process.exit(0),10000);";
        std::fs::write(&bridge, format!(
            "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{detached:true,windowsHide:true,stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0)}})}});process.stdin.resume();",
            serde_json::to_string(descendant).map_err(err)?,
            serde_json::to_string(&release.0.to_string_lossy()).map_err(err)?,
            serde_json::to_string(&marker.to_string_lossy()).map_err(err)?,
        )).map_err(err)?;
        let mut b = budget();
        // Reserved before launch; never overwritten while occupied.
        let mut slot: Option<Cleanup> = None;
        {
            let prepared = make(Preference::KaTeXPreferred)?;
            let node = cfg.node.to_path_buf();
            let modules = cfg.modules_url.to_owned();
            let selected = Config {
                node: &node,
                bridge: &bridge,
                modules_url: &modules,
                timeout: if mode == 3 {
                    Duration::from_secs(2)
                } else {
                    cfg.timeout
                },
                ..cfg
            };
            if callback {
                let assets = fixed_assets()?;
                let mut observed_ready = false;
                let result = prepared.generate_owned(
                    Setup {
                        controls: request_controls(),
                        request_cap: 4096,
                        config: selected,
                        assets: &assets,
                        scope: "cleanup-scope",
                    },
                    |req, config, b| {
                        let mut d = driver::start(req, config, b).map_err(|_| ())?;
                        let readiness = ready(&mut d, b, &marker);
                        observed_ready = readiness.is_ok();
                        b.cancel();
                        slot = Some(d.into_cleanup(b));
                        readiness?;
                        Err::<process::reply::Reply<'_>, ()>(())
                    },
                    &mut b,
                );
                assert!(
                    observed_ready,
                    "callback readiness must survive discarded error"
                );
                assert!(matches!(
                    result,
                    Err(GenerationError::Stopped(StopReason::Cancelled))
                ));
            } else {
                let request = request::prepare(&prepared, request_controls(), 4096, &mut b)
                    .map_err(err)?
                    .ok_or("request")?;
                let mut d = driver::start(&request, selected, &mut b).map_err(err)?;
                let readiness = ready(&mut d, &mut b, &marker);
                if mode == 2 {
                    d.cancel();
                } else if mode == 3 {
                    thread::sleep(Duration::from_millis(2100));
                    assert!(matches!(
                        d.poll(&mut b),
                        driver::Poll::Pending {
                            failure: Some(Failure::Deadline),
                            ..
                        }
                    ));
                } else {
                    b.cancel();
                }
                slot = Some(d.into_cleanup(&b));
                readiness.map_err(|_| "cleanup readiness")?;
            }
        }
        // Request, preparation and owned config backing storage have gone away.
        let mut cleanup = slot.ok_or("cleanup slot empty")?;
        assert_eq!(
            cleanup.parent_stop(),
            if mode < 2 {
                Some(StopReason::Cancelled)
            } else {
                None
            }
        );
        let usage = b.usage();
        for _ in 0..3 {
            assert!(matches!(
                cleanup.poll(),
                CleanupPoll::Pending {
                    failure: Some(failure),
                    ..
                } if failure == expected
            ));
        }
        std::fs::write(&release.0, "release").map_err(err)?;
        reclaim(&mut cleanup, expected)?;
        assert_eq!(b.usage(), usage);
        assert_eq!(
            b.poll(),
            if mode < 2 {
                Err(StopReason::Cancelled)
            } else {
                Ok(())
            }
        );
    }
    ordinary_failure(make, cfg, temp)?;
    consumed(make, cfg, temp)?;
    Ok(())
}

fn ordinary_failure(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    let release = Release(temp.join("cleanup-ordinary-release"));
    let bridge = temp.join("cleanup-ordinary.cjs");
    // Descendant keeps stdout open. Stderr is reported only after it exists.
    let descendant = "const fs=require('node:fs');setInterval(()=>{if(fs.existsSync(process.argv[1]))process.exit(0)},10);setTimeout(()=>process.exit(0),10000);";
    std::fs::write(&bridge, format!(
        "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{detached:true,windowsHide:true,stdio:['ignore',1,'ignore']}});c.on('spawn',()=>{{process.stderr.write('bad',()=>process.exit(0))}})}});process.stdin.resume();",
        serde_json::to_string(descendant).map_err(err)?,
        serde_json::to_string(&release.0.to_string_lossy()).map_err(err)?,
    )).map_err(err)?;
    let mut b = budget();
    let mut cleanup = {
        let prepared = make(Preference::KaTeXPreferred)?;
        let request = request::prepare(&prepared, request_controls(), 4096, &mut b)
            .map_err(err)?
            .ok_or("request")?;
        let mut d = driver::start(
            &request,
            Config {
                bridge: &bridge,
                timeout: Duration::from_secs(2),
                ..cfg
            },
            &mut b,
        )
        .map_err(err)?;
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            match d.poll(&mut b) {
                driver::Poll::Pending {
                    failure: Some(Failure::UnexpectedStderr),
                    ..
                } => break,
                driver::Poll::Pending { failure: None, .. } if Instant::now() < until => {
                    thread::sleep(Duration::from_millis(2))
                }
                _ => return Err("ordinary cause not observed".into()),
            }
        }
        b.cancel();
        d.into_cleanup(&b)
    };
    let usage = b.usage();
    assert_eq!(cleanup.parent_stop(), Some(StopReason::Cancelled));
    // Cross the old execution deadline; cleanup must not reprioritize the cause.
    thread::sleep(Duration::from_millis(2100));
    for _ in 0..3 {
        assert!(matches!(
            cleanup.poll(),
            CleanupPoll::Pending {
                failure: Some(Failure::UnexpectedStderr),
                ..
            }
        ));
    }
    std::fs::write(&release.0, "release").map_err(err)?;
    reclaim(&mut cleanup, Failure::UnexpectedStderr)?;
    assert_eq!(b.usage(), usage);
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}
fn consumed(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    temp: &Path,
) -> Result<(), String> {
    for (json, source) in [
        (false, "process.stdin.resume();"),
        (true, "process.stdin.resume();process.stdout.write('{');"),
    ] {
        let bridge = temp.join("cleanup-consumed.cjs");
        std::fs::write(&bridge, source).map_err(err)?;
        let mut b = budget();
        let mut cleanup = {
            let prepared = make(Preference::KaTeXPreferred)?;
            let request = request::prepare(&prepared, request_controls(), 4096, &mut b)
                .map_err(err)?
                .ok_or("request")?;
            let mut d = driver::start(
                &request,
                Config {
                    bridge: &bridge,
                    ..cfg
                },
                &mut b,
            )
            .map_err(err)?;
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match d.poll(&mut b) {
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
                    driver::Poll::Pending { .. } if Instant::now() < until => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    _ => return Err("terminal fixture failed".into()),
                }
            }
            d.into_cleanup(&b)
        };
        assert!(matches!(cleanup.poll(), CleanupPoll::Consumed));
        assert!(matches!(cleanup.poll(), CleanupPoll::Consumed));
    }
    Ok(())
}
