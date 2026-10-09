use super::math::request_controls;
use super::*;
use nepl3_tools::doc::math::display::{
    Preference, TexPreparation,
    process::{self, Completed, Config, Failure, Poll, Process},
    request::{self, PreparedRequest},
};
use std::{
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn finish<'a>(p: &mut Process<'a>, b: &mut Budget) -> Result<Completed<'a>, Failure> {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match p.poll(b) {
            Poll::Complete(c) => return Ok(c),
            Poll::Failed(f) => return Err(f),
            Poll::Consumed => return Err(Failure::Pipe),
            Poll::Pending { .. } => {
                assert!(Instant::now() < until, "cleanup watchdog");
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
// Readiness is a fixture handshake, not a new process deadline. Poll first,
// including when the file exists, so cancellation cannot hide an earlier failure.
fn await_descendant_ready(
    p: &mut Process<'_>,
    ready: &Path,
    release: &Path,
    b: &mut Budget,
) -> Result<(), String> {
    let watchdog = Instant::now() + Duration::from_secs(20);
    loop {
        match p.poll(b) {
            Poll::Pending {
                failure: None,
                termination_error: None,
                ..
            } => {
                if ready.exists() {
                    return Ok(());
                }
                if Instant::now() >= watchdog {
                    eprintln!("descendant startup test watchdog; releasing before cleanup");
                    let release_error = std::fs::write(release, "release").err();
                    p.cancel();
                    let cleanup = finish(p, b).err();
                    return Err(format!(
                        "descendant startup test watchdog; release={release_error:?}; cleanup={cleanup:?}"
                    ));
                }
            }
            Poll::Pending {
                failure,
                termination_error,
                ..
            } => {
                eprintln!(
                    "descendant startup failure={failure:?}, termination_error={termination_error:?}; observing cleanup"
                );
                let release_error = std::fs::write(release, "release").err();
                let cleanup = finish(p, b).err();
                return Err(format!(
                    "descendant startup failure={failure:?}; termination_error={termination_error:?}; release={release_error:?}; cleanup={cleanup:?}"
                ));
            }
            Poll::Failed(failure) => {
                let release_error = std::fs::write(release, "release").err();
                return Err(format!(
                    "descendant startup terminal failure={failure:?}; release={release_error:?}"
                ));
            }
            Poll::Complete(completed) => {
                return Err(format!(
                    "descendant startup completed before held-pipe observation: {:?}",
                    completed.exit()
                ));
            }
            Poll::Consumed => return Err("descendant startup process already consumed".into()),
        }
        thread::sleep(Duration::from_millis(2));
    }
}
fn config<'a>(node: &'a str, path: &'a Path, cap: usize, timeout: u64) -> Config<'a> {
    Config {
        node: Path::new(node),
        bridge: path,
        modules_url: "file:///",
        input_cap: 4096,
        output_cap: cap,
        timeout: Duration::from_millis(timeout),
    }
}
pub fn check(request: &PreparedRequest<'_>, node: &str) -> Result<(), String> {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let temp = Temp(std::env::temp_dir().join(format!(
        "nepl3-math-process-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&temp.0).map_err(err)?;
    for (name, source, expected) in [
        (
            "exact",
            "process.stdin.resume();process.stdout.write('a'.repeat(256));",
            None,
        ),
        (
            "overflow",
            "process.stdin.resume();process.stdout.write('a'.repeat(257));",
            Some(Failure::OutputLimit),
        ),
        (
            "stderr",
            "process.stdin.resume();process.stderr.write('x');",
            Some(Failure::UnexpectedStderr),
        ),
        (
            "exit",
            "process.stdin.resume();process.stdout.write('x');process.exitCode=7;",
            Some(Failure::Exit),
        ),
    ] {
        let path = temp.0.join(format!("{name}.cjs"));
        std::fs::write(&path, source).map_err(err)?;
        let mut b = budget();
        let mut p = process::start(request, config(node, &path, 256, 5000), &mut b).map_err(err)?;
        match (finish(&mut p, &mut b), expected) {
            (Ok(c), None) => {
                assert_eq!(c.bytes(), vec![b'a'; 256]);
                assert!(core::ptr::eq(c.owner(), request.owner()));
                assert!(core::ptr::eq(c.request(), request));
                assert_eq!(c.request().controls(), request.controls());
                assert_eq!(c.config(), config(node, &path, 256, 5000));

                assert!(c.exit().success());
            }
            (Err(actual), Some(want)) => assert_eq!(actual, want),
            _ => return Err(format!("unexpected {name} result")),
        }
        assert!(matches!(p.poll(&mut b), Poll::Consumed));
    }
    let path = temp.0.join("waiting.cjs");
    std::fs::write(&path,"process.stdin.resume();process.stdout.end('looks complete');process.stderr.end();setInterval(()=>{},1000);").map_err(err)?;
    for cancelled in [false, true] {
        let output_ready = temp.0.join(format!("output-ready-{cancelled}"));
        let source = format!(
            "process.stdin.resume();process.stdout.end('looks complete',()=>require('node:fs').writeFileSync({},'ready'));process.stderr.end();setInterval(()=>{{}},1000);",
            serde_json::to_string(&output_ready.to_string_lossy()).map_err(err)?
        );
        std::fs::write(&path, source).map_err(err)?;
        let mut b = budget();
        let mut p = process::start(
            request,
            config(node, &path, 256, if cancelled { 5000 } else { 100 }),
            &mut b,
        )
        .map_err(err)?;
        if cancelled {
            let until = Instant::now() + Duration::from_secs(3);
            while !output_ready.exists() {
                assert!(Instant::now() < until, "output readiness");
                thread::sleep(Duration::from_millis(2));
            }
            assert!(matches!(
                p.poll(&mut b),
                Poll::Pending { failure: None, .. }
            ));
            p.cancel();
        }
        let expected = if cancelled {
            Failure::Cancelled
        } else {
            Failure::Deadline
        };
        assert!(matches!(finish(&mut p,&mut b),Err(f) if f==expected));
    }
    // A stopped Budget must still permit cleanup; no early poll()? escape.
    let mut b = budget();
    let mut p = process::start(request, config(node, &path, 256, 5000), &mut b).map_err(err)?;
    b.cancel();
    assert!(matches!(
        finish(&mut p, &mut b),
        Err(Failure::Budget(StopReason::Cancelled))
    ));
    // A descendant retains inherited pipes until an explicit test release.
    // Direct-child cancellation must not block poll or claim cleanup complete.
    let release = temp.0.join("release");
    let ready = temp.0.join("ready");
    let release_json = serde_json::to_string(&release.to_string_lossy()).map_err(err)?;
    let ready_json = serde_json::to_string(&ready.to_string_lossy()).map_err(err)?;
    let descendant = "const fs=require('node:fs'); const release=process.argv[1]; const timer=setInterval(()=>{if(fs.existsSync(release)){clearInterval(timer);process.exit(0);}},10);setTimeout(()=>process.exit(0),10000);";
    let source = format!(
        "process.stdin.on('end',()=>{{const c=require('node:child_process').spawn(process.execPath,['-e',{},{}],{{stdio:['ignore',1,2]}});c.on('spawn',()=>{{require('node:fs').writeFileSync({},'ready');process.exit(0);}});}});process.stdin.resume();",
        serde_json::to_string(descendant).map_err(err)?,
        release_json,
        ready_json
    );
    let path = temp.0.join("descendant.cjs");
    std::fs::write(&path, source).map_err(err)?;
    let mut b = budget();
    let mut p = process::start(request, config(node, &path, 256, 5000), &mut b).map_err(err)?;
    await_descendant_ready(&mut p, &ready, &release, &mut b)?;
    p.cancel();
    for _ in 0..3 {
        assert!(matches!(
            p.poll(&mut b),
            Poll::Pending {
                failure: Some(Failure::Cancelled),
                ..
            }
        ));
        thread::sleep(Duration::from_millis(2));
    }
    std::fs::write(&release, "release").map_err(err)?;
    assert!(matches!(finish(&mut p, &mut b), Err(Failure::Cancelled)));
    // The helper retains the native deadline and reports early terminal failure;
    // no ready marker is sufficient to turn either into success.
    for (name, source, timeout, expected) in [
        (
            "never-ready",
            "process.stdin.resume();setInterval(()=>{},1000);",
            1000,
            "Deadline",
        ),
        (
            "early-exit",
            "process.stdin.on('end',()=>process.exit(0));process.stdin.resume();",
            5000,
            "EmptyReply",
        ),
        (
            "canceled-ready",
            "process.stdin.resume();setInterval(()=>{},1000);",
            5000,
            "Budget(Cancelled)",
        ),
    ] {
        let path = temp.0.join(format!("{name}.cjs"));
        let ready = temp.0.join(format!("{name}-ready"));
        let release = temp.0.join(format!("{name}-release"));
        std::fs::write(&path, source).map_err(err)?;
        let mut b = budget();
        let mut p =
            process::start(request, config(node, &path, 256, timeout), &mut b).map_err(err)?;
        if name == "canceled-ready" {
            std::fs::write(&ready, "stale ready").map_err(err)?;
            b.cancel();
        }
        let Err(failure) = await_descendant_ready(&mut p, &ready, &release, &mut b) else {
            return Err("fixture startup must fail".into());
        };
        assert!(failure.contains(expected), "{failure}");
        assert!(matches!(p.poll(&mut b), Poll::Consumed));
    }
    // Real fixed renderer, using a URL created by Node rather than guessing OS URLs.
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = tools.join("audit/math/node_modules");
    let url = std::process::Command::new(node)
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(&root)
        .output()
        .map_err(err)?;
    assert!(url.status.success());
    let url = String::from_utf8(url.stdout).map_err(err)?;
    let script = tools.join("math/katex/node/stdio.mjs");
    let mut b = budget();
    let mut p = process::start(
        request,
        Config {
            modules_url: url.trim(),
            ..config(node, &script, 1000000, 10000)
        },
        &mut b,
    )
    .map_err(err)?;
    let c = finish(&mut p, &mut b).map_err(err)?;
    assert!(core::ptr::eq(c.request(), request));
    assert_eq!(c.config().modules_url, url.trim());
    assert_eq!(c.config().output_cap, 1000000);

    let value: serde_json::Value = serde_json::from_slice(c.bytes()).map_err(err)?;
    assert_eq!(value["result"]["kind"], "visual-parsed-unchecked");
    assert_eq!(value["version"], 1);
    let decoded = process::reply::decode(c, &mut b).map_err(err)?;
    assert!(matches!(
        decoded.outcome(),
        process::reply::Outcome::Visual(_)
    ));
    assert!(core::ptr::eq(decoded.request(), request));
    assert!(decoded.observations().is_some());
    let assets = fixed_assets()?;
    let before = b.usage().output_bytes;
    let process::reply::visual::Preparation::Visual(associated) = decoded
        .prepare_visual(&assets, "nepl-math-associated", &mut b)
        .map_err(err)?
    else {
        return Err("missing visual".into());
    };
    assert_eq!(b.usage().output_bytes, before);
    assert!(core::ptr::eq(associated.request(), request));
    assert!(core::ptr::eq(associated.visual().assets(), &assets));
    let rendered = associated.serialize(&mut b).map_err(err)?;
    assert!(core::ptr::eq(rendered.request(), request));
    assert!(core::ptr::eq(rendered.visual().assets(), &assets));
    assert_eq!(rendered.config(), associated.config());
    assert!(rendered.observations().is_some());
    let after = b.usage().output_bytes;
    let again = associated.serialize(&mut b).map_err(err)?;
    assert_eq!(
        again.visual().visual().html(),
        rendered.visual().visual().html()
    );
    assert_eq!(
        again.visual().visual().stylesheet(),
        rendered.visual().visual().stylesheet()
    );
    assert_eq!(b.usage().output_bytes - after, after - before);
    let expected_css = again.visual().visual().stylesheet().to_owned();
    let before_projection = b.usage().output_bytes;
    let projected = associated.into_html(&mut b).map_err(err)?;
    assert!(core::ptr::eq(projected.request(), request));
    assert!(core::ptr::eq(projected.visual().assets(), &assets));
    assert_eq!(projected.config().modules_url, url.trim());
    assert_eq!(projected.config().output_cap, 1000000);
    assert!(projected.observations().is_some());
    assert_eq!(projected.visual().visual().stylesheet(), expected_css);
    assert_eq!(
        b.usage().output_bytes - before_projection,
        expected_css.len() as u64
    );
    let markup = projected.visual().visual().request();
    let checked =
        nepl3_markup::html::validate(&markup.fragment, markup.slot, &markup.policy, &mut b)
            .map_err(err)?;
    let typed = nepl3_markup::html::serialize(&checked, &mut b).map_err(err)?;
    assert!(typed.starts_with("<span aria-hidden=\"true\" class=\"nepl-math-associated\">"));
    assert!(!typed.contains(" style="));
    check_association(request, node, &temp.0, &value, &assets)?;

    check_replies(request, node, &temp.0, &value)?;
    Ok(())
}

fn recalculate(value: &mut serde_json::Value) -> Result<(), String> {
    if value.get("diagnostics").is_some() {
        let inner = serde_json::json!({"result":value["result"],"diagnostics":value["diagnostics"],"diagnosticBytes":value["diagnosticBytes"],"implementation":value["implementation"]});
        value["replyBytes"] = serde_json::json!(serde_json::to_vec(&inner).map_err(err)?.len());
    }
    Ok(())
}
fn payload(
    request: &PreparedRequest<'_>,
    node: &str,
    root: &Path,
    name: &str,
    bytes: &[u8],
    valid: bool,
) -> Result<(), String> {
    let script = root.join("reply.cjs");
    std::fs::write(
        &script,
        format!(
            "process.stdin.resume();process.stdout.write(Buffer.from({}));",
            serde_json::to_string(bytes).map_err(err)?
        ),
    )
    .map_err(err)?;
    let mut b = budget();
    let mut p = process::start(
        request,
        config(node, &script, bytes.len().max(256), 5000),
        &mut b,
    )
    .map_err(err)?;
    let completed = finish(&mut p, &mut b).map_err(err)?;
    let output = b.usage().output_bytes;
    let sources = b.usage().source_bytes;
    let result = process::reply::decode(completed, &mut b);
    assert_eq!(result.is_ok(), valid, "{name}");
    if let Ok(reply) = &result {
        let expected: serde_json::Value = serde_json::from_slice(bytes).map_err(err)?;
        assert_eq!(
            reply.observations().is_some(),
            expected.get("diagnostics").is_some()
        );
        match (expected["result"]["kind"].as_str(), reply.outcome()) {
            (Some("stopped"), process::reply::Outcome::Stopped(cause))
            | (Some("provider-violation"), process::reply::Outcome::Violation(cause)) => {
                assert_eq!(Some(cause.code()), expected["result"]["reason"].as_str())
            }
            (Some("invalid-request"), process::reply::Outcome::InvalidRequest(_))
            | (Some("unavailable"), process::reply::Outcome::Unavailable(_))
            | (Some("render-error"), process::reply::Outcome::RenderError)
            | (Some("visual-parsed-unchecked"), process::reply::Outcome::Visual(_)) => {}
            _ => return Err(format!("wrong typed result {name}")),
        }
    }
    if name == "invalid-json" {
        assert!(matches!(result, Err(process::reply::Error::Json)));
    }

    assert_eq!(b.usage().output_bytes, output);
    assert_eq!(b.usage().source_bytes, sources);
    Ok(())
}
fn check_replies(
    request: &PreparedRequest<'_>,
    node: &str,
    root: &Path,
    success: &serde_json::Value,
) -> Result<(), String> {
    use serde_json::json;
    for (kind, reasons) in [
        (
            "invalid-request",
            vec![
                "transport-config",
                "transport-json",
                "transport-canonical",
                "transport-schema",
            ],
        ),
        (
            "stopped",
            vec![
                "transport-input-limit",
                "transport-output-limit",
                "transport-deadline",
                "reply-limit",
                "deadline",
                "cancelled",
                "input-limit",
            ],
        ),
        (
            "provider-violation",
            vec![
                "transport-input",
                "transport-exception",
                "worker-start",
                "worker",
                "message",
                "stdio",
                "worker-exit",
                "serialization",
            ],
        ),
    ] {
        for reason in reasons {
            payload(
                request,
                node,
                root,
                reason,
                &serde_json::to_vec(&json!({"version":1,"result":{"kind":kind,"reason":reason}}))
                    .map_err(err)?,
                true,
            )?;
        }
    }
    payload(
        request,
        node,
        root,
        "bare-invalid",
        br#"{"version":1,"result":{"kind":"invalid-request"}}"#,
        true,
    )?;
    for (kind, reasons) in [
        ("unavailable", vec!["vm-modules"]),
        ("render-error", vec!["parse"]),
        (
            "stopped",
            vec![
                "input-limit",
                "output-limit",
                "expansion-limit",
                "module-limit",
                "node-limit",
                "depth-limit",
                "diagnostic-limit",
            ],
        ),
        (
            "provider-violation",
            vec![
                "renderer",
                "output",
                "exception",
                "module-read",
                "unissued-module-bundle",
                "module-graph",
                "module-execution",
                "parser-export",
                "markup",
                "parser",
                "runtime-warning",
            ],
        ),
    ] {
        for reason in reasons {
            let mut value = success.clone();
            value["result"] = json!({"kind":kind,"reason":reason});
            recalculate(&mut value)?;
            payload(
                request,
                node,
                root,
                reason,
                &serde_json::to_vec(&value).map_err(err)?,
                true,
            )?;
        }
    }
    for kind in ["unavailable", "invalid-request"] {
        let mut value = success.clone();
        value["result"] = json!({"kind":kind});
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            kind,
            &serde_json::to_vec(&value).map_err(err)?,
            true,
        )?;
    }
    for (kind, reason) in [
        ("unavailable", "module-file"),
        ("provider-violation", "module-size"),
        ("provider-violation", "module-identity"),
        ("provider-violation", "module-encoding"),
    ] {
        let mut value = success.clone();
        value["result"] = json!({"kind":kind,"reason":reason,"id":"katex/dist/katex.mjs"});
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            reason,
            &serde_json::to_vec(&value).map_err(err)?,
            true,
        )?;
        value["result"]["id"] = json!("unrequested/module.js");
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            "wrong-module-id",
            &serde_json::to_vec(&value).map_err(err)?,
            false,
        )?;
    }
    for (pointer, replacement) in [
        ("/version", json!(2)),
        ("/result/sourceIdentity", json!("wrong")),
        ("/result/sourceBytes", json!(1)),
        ("/result/moduleCount", json!(21)),
        ("/result/loaderPolicy", json!("wrong")),
        ("/result/version", json!("0.0.0")),
        ("/result/inputBytes", json!(0)),
        ("/result/outputBytes", json!(0)),
        ("/diagnosticBytes", json!(1)),
        ("/implementation/expectedVmWarnings", json!(2)),
        ("/result/visual/nodes", json!([])),
    ] {
        let mut value = success.clone();
        *value.pointer_mut(pointer).ok_or("missing test field")? = replacement;
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            pointer,
            &serde_json::to_vec(&value).map_err(err)?,
            false,
        )?;
    }
    for field in [
        "diagnostics",
        "diagnosticBytes",
        "implementation",
        "replyBytes",
    ] {
        let mut value = success.clone();
        value.as_object_mut().ok_or("object")?.remove(field);
        payload(
            request,
            node,
            root,
            "partial-observation",
            &serde_json::to_vec(&value).map_err(err)?,
            false,
        )?;
    }
    let mut value = success.clone();
    value["unexpected"] = json!(0);
    payload(
        request,
        node,
        root,
        "outer-extra",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    let mut value = success.clone();
    value["result"]["unexpected"] = json!(0);
    recalculate(&mut value)?;
    payload(
        request,
        node,
        root,
        "result-extra",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    let mut value = success.clone();
    value["terminationFailure"] = json!(true);
    payload(
        request,
        node,
        root,
        "termination-observation",
        &serde_json::to_vec(&value).map_err(err)?,
        true,
    )?;
    value["terminationFailure"] = json!(false);
    payload(
        request,
        node,
        root,
        "false-termination-field",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    let mut value = success.clone();
    value["replyBytes"] = json!(serde_json::to_vec(success).map_err(err)?.len());
    payload(
        request,
        node,
        root,
        "outer-length-not-inner",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    for bytes in [
        vec![255],
        br#"{"version":1,"version":1,"result":{"kind":"invalid-request"}}"#.to_vec(),
        br#"{"version":1,"result":{"kind":"invalid-request"}} {}"#.to_vec(),
        br#"{"version":1,"result":{"kind":"invalid-request","reason":"\ud800"}}"#.to_vec(),
    ] {
        payload(request, node, root, "invalid-json", &bytes, false)?;
    }
    let mut controls = request.controls();
    controls.options.reply_bytes = 0;
    let tiny = nepl3_tools::doc::math::display::request::prepare(
        request.owner(),
        controls,
        4096,
        &mut budget(),
    )
    .map_err(err)?
    .ok_or("request")?;
    payload(
        &tiny,
        node,
        root,
        "zero-reply-control",
        br#"{"version":1,"result":{"kind":"stopped","reason":"reply-limit"}}"#,
        true,
    )?;
    payload(
        &tiny,
        node,
        root,
        "zero-reply-success",
        &serde_json::to_vec(success).map_err(err)?,
        false,
    )?;
    extra_reply_checks(request, node, root, success)?;
    Ok(())
}

fn extra_reply_checks(
    request: &PreparedRequest<'_>,
    node: &str,
    root: &Path,
    success: &serde_json::Value,
) -> Result<(), String> {
    use serde_json::json;
    let nodes = success["result"]["visual"]["nodes"]
        .as_array()
        .ok_or("nodes")?;
    let mut heights: Vec<u64> = Vec::new();
    for value in nodes {
        let mut d = 1;
        if let Some(children) = value.get("children").and_then(|v| v.as_array()) {
            for child in children {
                let i = child.as_u64().ok_or("child")? as usize;
                d = d.max(heights[i] + 1);
            }
        }
        heights.push(d);
    }
    let mut exact = request.controls();
    exact.limits.input_bytes = success["result"]["inputBytes"].as_u64().ok_or("input")?;
    exact.limits.output_bytes = success["result"]["outputBytes"].as_u64().ok_or("output")?;
    exact.parse_limits.input_bytes = exact.limits.output_bytes;
    exact.parse_limits.nodes = nodes.len() as u64;
    exact.parse_limits.depth = *heights.last().ok_or("depth")?;
    exact.options.module_bytes = success["result"]["sourceBytes"].as_u64().ok_or("source")?;
    exact.options.reply_bytes = success["replyBytes"].as_u64().ok_or("reply")?;
    let encoded = serde_json::to_vec(success).map_err(err)?;
    for i in 0..8 {
        let mut c = exact;
        match i {
            0 => {}
            1 => c.limits.input_bytes -= 1,
            2 => c.limits.output_bytes -= 1,
            3 => c.parse_limits.input_bytes -= 1,
            4 => c.parse_limits.nodes -= 1,
            5 => c.parse_limits.depth -= 1,
            6 => c.options.module_bytes -= 1,
            _ => c.options.reply_bytes -= 1,
        }
        let prepared = nepl3_tools::doc::math::display::request::prepare(
            request.owner(),
            c,
            4096,
            &mut budget(),
        )
        .map_err(err)?
        .ok_or("request")?;
        payload(&prepared, node, root, "retained-cap", &encoded, i == 0)?;
    }
    for case in 0..5 {
        let mut value = success.clone();
        let array = value["result"]["visual"]["nodes"]
            .as_array_mut()
            .ok_or("nodes")?;
        let last = array.len() - 1;
        match case {
            0 => array[last]["children"] = json!([last]),
            1 => {
                let i = array
                    .iter()
                    .position(|v| v.get("children").is_some())
                    .ok_or("branch")?;
                array[i]["children"] = json!([last]);
            }
            2 => {
                let child = array[last]["children"][0].clone();
                array[last]["children"] = json!([child, child]);
            }
            3 => array.push(
                json!({"kind":"span","classes":"","style":"","aria_hidden":null,"children":[]}),
            ),
            _ => *array = vec![json!({"kind":"text","text":"x"})],
        }
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            "malformed-graph",
            &serde_json::to_vec(&value).map_err(err)?,
            false,
        )?;
    }
    for raw in [
        br#"{"version":1,"result":{"kind":"invalid-request","kind":"invalid-request"}}"#.as_slice(),
        br#"{"version":1,"result":{"kind":"invalid-request","\u006bind":"invalid-request"}}"#
            .as_slice(),
    ] {
        payload(request, node, root, "invalid-json", raw, false)?;
    }
    for counter in [json!(1.5), json!(-1), json!(9007199254740992u64)] {
        let mut value = success.clone();
        value["result"]["inputBytes"] = counter;
        recalculate(&mut value)?;
        payload(
            request,
            node,
            root,
            "non-safe-counter",
            &serde_json::to_vec(&value).map_err(err)?,
            false,
        )?;
    }
    let mut value = success.clone();
    value["result"] = json!({"kind":"stopped","reason":"transport-deadline"});
    recalculate(&mut value)?;
    payload(
        request,
        node,
        root,
        "observed-transport-control",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    let mut value = success.clone();
    for key in [
        "diagnostics",
        "diagnosticBytes",
        "implementation",
        "replyBytes",
    ] {
        value.as_object_mut().ok_or("object")?.remove(key);
    }
    payload(
        request,
        node,
        root,
        "unobserved-visual",
        &serde_json::to_vec(&value).map_err(err)?,
        false,
    )?;
    // Node itself supplies canonical escaped-string lengths, not serde on both sides.
    let mut unicode = success.clone();
    let message = "日本😀\"\\\n\t\u{0001}\u{2028}\u{2029}";
    unicode["diagnostics"] = json!([{"level":"warn","text":message}]);
    unicode["diagnosticBytes"] = json!(message.len() + 1);
    let script = root.join("unicode-reply.cjs");
    let js = format!(
        "process.stdin.resume();const x={}; const inner={{result:x.result,diagnostics:x.diagnostics,diagnosticBytes:x.diagnosticBytes,implementation:x.implementation}};x.replyBytes=Buffer.byteLength(JSON.stringify(inner));process.stdout.write(JSON.stringify(x));",
        serde_json::to_string(&unicode).map_err(err)?
    );
    std::fs::write(&script, js).map_err(err)?;
    let mut b = budget();
    let mut p =
        process::start(request, config(node, &script, 100000, 5000), &mut b).map_err(err)?;
    let completed = finish(&mut p, &mut b).map_err(err)?;
    let reply = process::reply::decode(completed, &mut b).map_err(err)?;
    assert_eq!(
        reply.observations().ok_or("observations")?.diagnostics[0].text,
        message
    );
    recalculate(&mut unicode)?;
    let mut c = request.controls();
    c.options.diagnostic_bytes = message.len() as u64;
    let limited =
        nepl3_tools::doc::math::display::request::prepare(request.owner(), c, 4096, &mut budget())
            .map_err(err)?
            .ok_or("request")?;
    payload(
        &limited,
        node,
        root,
        "diagnostic-cap",
        &serde_json::to_vec(&unicode).map_err(err)?,
        false,
    )?;
    for case in 0..3 {
        let script = root.join("budget-reply.cjs");
        std::fs::write(
            &script,
            format!(
                "process.stdin.resume();process.stdout.write(Buffer.from({}));",
                serde_json::to_string(&encoded).map_err(err)?
            ),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut p = process::start(
            request,
            config(node, &script, encoded.len().max(256), 5000),
            &mut b,
        )
        .map_err(err)?;
        let completed = finish(&mut p, &mut b).map_err(err)?;
        let mut limits = budget().limits();
        let expected = match case {
            0 => StopReason::Cancelled,
            1 => {
                limits.work = 0;
                StopReason::WorkLimit
            }
            _ => {
                limits.allocation_units = 0;
                StopReason::AllocationLimit
            }
        };
        let mut decode_budget = Budget::new(limits);
        if case == 0 {
            decode_budget.cancel();
        }
        assert!(
            matches!(process::reply::decode(completed,&mut decode_budget),Err(process::reply::Error::Stopped(s)) if s==expected)
        );
    }
    Ok(())
}

pub(super) fn fixed_assets() -> Result<nepl3_tools::doc::math::assets::PreparedAssets, String> {
    use nepl3_tools::doc::math::assets::{self, Input};
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../../math/katex/assets.json")).map_err(err)?;
    let pins = inventory["files"].as_array().ok_or("assets")?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("audit/math/node_modules/katex");
    let mut bytes = Vec::new();
    for pin in pins {
        bytes.push(std::fs::read(root.join(pin["source"].as_str().ok_or("source")?)).map_err(err)?);
    }
    let mut input = Vec::new();
    for (pin, bytes) in pins.iter().zip(&bytes) {
        input.push(Input {
            path: pin["path"].as_str().ok_or("path")?,
            mime: pin["mime"].as_str().ok_or("mime")?,
            bytes,
        });
    }
    assets::prepare(&input, 2000000, &mut budget()).map_err(err)
}
fn check_association(
    request: &PreparedRequest<'_>,
    node: &str,
    root: &Path,
    success: &serde_json::Value,
    assets: &nepl3_tools::doc::math::assets::PreparedAssets,
) -> Result<(), String> {
    use process::reply::{Outcome, visual::Preparation};
    use serde_json::json;
    let mut projection_usage: Option<nepl3_core::budget::Usage> = None;
    for case in 0..28 {
        let mut value = success.clone();
        let mut scope = "nepl-math-associated";
        match case {
            0 => {
                value["result"]["html"] = json!("<script>untrusted producer</script>");
                value["result"]["outputBytes"] = json!("<script>untrusted producer</script>".len());
                value["terminationFailure"] = json!(true);
                value["diagnostics"] = json!([{"level":"warn","text":"preserved diagnostic"}]);
                value["diagnosticBytes"] = json!("preserved diagnostic".len() + 1);
            }
            19 => {
                let object = value.as_object_mut().ok_or("reply object")?;
                for key in [
                    "diagnostics",
                    "diagnosticBytes",
                    "implementation",
                    "replyBytes",
                ] {
                    object.remove(key);
                }
            }
            1 => {
                let nodes = value["result"]["visual"]["nodes"]
                    .as_array_mut()
                    .ok_or("nodes")?;
                let last = nodes.len() - 1;
                nodes[last]["classes"] = json!("unrequested-class");
            }
            2 => {
                let nodes = value["result"]["visual"]["nodes"]
                    .as_array_mut()
                    .ok_or("nodes")?;
                let last = nodes.len() - 1;
                nodes[last]["style"] = json!("background-image:url(javascript:bad)");
            }
            3 => scope = "invalid scope",
            11 => {
                value["result"]["visual"]["nodes"] = json!([
                    {"kind":"path","data":"not-path"},
                    {"kind":"svg","width":"1em","height":"1em","view_box":"0 0 1 1","aspect":null,"children":[0]},
                    {"kind":"span","classes":"katex","style":"","aria_hidden":true,"children":[1]}
                ])
            }
            12 => {
                value["result"]["visual"]["nodes"] = json!([
                    {"kind":"path","data":"M0 0 L1 1"},
                    {"kind":"span","classes":"katex","style":"","aria_hidden":true,"children":[0]}
                ])
            }

            _ => {}
        }
        recalculate(&mut value)?;
        let script = root.join("association.cjs");
        std::fs::write(
            &script,
            format!(
                "process.stdin.resume();process.stdout.write(Buffer.from({}));",
                serde_json::to_string(&serde_json::to_vec(&value).map_err(err)?).map_err(err)?
            ),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut p =
            process::start(request, config(node, &script, 100000, 5000), &mut b).map_err(err)?;
        let decoded = process::reply::decode(finish(&mut p, &mut b).map_err(err)?, &mut b);
        if case == 19 {
            // Visual success requires its complete observations envelope.
            // Absence is unknown, not an admissible visual association.
            assert!(matches!(decoded, Err(process::reply::Error::Shape)));
            continue;
        }
        let reply = decoded.map_err(|e| format!("association case {case}: {e:?}"))?;
        let mut limits = budget().limits();
        match case {
            4 => limits.work = 0,
            5 => limits.allocation_units = 0,
            6 => limits.depth = 0,
            7 => limits.nodes = 0,
            8 => limits.output_bytes = 0,
            _ => {}
        }
        let mut prepare_budget = Budget::new(limits);
        if case == 9 {
            prepare_budget.cancel();
        }
        let result = reply.prepare_visual(assets, scope, &mut prepare_budget);
        match case {
            0 | 8 | 10 | 13..=18 | 20..=27 => {
                let Preparation::Visual(visual) = result.map_err(err)? else {
                    return Err("visual".into());
                };
                assert_eq!(prepare_budget.usage().output_bytes, 0);
                assert!(core::ptr::eq(visual.request(), request));
                let rendered = visual.serialize(&mut budget()).map_err(err)?;
                assert!(
                    !rendered
                        .visual()
                        .visual()
                        .html()
                        .contains("untrusted producer")
                );
                assert!(!rendered.visual().visual().html().contains("<script"));
                if case == 0 {
                    assert!(visual.termination_failure());
                    assert!(rendered.termination_failure());
                    let observations = visual.observations().ok_or("observations")?;
                    assert_eq!(observations.diagnostics.len(), 1);
                    assert_eq!(
                        observations.diagnostics[0].level,
                        process::reply::Level::Warn
                    );
                    assert_eq!(observations.diagnostics[0].text, "preserved diagnostic");
                    assert_eq!(
                        observations.diagnostic_bytes,
                        ("preserved diagnostic".len() + 1) as u64
                    );
                    assert!(core::ptr::eq(
                        observations,
                        rendered.observations().ok_or("serialized observations")?
                    ));
                }
                let expected_observations = observation_value(visual.observations());
                let buffers = visual
                    .observations()
                    .map(|o| (o.diagnostics.as_ptr(), o.implementation.node.as_ptr()));
                let termination_failure = visual.termination_failure();
                let mut limits = budget().limits();
                let expected_stop = match case {
                    13 => {
                        limits.work = 0;
                        Some(StopReason::WorkLimit)
                    }
                    14 => {
                        limits.allocation_units = 0;
                        Some(StopReason::AllocationLimit)
                    }
                    15 => {
                        limits.depth = 0;
                        Some(StopReason::DepthLimit)
                    }
                    16 => {
                        limits.nodes = 0;
                        Some(StopReason::NodeLimit)
                    }
                    17 => {
                        limits.output_bytes = 0;
                        Some(StopReason::OutputLimit)
                    }
                    18 => Some(StopReason::Cancelled),
                    20..=27 => {
                        let usage = projection_usage.ok_or("projection baseline")?;
                        let short = case % 2 == 1;
                        let reason = match (case - 20) / 2 {
                            0 => {
                                limits.work = usage.work - u64::from(short);
                                StopReason::WorkLimit
                            }
                            1 => {
                                limits.allocation_units = usage.allocation_units - u64::from(short);
                                StopReason::AllocationLimit
                            }
                            2 => {
                                limits.nodes = usage.nodes - u64::from(short);
                                StopReason::NodeLimit
                            }
                            _ => {
                                limits.output_bytes = usage.output_bytes - u64::from(short);
                                StopReason::OutputLimit
                            }
                        };
                        short.then_some(reason)
                    }
                    _ => None,
                };
                let mut projected_budget = Budget::new(limits);
                if case == 18 {
                    projected_budget.cancel();
                }
                let projected = visual.into_html(&mut projected_budget);
                if let Some(reason) = expected_stop {
                    assert!(
                        matches!(projected, Err(process::reply::Error::Stopped(s)) if s == reason)
                    );
                    assert_eq!(projected_budget.poll(), Err(reason));
                    if case == 25 {
                        let usage = projection_usage.ok_or("projection baseline")?;
                        assert_eq!(projected_budget.usage().output_bytes, usage.output_bytes);
                        assert_eq!(projected_budget.usage().nodes, usage.nodes - 1);
                    }
                } else {
                    let projected = projected.map_err(err)?;
                    if case == 10 {
                        projection_usage = Some(projected_budget.usage());
                    }
                    assert!(core::ptr::eq(projected.request(), request));
                    assert!(core::ptr::eq(projected.visual().assets(), assets));
                    assert_eq!(projected.config(), config(node, &script, 100000, 5000));
                    assert_eq!(
                        observation_value(projected.observations()),
                        expected_observations
                    );
                    assert_eq!(
                        projected
                            .observations()
                            .map(|o| (o.diagnostics.as_ptr(), o.implementation.node.as_ptr())),
                        buffers
                    );
                    assert_eq!(projected.termination_failure(), termination_failure);
                }
            }
            1..=3 | 11 | 12 => assert!(matches!(result, Err(process::reply::Error::Visual(_)))),
            _ => {
                let expected = match case {
                    4 => StopReason::WorkLimit,
                    5 => StopReason::AllocationLimit,
                    6 => StopReason::DepthLimit,
                    7 => StopReason::NodeLimit,
                    _ => StopReason::Cancelled,
                };
                assert!(matches!(result, Err(process::reply::Error::Stopped(s)) if s==expected));
                assert_eq!(prepare_budget.poll(), Err(expected));
            }
        }
    }
    for kind in [
        "invalid-request",
        "unavailable",
        "render-error",
        "stopped",
        "provider-violation",
    ] {
        let mut value = success.clone();
        value["result"] = match kind {
            "invalid-request" | "unavailable" => json!({"kind":kind}),
            "render-error" => json!({"kind":kind,"reason":"parse"}),
            "stopped" => json!({"kind":kind,"reason":"output-limit"}),
            _ => json!({"kind":kind,"reason":"markup"}),
        };
        value["terminationFailure"] = json!(true);
        recalculate(&mut value)?;
        let script = root.join("other.cjs");
        std::fs::write(
            &script,
            format!(
                "process.stdin.resume();process.stdout.write(Buffer.from({}));",
                serde_json::to_string(&serde_json::to_vec(&value).map_err(err)?).map_err(err)?
            ),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut p =
            process::start(request, config(node, &script, 100000, 5000), &mut b).map_err(err)?;
        let reply =
            process::reply::decode(finish(&mut p, &mut b).map_err(err)?, &mut b).map_err(err)?;
        let Preparation::Other(other) = reply
            .prepare_visual(assets, "nepl-math-associated", &mut b)
            .map_err(err)?
        else {
            return Err("promoted nonvisual".into());
        };
        assert!(core::ptr::eq(other.request(), request));
        assert!(other.observations().is_some());
        assert!(other.termination_failure());
        let observed = other.observations().ok_or("observations")?;
        assert_eq!(
            observed.reply_bytes,
            value["replyBytes"].as_u64().ok_or("reply bytes")?
        );
        assert_eq!(
            observed.implementation.node,
            value["implementation"]["node"].as_str().ok_or("node")?
        );

        assert!(matches!(
            (kind, other.outcome()),
            ("invalid-request", Outcome::InvalidRequest(_))
                | ("unavailable", Outcome::Unavailable(_))
                | ("render-error", Outcome::RenderError)
                | ("stopped", Outcome::Stopped(_))
                | ("provider-violation", Outcome::Violation(_))
        ));
    }
    Ok(())
}

fn observation_value(value: Option<&process::reply::Observations>) -> serde_json::Value {
    match value {
        None => serde_json::Value::Null,
        Some(o) => serde_json::json!({
            "diagnostics": o.diagnostics.iter().map(|d| serde_json::json!({"level":format!("{:?}",d.level),"text":d.text})).collect::<Vec<_>>(),
            "diagnosticBytes":o.diagnostic_bytes,"replyBytes":o.reply_bytes,
            "implementation":{"node":o.implementation.node,"platform":o.implementation.platform,"arch":o.implementation.arch,"expectedVmWarnings":o.implementation.expected_vm_warnings}
        }),
    }
}

pub(super) fn owned_driver<'a>(
    request: &'a PreparedRequest<'a>,
    cfg: Config<'a>,
    b: &mut Budget,
) -> Result<process::reply::Reply<'a>, String> {
    let mut process = process::start(request, cfg, b).map_err(err)?;
    process::reply::decode(finish(&mut process, b).map_err(err)?, b).map_err(err)
}
fn wrong_request_driver<'a>(
    request: &'a PreparedRequest<'a>,
    cfg: Config<'a>,
    b: &mut Budget,
) -> Result<process::reply::Reply<'a>, String> {
    let other = request::prepare(request.owner(), request.controls(), 4096, b)
        .map_err(err)?
        .ok_or("request")?;
    assert_eq!(request.bytes(), other.bytes());
    // One bounded test-only leak demonstrates why lifetimes/equal bytes alone
    // do not prove the identity of the request selected by the owning scope.
    let other = Box::leak(Box::new(other));
    owned_driver(other, cfg, b)
}
pub fn check_owned(
    make: &mut impl FnMut(
        Preference,
    ) -> Result<nepl3_tools::doc::math::display::PreparedDisplay, String>,
    node: &str,
) -> Result<(), String> {
    use nepl3_tools::doc::math::display::generation::{
        Attempt, Error as GenerationError, Mismatch, Setup,
    };
    let assets = fixed_assets()?;
    let tools = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = tools.join("audit/math/node_modules");
    let output = std::process::Command::new(node)
        .args([
            "-p",
            "require('node:url').pathToFileURL(process.argv[1]+require('node:path').sep).href",
        ])
        .arg(&root)
        .output()
        .map_err(err)?;
    assert!(output.status.success());
    let url = String::from_utf8(output.stdout).map_err(err)?;
    let script = tools.join("math/katex/node/stdio.mjs");
    let cfg = Config {
        modules_url: url.trim(),
        ..config(node, &script, 1000000, 10000)
    };
    let missing_url = format!("{}missing/", url.trim());
    // Fault-injected wire fixture, deliberately distinct from qualified
    // renderer execution: preserve a true cleanup-failure flag and diagnostics.
    let fixture_owner = make(Preference::KaTeXPreferred)?;
    let mut fixture_budget = budget();
    let fixture_request = request::prepare(
        &fixture_owner,
        request_controls(),
        4096,
        &mut fixture_budget,
    )
    .map_err(err)?
    .ok_or("fixture request")?;
    let mut producer = process::start(&fixture_request, cfg, &mut fixture_budget).map_err(err)?;
    let completed = finish(&mut producer, &mut fixture_budget).map_err(err)?;
    let mut fixture: serde_json::Value = serde_json::from_slice(completed.bytes()).map_err(err)?;
    fixture["terminationFailure"] = serde_json::json!(true);
    fixture["diagnostics"] =
        serde_json::json!([{"level":"warn","text":"composition preserved diagnostic"}]);
    fixture["diagnosticBytes"] = serde_json::json!("composition preserved diagnostic".len() + 1);
    recalculate(&mut fixture)?;
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(err)?
        .as_nanos();
    let temp = Temp(std::env::temp_dir().join(format!(
        "nepl3-composite-observation-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&temp.0).map_err(err)?;
    let fixture_path = temp.0.join("reply.cjs");
    std::fs::write(
        &fixture_path,
        format!(
            "process.stdin.resume();process.stdout.write(Buffer.from({}));",
            serde_json::to_string(&serde_json::to_vec(&fixture).map_err(err)?).map_err(err)?
        ),
    )
    .map_err(err)?;

    let mut composition_usage = None;
    let mut composition_depth = 0;
    for case in 0..31 {
        let preference = if case == 1 || case == 7 {
            Preference::MathMLOnly
        } else {
            Preference::KaTeXPreferred
        };
        let expected_math = make(preference)?
            .into_parts()
            .0
            .into_html(&mut budget())
            .map_err(err)?;
        let prepared = make(preference)?;
        let source = prepared.mathml().syntax.value.nodes.as_ptr();
        let markup = prepared.mathml().rendered.fragment.nodes.as_ptr();
        let doc_node = prepared.doc_node();
        let display = prepared.display();
        let mut controls = request_controls();
        if case == 7 {
            controls.options.timeout_millis = 0;
        }
        let selected_config = if case == 26 {
            Config {
                bridge: &fixture_path,
                ..cfg
            }
        } else if case == 8 || case == 30 {
            Config {
                modules_url: &missing_url,
                ..cfg
            }
        } else {
            cfg
        };
        let transient_scope = String::from(if case == 9 {
            "invalid scope"
        } else {
            "nepl-math-owned"
        });
        let setup = Setup {
            controls,
            request_cap: 4096,
            config: selected_config,
            assets: &assets,
            scope: &transient_scope,
        };
        let mut calls = 0;
        let mut limits = budget().limits();
        if case == 27 {
            limits.work = transient_scope.len() as u64 - 1;
        }
        if case == 28 {
            limits.allocation_units = transient_scope.len() as u64 - 1;
        }
        let mut b = Budget::new(limits);
        if case == 29 {
            b.cancel();
        }
        let result = prepared.generate_owned(
            setup,
            |request, mut cfg, b| {
                calls += 1;
                if case == 2 || case == 25 {
                    return Err("driver failure".into());
                }
                if case == 3 {
                    b.cancel();
                    return Err("driver failure after cancel".into());
                }
                if case == 4 {
                    return wrong_request_driver(request, cfg, b);
                }
                if case == 5 {
                    cfg.output_cap += 1;
                }
                let reply = owned_driver(request, cfg, b)?;
                if case == 6 {
                    b.cancel();
                }
                if case == 11 {
                    b.charge(
                        nepl3_core::budget::Resource::Work,
                        b.limits().work - b.usage().work - 1,
                    )
                    .map_err(err)?;
                }
                if case == 12 {
                    b.charge(
                        nepl3_core::budget::Resource::OutputBytes,
                        b.limits().output_bytes - b.usage().output_bytes,
                    )
                    .map_err(err)?;
                }
                Ok(reply)
            },
            &mut b,
        );
        drop(transient_scope);
        if (27..=29).contains(&case) {
            let reason = match case {
                27 => StopReason::WorkLimit,
                28 => StopReason::AllocationLimit,
                _ => StopReason::Cancelled,
            };
            assert!(matches!(result, Err(GenerationError::Stopped(s)) if s == reason));
            assert_eq!(calls, 0);
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(b.usage().output_bytes, 0);
            continue;
        }

        if case == 3 || case == 6 {
            assert!(matches!(
                result,
                Err(GenerationError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
            continue;
        }
        if case == 11 || case == 12 {
            let reason = if case == 11 {
                StopReason::WorkLimit
            } else {
                StopReason::OutputLimit
            };
            assert!(matches!(result, Err(GenerationError::Stopped(s)) if s == reason));
            assert_eq!(b.poll(), Err(reason));
            continue;
        }
        let result = result.map_err(err)?;
        assert_eq!(
            result.prepared().mathml().syntax.value.nodes.as_ptr(),
            source
        );
        assert_eq!(
            result.prepared().mathml().rendered.fragment.nodes.as_ptr(),
            markup
        );
        assert_eq!(result.prepared().doc_node(), doc_node);
        assert_eq!(result.prepared().display(), display);
        assert_eq!(result.selected_config(), selected_config);
        assert_eq!(result.controls(), controls);
        assert_eq!(
            result.selected_scope(),
            if case == 9 {
                "invalid scope"
            } else {
                "nepl-math-owned"
            }
        );
        match case {
            0 => {
                let Attempt::Visual(visual) = result.attempt() else {
                    return Err("owned visual".into());
                };
                assert!(core::ptr::eq(visual.assets(), &assets));
                assert!(result.observations().is_some());
                assert_eq!(result.termination_failure(), Some(false));
                let request = visual.visual().request();
                nepl3_markup::html::validate(
                    &request.fragment,
                    request.slot,
                    &request.policy,
                    &mut b,
                )
                .map_err(err)?;
            }
            1 => {
                assert!(matches!(result.attempt(), Attempt::NotRequested));
                assert_eq!(calls, 0);
                assert_eq!(result.prepared().tex(), &TexPreparation::MathMLOnly);
                assert!(result.observations().is_none());
                assert_eq!(result.termination_failure(), None);
            }
            2 | 25 => {
                assert!(matches!(result.attempt(),Attempt::DriverFailed(e) if e=="driver failure"));
                assert!(result.observations().is_none());
                assert_eq!(result.termination_failure(), None);
            }
            4 => assert!(matches!(
                result.attempt(),
                Attempt::AssociationRejected(Mismatch::Request)
            )),
            5 => assert!(matches!(
                result.attempt(),
                Attempt::AssociationRejected(Mismatch::Config)
            )),
            7 => {
                assert!(matches!(
                    result.attempt(),
                    Attempt::RequestRejected(request::Error::InvalidControls)
                ));
                assert_eq!(calls, 0);
            }
            8 | 30 => {
                assert!(matches!(
                    result.attempt(),
                    Attempt::Other(process::reply::Outcome::Unavailable(_))
                ));
                assert!(result.observations().is_some());
                assert_eq!(result.termination_failure(), Some(false));
            }
            9 => {
                assert!(matches!(result.attempt(), Attempt::ValidationFailed(_)));
                assert!(result.observations().is_some());
                assert_eq!(result.termination_failure(), Some(false));
            }
            10 | 13..=24 | 26 => assert!(matches!(result.attempt(), Attempt::Visual(_))),
            _ => return Err("case".into()),
        }
        use nepl3_tools::doc::math::display::generation::composite::{Composition, Representation};

        let expected_observations = observation_value(result.observations());
        let expected_termination = result.termination_failure();
        let old_visual = match result.attempt() {
            Attempt::Visual(v) => Some(v.visual().request().clone()),
            _ => None,
        };
        let old_css = match result.attempt() {
            Attempt::Visual(v) => Some(v.visual().stylesheet().to_owned()),
            _ => None,
        };
        // Isolated composition-budget boundary; the existing generation tests
        // separately establish cumulative driver/projection charging.
        let mut limits = budget().limits();
        let expected_stop = if (13..=20).contains(&case) {
            let usage: nepl3_core::budget::Usage =
                composition_usage.ok_or("composition baseline")?;
            let short = case % 2 == 0;
            let reason = match (case - 13) / 2 {
                0 => {
                    limits.work = usage.work - u64::from(short);
                    StopReason::WorkLimit
                }
                1 => {
                    limits.allocation_units = usage.allocation_units - u64::from(short);
                    StopReason::AllocationLimit
                }
                2 => {
                    limits.nodes = usage.nodes - u64::from(short);
                    StopReason::NodeLimit
                }
                _ => {
                    limits.output_bytes = usage.output_bytes - u64::from(short);
                    StopReason::OutputLimit
                }
            };
            short.then_some(reason)
        } else if case == 21 || case == 25 {
            Some(StopReason::Cancelled)
        } else if case == 22 {
            limits.depth = 0;
            Some(StopReason::DepthLimit)
        } else if case == 23 || case == 24 {
            limits.depth = composition_depth + 5 - u64::from(case == 24);
            (case == 24).then_some(StopReason::DepthLimit)
        } else {
            None
        };
        let mut cb = Budget::new(limits);
        if case == 21 || case == 25 {
            cb.cancel();
        }
        let previous_diagnostics = b.usage().diagnostics;
        let composition = if case == 30 {
            result.into_composite_with_policy(
                nepl3_tools::doc::math::display::generation::composite::CompletionPolicy::OrdinaryMathmlFallback,
                &mut b,
            )
        } else if case == 23 || case == 24 {
            cb.with_depth_at_least(5, |b| result.into_composite(b))
        } else {
            result.into_composite(&mut cb)
        };
        if let Some(reason) = expected_stop {
            assert!(
                matches!(composition, Err(nepl3_tools::doc::math::display::generation::composite::Error::Stopped(s)) if s == reason)
            );
            assert_eq!(cb.poll(), Err(reason));
            continue;
        }
        if case == 10 {
            composition_usage = Some(cb.usage());
        }
        match composition.map_err(err)? {
            Composition::Ready(composed) => {
                assert!(matches!(case, 0 | 1 | 10 | 13..=24 | 26 | 30));
                assert_eq!(composed.doc_node(), doc_node);
                assert_eq!(composed.display(), display);
                assert_eq!(composed.math().syntax.value.nodes.as_ptr(), source);
                assert_eq!(composed.selected_config(), Some(selected_config));
                assert_eq!(composed.controls(), Some(controls));
                assert_eq!(
                    observation_value(composed.observations()),
                    expected_observations
                );
                assert_eq!(composed.termination_failure(), expected_termination);
                if case == 30 {
                    use nepl3_tools::doc::math::display::generation::composite::FallbackReason;
                    let Some(FallbackReason::Unavailable(Some(cause))) = composed.fallback() else {
                        return Err("missing retained fallback cause".into());
                    };
                    assert_eq!(cause.code(), "module-file");
                    assert!(cause.module().is_some());
                    assert_eq!(composed.representation(), Representation::MathML);
                    assert!(composed.generated().is_none());
                    assert!(composed.stylesheet().is_empty());
                    assert!(composed.assets().is_none());
                    assert_eq!(b.usage().diagnostics, previous_diagnostics + 1);
                } else {
                    assert!(composed.fallback().is_none());
                }
                if case == 26 {
                    assert_eq!(composed.termination_failure(), Some(true));
                    let observations = composed.observations().ok_or("fixture observations")?;
                    assert_eq!(
                        observations.diagnostics[0].text,
                        "composition preserved diagnostic"
                    );
                    assert_eq!(
                        observations.diagnostic_bytes,
                        "composition preserved diagnostic".len() as u64 + 1
                    );
                    assert_eq!(
                        observations.reply_bytes,
                        fixture["replyBytes"]
                            .as_u64()
                            .ok_or("fixture reply bytes")?
                    );
                }

                assert_eq!(composed.selected_scope(), "nepl-math-owned");
                assert_eq!(composed.math().node_roots, expected_math.node_roots);
                assert!(
                    composed
                        .math()
                        .annotation_roots
                        .iter()
                        .map(|r| (r.node, r.markup))
                        .eq(expected_math
                            .annotation_roots
                            .iter()
                            .map(|r| (r.node, r.markup)))
                );
                if matches!(case, 0 | 1) {
                    let mut emission_budget = budget();
                    let emitted = composed.serialize_html(&mut emission_budget).map_err(err)?;
                    assert!(core::ptr::eq(emitted.source(), &composed));
                    assert_eq!(
                        emission_budget.usage().output_bytes,
                        emitted.html().len() as u64
                    );
                    let baseline = emission_budget.usage();
                    let again = composed.serialize_html(&mut emission_budget).map_err(err)?;
                    assert_eq!(again.html(), emitted.html());
                    assert_eq!(
                        emission_budget.usage().output_bytes,
                        baseline.output_bytes * 2
                    );
                    assert_eq!(emission_budget.usage().work, baseline.work * 2);
                    assert_eq!(
                        emission_budget.usage().allocation_units,
                        baseline.allocation_units * 2
                    );
                    assert_eq!(emission_budget.usage().nodes, baseline.nodes * 2);
                    for resource in 0..4 {
                        for short in [false, true] {
                            let mut limits = budget().limits();
                            let reason = match resource {
                                0 => {
                                    limits.work = baseline.work - u64::from(short);
                                    StopReason::WorkLimit
                                }
                                1 => {
                                    limits.allocation_units =
                                        baseline.allocation_units - u64::from(short);
                                    StopReason::AllocationLimit
                                }
                                2 => {
                                    limits.nodes = baseline.nodes - u64::from(short);
                                    StopReason::NodeLimit
                                }
                                _ => {
                                    limits.output_bytes = baseline.output_bytes - u64::from(short);
                                    StopReason::OutputLimit
                                }
                            };
                            let mut limited = Budget::new(limits);
                            let result = composed.serialize_html(&mut limited);
                            if short {
                                assert!(
                                    matches!(result, Err(nepl3_tools::doc::math::display::generation::composite::Error::Stopped(s)) if s == reason)
                                );
                                let before = limited.usage();
                                assert!(
                                    matches!(composed.serialize_html(&mut limited), Err(nepl3_tools::doc::math::display::generation::composite::Error::Stopped(s)) if s == reason)
                                );
                                assert_eq!(limited.usage(), before);
                            } else {
                                assert_eq!(result.map_err(err)?.html(), emitted.html());
                            }
                        }
                    }
                    let mut max_depth = 0;
                    let mut stack = vec![(composed.math().markup.fragment.root, 1)];
                    while let Some((index, depth)) = stack.pop() {
                        use nepl3_markup::html::HtmlNode;
                        max_depth = max_depth.max(depth);
                        match &composed.math().markup.fragment.nodes[index as usize] {
                            HtmlNode::Element { children, .. }
                            | HtmlNode::MathElement { children, .. }
                            | HtmlNode::SvgElement { children, .. } => {
                                stack.extend(children.iter().map(|n| (*n, depth + 1)))
                            }
                            HtmlNode::Text { .. } => {}
                        }
                    }
                    for short in [false, true] {
                        let mut nested = Budget::new(nepl3_core::budget::Limits {
                            depth: max_depth + 5 - u64::from(short),
                            ..budget().limits()
                        });
                        let result = nested.with_depth_at_least(5, |b| composed.serialize_html(b));
                        if short {
                            assert!(matches!(result, Err(nepl3_tools::doc::math::display::generation::composite::Error::Stopped(StopReason::DepthLimit))));
                        } else {
                            assert_eq!(result.map_err(err)?.html(), emitted.html());
                        }
                    }
                    let mut canceled = budget();
                    canceled.cancel();
                    assert!(matches!(
                        composed.serialize_html(&mut canceled),
                        Err(
                            nepl3_tools::doc::math::display::generation::composite::Error::Stopped(
                                StopReason::Cancelled
                            )
                        )
                    ));
                }
                let request = &composed.math().markup;
                if case == 10 {
                    use nepl3_markup::html::HtmlNode;
                    let mut stack = vec![(request.fragment.root, 1)];
                    while let Some((index, depth)) = stack.pop() {
                        composition_depth = composition_depth.max(depth);
                        match &request.fragment.nodes[index as usize] {
                            HtmlNode::Element { children, .. }
                            | HtmlNode::MathElement { children, .. }
                            | HtmlNode::SvgElement { children, .. } => {
                                stack.extend(children.iter().map(|n| (*n, depth + 1)));
                            }
                            HtmlNode::Text { .. } => {}
                        }
                    }
                }

                nepl3_markup::html::validate(
                    &request.fragment,
                    request.slot,
                    &request.policy,
                    &mut b,
                )
                .map_err(err)?;
                if case == 1 || case == 30 {
                    assert_eq!(composed.representation(), Representation::MathML);
                    assert!(composed.generated().is_none());
                    assert!(composed.stylesheet().is_empty());
                    assert!(composed.assets().is_none());
                    if case == 1 {
                        assert_eq!(composed.tex(), &TexPreparation::MathMLOnly);
                    } else {
                        assert!(matches!(composed.tex(), TexPreparation::Ready { .. }));
                    }
                } else {
                    assert_eq!(composed.representation(), Representation::Dual);
                    assert_eq!(
                        cb.usage().output_bytes,
                        (composed.stylesheet().len() - old_css.as_ref().ok_or("css")?.len()) as u64
                    );
                    let range = composed.generated().ok_or("missing generated range")?;
                    assert_eq!(
                        range.first + range.elements,
                        request.fragment.nodes.len() as u64
                    );
                    assert_eq!(
                        &request.fragment.nodes[..range.first as usize],
                        expected_math.markup.fragment.nodes.as_slice()
                    );
                    use nepl3_markup::html::{HtmlAttribute, HtmlNode, HtmlTag};
                    let old_visual = old_visual.ok_or("visual reference")?;
                    if display == nepl3_markup::mathml::Display::Inline {
                        assert!(
                            old_visual
                                .fragment
                                .nodes
                                .iter()
                                .any(|n| matches!(n, HtmlNode::SvgElement { .. }))
                        );
                    }
                    for (i, mut original) in old_visual.fragment.nodes.into_iter().enumerate() {
                        match &mut original {
                            HtmlNode::Element { children, .. }
                            | HtmlNode::MathElement { children, .. }
                            | HtmlNode::SvgElement { children, .. } => {
                                for child in children {
                                    *child += range.first;
                                }
                            }
                            HtmlNode::Text { .. } => {}
                        }
                        assert_eq!(request.fragment.nodes[range.first as usize + i], original);
                    }

                    let tag = match display {
                        nepl3_markup::mathml::Display::Inline => HtmlTag::Span,
                        nepl3_markup::mathml::Display::Block => HtmlTag::Div,
                    };
                    let HtmlNode::Element {
                        tag: outer_tag,
                        attributes,
                        children,
                    } = &request.fragment.nodes[request.fragment.root as usize]
                    else {
                        return Err("outer wrapper".into());
                    };
                    assert_eq!(*outer_tag, tag);
                    assert_eq!(
                        attributes,
                        &[HtmlAttribute::Class {
                            values: vec![composed.selected_scope().into()]
                        }]
                    );
                    assert!(
                        !attributes
                            .iter()
                            .any(|a| matches!(a, HtmlAttribute::AriaHidden { .. }))
                    );
                    assert_eq!(children, &[range.accessible_root, range.visual_root]);
                    let HtmlNode::Element {
                        tag: accessible_tag,
                        attributes,
                        children,
                    } = &request.fragment.nodes[range.accessible_root as usize]
                    else {
                        return Err("accessible wrapper".into());
                    };
                    assert_eq!(*accessible_tag, tag);
                    assert_eq!(
                        attributes,
                        &[HtmlAttribute::Class {
                            values: vec![format!("{}-accessible", composed.selected_scope())]
                        }]
                    );
                    assert!(
                        !attributes
                            .iter()
                            .any(|a| matches!(a, HtmlAttribute::AriaHidden { .. }))
                    );
                    assert_eq!(children, &[expected_math.markup.fragment.root]);
                    let HtmlNode::Element { attributes, .. } =
                        &request.fragment.nodes[range.visual_root as usize]
                    else {
                        return Err("visual wrapper".into());
                    };
                    assert!(
                        attributes
                            .iter()
                            .any(|a| matches!(a, HtmlAttribute::AriaHidden { value: true }))
                    );
                    assert!(composed.math().node_roots.iter().all(|r| *r < range.first));
                    assert!(core::ptr::eq(composed.assets().ok_or("assets")?, &assets));
                    assert!(
                        composed
                            .stylesheet()
                            .starts_with(old_css.as_deref().ok_or("css")?)
                    );
                    assert!(
                        composed
                            .stylesheet()
                            .contains(".nepl-math-owned .nepl-math-owned-accessible{")
                    );
                    assert_eq!(composed.termination_failure(), Some(case == 26));
                }
            }
            Composition::Deferred(deferred) => {
                assert!(!matches!(case, 0 | 1 | 10 | 13..=24 | 26));
                assert!(match (case, deferred.attempt()) {
                    (2, Attempt::DriverFailed(e)) => e == "driver failure",
                    (4, Attempt::AssociationRejected(Mismatch::Request))
                    | (5, Attempt::AssociationRejected(Mismatch::Config))
                    | (7, Attempt::RequestRejected(_))
                    | (8, Attempt::Other(_))
                    | (9, Attempt::ValidationFailed(_)) => true,
                    _ => false,
                });
                assert_eq!(
                    deferred.prepared().mathml().syntax.value.nodes.as_ptr(),
                    source
                );
                assert_eq!(
                    deferred
                        .prepared()
                        .mathml()
                        .rendered
                        .fragment
                        .nodes
                        .as_ptr(),
                    markup
                );
            }
        }
    }
    fallback_policy::check(make, cfg, &assets, &fixture, &temp.0)?;
    polled_driver::check(make, cfg, &fixture, &temp.0)?;
    cleanup::check(make, cfg, &temp.0)?;
    supervisor::check(make, cfg, &temp.0)?;
    owned_launch::check(make, cfg, &fixture, &temp.0)?;
    owned_supervisor::check(make, cfg, &temp.0)?;
    Ok(())
}

pub fn check_owned_not_requested(
    prepared: nepl3_tools::doc::math::display::PreparedDisplay,
    expected: nepl3_tools::doc::math::RenderedHtmlMath,
    node: &str,
) -> Result<(), String> {
    use nepl3_tools::doc::math::display::generation::{Attempt, Setup};
    assert!(matches!(
        prepared.tex(),
        TexPreparation::Unsupported {
            reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
            ..
        }
    ));
    let annotations = prepared.mathml().annotations.as_ptr();
    let sentence = prepared.mathml().annotations[0]
        .sentence
        .value
        .nodes
        .as_ptr();
    let assets = fixed_assets()?;
    let mut calls = 0;
    let cfg = config(node, Path::new("must-not-execute.mjs"), 100000, 5000);
    let result = prepared
        .generate_owned(
            Setup {
                controls: request_controls(),
                request_cap: 0,
                config: cfg,
                assets: &assets,
                scope: "nepl-math-skipped",
            },
            |_, _, _| {
                calls += 1;
                Err::<process::reply::Reply<'_>, String>("unexpected driver".into())
            },
            &mut budget(),
        )
        .map_err(err)?;
    assert_eq!(calls, 0);
    assert!(matches!(result.attempt(), Attempt::NotRequested));
    assert!(matches!(
        result.prepared().tex(),
        TexPreparation::Unsupported {
            reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
            ..
        }
    ));
    assert_eq!(result.prepared().mathml().annotations.as_ptr(), annotations);
    assert_eq!(
        result.prepared().mathml().annotations[0]
            .sentence
            .value
            .nodes
            .as_ptr(),
        sentence
    );
    assert!(result.observations().is_none());
    assert_eq!(result.termination_failure(), None);
    use nepl3_tools::doc::math::display::generation::composite::{Composition, Representation};
    let Composition::Ready(composed) = result.into_composite(&mut budget()).map_err(err)? else {
        return Err("unsupported TeX MathML composition deferred".into());
    };
    assert_eq!(composed.representation(), Representation::MathML);
    assert_eq!(composed.selected_scope(), "nepl-math-skipped");
    assert!(composed.generated().is_none());
    assert!(composed.stylesheet().is_empty());
    assert!(composed.assets().is_none());
    assert_eq!(composed.math().annotations.as_ptr(), annotations);
    assert_eq!(
        composed.math().annotations[0].sentence.value.nodes.as_ptr(),
        sentence
    );
    assert!(!composed.math().annotation_roots.is_empty());
    assert!(!composed.math().annotations[0].foreign.is_empty());
    assert_eq!(
        annotation_mappings(&composed.math().annotations),
        annotation_mappings(&expected.annotations)
    );
    let mut emitted_budget = budget();
    let serialized = composed.serialize_html(&mut emitted_budget).map_err(err)?;
    assert!(core::ptr::eq(serialized.source(), &composed));
    assert_eq!(
        emitted_budget.usage().output_bytes,
        serialized.html().len() as u64
    );
    let mut reference_budget = budget();
    let reference = nepl3_markup::html::validate(
        &expected.markup.fragment,
        expected.markup.slot,
        &expected.markup.policy,
        &mut reference_budget,
    )
    .map_err(err)?;
    assert_eq!(
        serialized.html(),
        nepl3_markup::html::serialize(&reference, &mut reference_budget).map_err(err)?
    );
    assert_eq!(composed.math().markup, expected.markup);
    assert_eq!(composed.math().node_roots, expected.node_roots);
    assert!(
        composed
            .math()
            .annotation_roots
            .iter()
            .map(|r| (r.node, r.markup))
            .eq(expected.annotation_roots.iter().map(|r| (r.node, r.markup)))
    );
    assert_eq!(
        composed.math().annotations.len(),
        expected.annotations.len()
    );
    for (actual, expected) in composed
        .math()
        .annotations
        .iter()
        .zip(&expected.annotations)
    {
        assert_eq!(actual.origins, expected.origins);
        assert_eq!(actual.embed, expected.embed);
        assert_eq!(actual.sentence_digest, expected.sentence_digest);
        assert_eq!(actual.foreign.len(), expected.foreign.len());
    }

    assert!(matches!(
        composed.tex(),
        TexPreparation::Unsupported {
            reason: nepl3_math_tex::Unsupported::ForeignAnnotation,
            ..
        }
    ));
    Ok(())
}

pub(super) fn annotation_mappings(
    records: &[nepl3_tools::doc::math::AnnotationRecord],
) -> serde_json::Value {
    use nepl3_tools::doc::annotations::ForeignRecord;
    serde_json::Value::Array(records.iter().map(|record| {
        let foreign: Vec<_> = record.foreign.iter().map(|foreign| match foreign {
            ForeignRecord::Math(r) => serde_json::json!({
                "kind":"math", "embed":r.embed.0, "roots":r.output.node_roots,
                "annotationRoots":r.output.annotation_roots.iter().map(|x| (x.node,x.markup)).collect::<Vec<_>>(),
                "annotations":annotation_mappings(&r.output.annotations)
            }),
            ForeignRecord::Document(r) => serde_json::json!({
                "kind":"document", "embed":r.embed.0,
                "origins":r.origins.iter().map(|x| (x.node,x.element)).collect::<Vec<_>>(),
                "foreign":r.foreign.iter().map(|m| serde_json::json!({
                    "embed":m.embed.0, "roots":m.output.node_roots,
                    "annotationRoots":m.output.annotation_roots.iter().map(|x| (x.node,x.markup)).collect::<Vec<_>>(),
                    "annotations":annotation_mappings(&m.output.annotations)
                })).collect::<Vec<_>>()
            }),
        }).collect();
        serde_json::json!({"embed":record.embed,"origins":record.origins.iter().map(|x| (x.node,x.element)).collect::<Vec<_>>(),"foreign":foreign})
    }).collect())
}

#[path = "math_fallback_policy.rs"]
mod fallback_policy;

#[path = "math_polled_driver.rs"]
mod polled_driver;

#[path = "math_cleanup.rs"]
mod cleanup;

#[path = "math_supervisor.rs"]
mod supervisor;

#[path = "math_owned_launch.rs"]
mod owned_launch;

#[path = "math_owned_supervisor.rs"]
mod owned_supervisor;
