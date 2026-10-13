//! Observe the real transport deadline independently of the fixture's eventual exit.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
type Error = Box<dyn std::error::Error>;

fn signal(pid: u32, signal: &str) -> Result<bool, Error> {
    Ok(Command::new("kill")
        .args([signal, &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}

fn host_pid(path: &Path) -> Result<u32, Error> {
    let pid: u32 = fs::read_to_string(path)?.trim().parse()?;
    if pid <= 1 {
        return Err("invalid fixture PID".into());
    }
    Ok(pid)
}

pub fn verify(binary: &str) -> Result<(), Error> {
    let root = std::env::temp_dir().join(format!("nepl3-katex-deadline-{}", std::process::id()));
    fs::create_dir(&root)?;
    let result = run(binary, &root);
    let cleanup = fs::remove_dir_all(&root);
    result?;
    cleanup?;
    Ok(())
}

fn run(binary: &str, root: &Path) -> Result<(), Error> {
    let mocks = root.join("bin");
    let temporary = root.join("temporary");
    fs::create_dir(&mocks)?;
    fs::create_dir(&temporary)?;
    let pid_file = root.join("host.pid");
    let directory_file = root.join("host.directory");
    let node = mocks.join("node");
    // exec keeps the recorded PID as the actual direct transport child. No
    // surviving descendant is introduced or claimed as a supported boundary.
    fs::write(
        &node,
        "#!/bin/sh\nprintf '%s' \"$$\" > \"$NEPL3_TEST_HOST_PID\"\nprintf '%s' \"$TMPDIR\" > \"$NEPL3_TEST_HOST_DIRECTORY\"\nexec sleep 120\n",
    )?;
    fs::set_permissions(&node, fs::Permissions::from_mode(0o700))?;
    let mut paths = vec![mocks];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").ok_or("PATH")?,
    ));
    let source = root.join("input.nepld");
    fs::write(
        &source,
        "article en \"Deadline\" body cons display Math frac 1 2 nil",
    )?;
    let out = root.join("output");
    let stderr = root.join("stderr");
    let start = Instant::now();
    let mut child = Command::new(binary)
        .env("PATH", std::env::join_paths(paths)?)
        .env("TMPDIR", &temporary)
        .env("TMP", &temporary)
        .env("TEMP", &temporary)
        .env("NEPL3_TEST_HOST_PID", &pid_file)
        .env("NEPL3_TEST_HOST_DIRECTORY", &directory_file)
        // The controlled launcher never loads packages. This regression needs
        // neither npm nor a real Node installation; the real-host test remains.
        .env("NEPL3_KATEX_NODE_MODULES", root)
        .args(["doc-html", "export"])
        .arg(&source)
        .arg(&out)
        .stdout(Stdio::null())
        .stderr(fs::File::create(&stderr)?)
        .spawn()?;
    // A separate watchdog must fail before the 120-second fixture naturally
    // exits. File-backed diagnostics avoid blocking on a captured pipe at cleanup.
    let status = (|| -> Result<_, Error> {
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if start.elapsed() >= Duration::from_secs(90) {
                return Err("independent watchdog: KaTeX deadline did not complete".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
    })();
    if status.is_err() {
        if let Ok(pid) = host_pid(&pid_file) {
            let _terminate_fixture = signal(pid, "-KILL");
        }
        let _terminate_export = child.kill();
        let _reap_export = child.wait();
    }
    let status = status?;
    let elapsed = start.elapsed();
    let pid = host_pid(&pid_file)?;
    let still_alive = signal(pid, "-0")?;
    if still_alive {
        let _terminate_fixture = signal(pid, "-KILL");
        return Err("deadline did not terminate and reap its direct host child".into());
    }
    if elapsed >= Duration::from_secs(90) {
        return Err("deadline result arrived after independent elapsed-time bound".into());
    }
    if status.success() || out.exists() {
        return Err("deadline incorrectly published output or succeeded".into());
    }
    let diagnostic = fs::read_to_string(&stderr)?;
    if !diagnostic.contains("KaTeX host deadline") {
        return Err(format!(
            "export failed for a reason other than the transport deadline: {diagnostic}"
        )
        .into());
    }
    let private = std::path::PathBuf::from(fs::read_to_string(directory_file)?);
    if !private.starts_with(&temporary) || private.exists() {
        return Err("invocation private directory was not identified and removed".into());
    }
    Ok(())
}
