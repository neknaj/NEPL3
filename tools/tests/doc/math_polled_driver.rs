//! Test-only scheduling watchdogs and fault-injected producers; the production
//! driver has neither sleeps nor a watchdog that abandons pending cleanup.
use super::*;
use nepl3_tools::doc::math::display::{
    PreparedDisplay,
    process::driver::{self, Driver, DriverError},
};
fn finish_driver<'a>(
    driver: &mut Driver<'a>,
    b: &mut Budget,
) -> Result<process::reply::Reply<'a>, DriverError> {
    let watchdog = Instant::now() + Duration::from_secs(20);
    loop {
        match driver.poll(b) {
            driver::Poll::Finished(result) => return result,
            driver::Poll::Consumed => return Err(DriverError::Transport(Failure::Pipe)),
            driver::Poll::Pending { .. } => {
                assert!(Instant::now() < watchdog, "test-only driver watchdog");
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
fn script(temp: &Path, name: &str, source: &str) -> Result<PathBuf, String> {
    let p = temp.join(name);
    std::fs::write(&p, source).map_err(err)?;
    Ok(p)
}
pub(super) fn check(
    make: &mut impl FnMut(Preference) -> Result<PreparedDisplay, String>,
    cfg: Config<'_>,
    fixture: &serde_json::Value,
    temp: &Path,
) -> Result<(), String> {
    let prepared = make(Preference::KaTeXPreferred)?;
    let mut b = budget();
    let request = request::prepare(&prepared, request_controls(), 4096, &mut b)
        .map_err(err)?
        .ok_or("request")?;
    let mut running = driver::start(&request, cfg, &mut b).map_err(err)?;
    let reply = finish_driver(&mut running, &mut b).map_err(err)?;
    assert!(core::ptr::eq(reply.request(), &request));
    assert_eq!(reply.config(), cfg);
    assert!(matches!(
        reply.outcome(),
        process::reply::Outcome::Visual(_)
    ));
    assert!(!reply.termination_failure());
    assert!(matches!(running.poll(&mut b), driver::Poll::Consumed));
    for (name, source, expected) in [
        (
            "driver-json.cjs",
            "process.stdin.resume();process.stdout.write('{');",
            0,
        ),
        (
            "driver-shape.cjs",
            "process.stdin.resume();process.stdout.write('{}');",
            1,
        ),
        (
            "driver-stderr.cjs",
            "process.stdin.resume();process.stderr.write('bad');",
            2,
        ),
        (
            "driver-exit.cjs",
            "process.stdin.on('end',()=>process.exit(7));process.stdin.resume();",
            3,
        ),
        ("driver-empty.cjs", "process.stdin.resume();", 4),
        (
            "driver-cap.cjs",
            "process.stdin.resume();process.stdout.write(Buffer.alloc(257));",
            5,
        ),
    ] {
        let path = script(temp, name, source)?;
        let selected = Config {
            bridge: &path,
            output_cap: 256,
            ..cfg
        };
        let mut b = budget();
        let mut running = driver::start(&request, selected, &mut b).map_err(err)?;
        let result = finish_driver(&mut running, &mut b);
        assert!(
            matches!(
                (expected, &result),
                (0, Err(DriverError::Decode(process::reply::Error::Json)))
                    | (1, Err(DriverError::Decode(process::reply::Error::Shape)))
                    | (2, Err(DriverError::Transport(Failure::UnexpectedStderr)))
                    | (3, Err(DriverError::Transport(Failure::Exit)))
                    | (4, Err(DriverError::Transport(Failure::EmptyReply)))
                    | (5, Err(DriverError::Transport(Failure::OutputLimit)))
            ),
            "{name}"
        );
        assert!(matches!(running.poll(&mut b), driver::Poll::Consumed));
    }
    let missing = temp.join("missing-node");
    assert!(matches!(
        driver::start(
            &request,
            Config {
                node: &missing,
                ..cfg
            },
            &mut budget()
        ),
        Err(DriverError::Transport(Failure::Spawn(_)))
    ));
    let path = script(
        temp,
        "driver-decode-budget.cjs",
        "process.stdin.resume();process.stdout.write('{');",
    )?;
    let mut b = budget();
    let mut running = driver::start(
        &request,
        Config {
            bridge: &path,
            output_cap: 256,
            ..cfg
        },
        &mut b,
    )
    .map_err(err)?;
    b.charge(Resource::Work, b.limits().work - b.usage().work - 1)
        .map_err(err)?;
    assert!(matches!(
        finish_driver(&mut running, &mut b),
        Err(DriverError::Decode(process::reply::Error::Stopped(
            StopReason::WorkLimit
        )))
    ));
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    assert!(matches!(running.poll(&mut b), driver::Poll::Consumed));
    for identity_bad in [false, true] {
        let mut wire = fixture.clone();
        if identity_bad {
            wire["result"]["sourceIdentity"] = serde_json::json!("00".repeat(32));
            recalculate(&mut wire)?;
        }
        let path = script(
            temp,
            "driver-injected.cjs",
            &format!(
                "process.stdin.resume();process.stdout.write(Buffer.from({}));",
                serde_json::to_string(&serde_json::to_vec(&wire).map_err(err)?).map_err(err)?
            ),
        )?;
        let mut b = budget();
        let mut running = driver::start(
            &request,
            Config {
                bridge: &path,
                ..cfg
            },
            &mut b,
        )
        .map_err(err)?;
        let result = finish_driver(&mut running, &mut b);
        if identity_bad {
            assert!(matches!(
                result,
                Err(DriverError::Decode(process::reply::Error::Source))
            ));
        } else {
            let reply = result.map_err(err)?;
            assert!(reply.termination_failure());
            assert_eq!(
                reply.observations().ok_or("observations")?.diagnostics[0].text,
                "composition preserved diagnostic"
            );
        }
        assert!(matches!(running.poll(&mut b), driver::Poll::Consumed));
    }
    // Complete bytes are not completion while the direct child remains alive.
    let ready = temp.join("driver-alive-ready");
    let ready_json = serde_json::to_string(&ready.to_string_lossy()).map_err(err)?;
    let path = script(
        temp,
        "driver-alive.cjs",
        &format!(
            "process.stdin.resume();process.stdout.write('{{}}');require('node:fs').writeFileSync({ready_json},'ready');setInterval(()=>{{}},1000);"
        ),
    )?;
    let mut b = budget();
    let mut running = driver::start(
        &request,
        Config {
            bridge: &path,
            ..cfg
        },
        &mut b,
    )
    .map_err(err)?;
    let until = Instant::now() + Duration::from_secs(5);
    while !ready.exists() {
        assert!(Instant::now() < until, "alive ready");
        assert!(matches!(
            running.poll(&mut b),
            driver::Poll::Pending { failure: None, .. }
        ));
        thread::sleep(Duration::from_millis(2));
    }
    assert!(matches!(
        running.poll(&mut b),
        driver::Poll::Pending { failure: None, .. }
    ));
    running.cancel();
    assert!(matches!(
        finish_driver(&mut running, &mut b),
        Err(DriverError::Transport(Failure::Cancelled))
    ));
    // A stopped parent budget still permits actual cleanup polling.
    let mut b = budget();
    let mut running = driver::start(
        &request,
        Config {
            bridge: &path,
            ..cfg
        },
        &mut b,
    )
    .map_err(err)?;
    b.cancel();
    assert!(matches!(
        finish_driver(&mut running, &mut b),
        Err(DriverError::Transport(Failure::Budget(
            StopReason::Cancelled
        )))
    ));
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    let mut b = budget();
    let mut running = driver::start(
        &request,
        Config {
            bridge: &path,
            timeout: Duration::from_millis(100),
            ..cfg
        },
        &mut b,
    )
    .map_err(err)?;
    assert!(matches!(
        finish_driver(&mut running, &mut b),
        Err(DriverError::Transport(Failure::Deadline))
    ));
    held_pipes(&request, cfg, temp)?;
    Ok(())
}
fn held_pipes(request: &PreparedRequest<'_>, cfg: Config<'_>, temp: &Path) -> Result<(), String> {
    for mode in 0..3 {
        let release = temp.join(format!("driver-descendant-release-{mode}"));
        let ready = temp.join(format!("driver-descendant-ready-{mode}"));
        let release_json = serde_json::to_string(&release.to_string_lossy()).map_err(err)?;
        let ready_json = serde_json::to_string(&ready.to_string_lossy()).map_err(err)?;
        let descendant = "const fs=require('node:fs');const release=process.argv[1];const t=setInterval(()=>{if(fs.existsSync(release)){clearInterval(t);process.exit(0);}},10);setTimeout(()=>process.exit(0),10000);";
        let path = script(
            temp,
            &format!("driver-descendant-{mode}.cjs"),
            &format!(
                "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0);}});}});process.stdin.resume();",
                serde_json::to_string(descendant).map_err(err)?,
                release_json,
                ready_json
            ),
        )?;
        let mut b = budget();
        let mut running = driver::start(
            request,
            Config {
                bridge: &path,
                timeout: if mode == 2 {
                    Duration::from_secs(3)
                } else {
                    cfg.timeout
                },
                ..cfg
            },
            &mut b,
        )
        .map_err(err)?;
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            match running.poll(&mut b) {
                driver::Poll::Pending { failure: None, .. } if ready.exists() => break,
                driver::Poll::Pending { failure: None, .. } if Instant::now() < until => {
                    thread::sleep(Duration::from_millis(2))
                }
                _ => {
                    std::fs::write(&release, "release").map_err(err)?;
                    running.cancel();
                    let _ = finish_driver(&mut running, &mut b);
                    return Err("descendant readiness failed".into());
                }
            }
        }
        let expected = match mode {
            0 => {
                running.cancel();
                Failure::Cancelled
            }
            1 => {
                b.cancel();
                Failure::Budget(StopReason::Cancelled)
            }
            _ => Failure::Deadline,
        };
        if mode == 2 {
            loop {
                match running.poll(&mut b) {
                    driver::Poll::Pending {
                        failure: Some(Failure::Deadline),
                        ..
                    } => break,
                    driver::Poll::Pending { failure: None, .. } if Instant::now() < until => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    _ => {
                        std::fs::write(&release, "release").map_err(err)?;
                        running.cancel();
                        let _ = finish_driver(&mut running, &mut b);
                        return Err("descendant deadline not retained".into());
                    }
                }
            }
        }
        for _ in 0..3 {
            if !matches!(running.poll(&mut b), driver::Poll::Pending { failure: Some(f), .. } if f == expected)
            {
                std::fs::write(&release, "release").map_err(err)?;
                running.cancel();
                let _ = finish_driver(&mut running, &mut b);
                return Err("pending cleanup was lost".into());
            }
            thread::sleep(Duration::from_millis(2));
        }
        std::fs::write(&release, "release").map_err(err)?;
        assert!(
            matches!(finish_driver(&mut running, &mut b), Err(DriverError::Transport(f)) if f == expected)
        );
        if mode == 1 {
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
        }
        assert!(matches!(running.poll(&mut b), driver::Poll::Consumed));
    }
    Ok(())
}
