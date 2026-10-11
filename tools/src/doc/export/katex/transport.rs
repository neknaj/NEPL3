//! Bounded native process boundary. This is synchronous immutable-source export,
//! not the browser Worker protocol or a hard real-time/physical-memory promise.
use nepl3_core::budget::{Budget, Resource};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
const LIMIT: usize = 8 * 1024 * 1024;
const REQUEST_LIMIT: usize = 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
const FILES: &[(&str, &[u8])] = &[
    (
        "node/host.mjs",
        include_bytes!("../../../../math/katex/node/host.mjs"),
    ),
    (
        "node/execution.mjs",
        include_bytes!("../../../../math/katex/node/execution.mjs"),
    ),
    (
        "node/assets.mjs",
        include_bytes!("../../../../math/katex/node/assets.mjs"),
    ),
    (
        "node/pinned.mjs",
        include_bytes!("../../../../math/katex/node/pinned.mjs"),
    ),
    (
        "node/render.mjs",
        include_bytes!("../../../../math/katex/node/render.mjs"),
    ),
    (
        "node/worker.mjs",
        include_bytes!("../../../../math/katex/node/worker.mjs"),
    ),
    (
        "render.mjs",
        include_bytes!("../../../../math/katex/render.mjs"),
    ),
    (
        "parse.mjs",
        include_bytes!("../../../../math/katex/parse.mjs"),
    ),
    (
        "assets.json",
        include_bytes!("../../../../math/katex/assets.json"),
    ),
    (
        "execution.json",
        include_bytes!("../../../../math/katex/execution.json"),
    ),
];
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _kill = self.0.kill();
        let _reap = self.0.wait();
    }
}
struct PrivateDir(PathBuf);
impl Drop for PrivateDir {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.0);
    }
}
fn private_dir() -> Result<PrivateDir, String> {
    for _ in 0..32 {
        let path = std::env::temp_dir().join(format!(
            "nepl3-doc-katex-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        match builder.create(&path) {
            Ok(()) => return Ok(PrivateDir(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("KaTeX host directory: {e}")),
        }
    }
    Err("KaTeX host directory collisions".into())
}
fn read_bounded(mut pipe: impl Read, cap: usize, overflow: &AtomicBool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = pipe
            .read(&mut chunk)
            .map_err(|e| format!("KaTeX host pipe: {e}"))?;
        if n == 0 {
            return Ok(out);
        }
        if n > cap - out.len() {
            overflow.store(true, Ordering::Release);
            return Err("KaTeX host transport limit".into());
        }
        out.try_reserve(n)
            .map_err(|e| format!("KaTeX host buffer: {e}"))?;
        out.extend_from_slice(&chunk[..n]);
    }
}
/// Package directory is trusted host configuration, never taken from Doc input.
/// Missing optional configuration/capability returns None. Configured process
/// failures, deadlines and overflows abort, never authorize MathML fallback.
pub fn invoke(request: &[u8], b: &mut Budget) -> Result<Option<Vec<u8>>, String> {
    b.poll().map_err(|e| format!("{e:?}"))?;
    let Some(packages) = std::env::var_os("NEPL3_KATEX_NODE_MODULES") else {
        return Ok(None);
    };
    if request.len() > REQUEST_LIMIT {
        return Err(format!(
            "{:?}",
            b.stop(nepl3_core::budget::StopReason::SourceLimit)
        ));
    }
    // Precharge bounded transport storage; internal JS/parser allocation and
    // work remain unobserved, not invented as zero in the export manifest.
    b.charge(
        Resource::AllocationUnits,
        (2 * LIMIT + 16384 + request.len()) as u64,
    )
    .map_err(|e| format!("{e:?}"))?;
    b.charge(Resource::Work, request.len() as u64)
        .map_err(|e| format!("{e:?}"))?;
    let directory = private_dir()?;
    for (path, bytes) in FILES {
        let target = directory.0.join(path);
        fs::create_dir_all(target.parent().ok_or("KaTeX host path")?).map_err(|e| e.to_string())?;
        fs::write(target, bytes).map_err(|e| e.to_string())?;
    }
    let mut child = Process(
        match Command::new("node")
            .args(["--input-type=commonjs", "-e", "if(Number(process.versions.node.split('.')[0])<24){process.exit(78)}else{import(require('node:url').pathToFileURL(process.argv[1]).href).catch(()=>process.exit(70))}"])
            .arg(directory.0.join("node/host.mjs"))
            .arg(packages)
            .env("TMPDIR", &directory.0)
            .env("TMP", &directory.0)
            .env("TEMP", &directory.0)
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("KaTeX host spawn: {e}")),
        },
    );
    let stdout = child.0.stdout.take().ok_or("KaTeX host stdout")?;
    let stderr = child.0.stderr.take().ok_or("KaTeX host stderr")?;
    let mut stdin = child.0.stdin.take().ok_or("KaTeX host stdin")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    let result = thread::scope(|scope| {
        let out_flag = Arc::clone(&overflow);
        let err_flag = Arc::clone(&overflow);
        let output = scope.spawn(move || read_bounded(stdout, LIMIT, &out_flag));
        let errors = scope.spawn(move || read_bounded(stderr, 8192, &err_flag));
        let input = scope.spawn(move || stdin.write_all(request).map_err(|e| e.to_string()));
        let status = loop {
            if overflow.load(Ordering::Acquire) || start.elapsed() >= Duration::from_secs(60) {
                let _kill = child.0.kill();
                let reaped = child.0.wait().map_err(|e| e.to_string());
                break reaped.and_then(|_| {
                    Err(if overflow.load(Ordering::Acquire) {
                        "KaTeX host transport limit"
                    } else {
                        "KaTeX host deadline"
                    }
                    .into())
                });
            }
            match child.0.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(e) => {
                    let _kill = child.0.kill();
                    let _reap = child.0.wait();
                    break Err(e.to_string());
                }
            }
        };
        let input = input.join().map_err(|_| "KaTeX host input thread")?;
        let output = output.join().map_err(|_| "KaTeX host output thread")?;
        let errors = errors.join().map_err(|_| "KaTeX host diagnostic thread")?;
        let status = status?;
        let output = output?;
        let errors = errors?;
        if status.code() == Some(78) && errors.is_empty() && output.is_empty() {
            return Ok(None);
        }
        input?;
        if !status.success() || !errors.is_empty() {
            return Err("KaTeX host process failure".into());
        }
        Ok(Some(output))
    });
    b.poll().map_err(|e| format!("{e:?}"))?;
    if result.is_err() {
        b.cancel();
    }
    result
}

pub fn identity() -> String {
    use sha2::{Digest as _, Sha256};
    let mut hash = Sha256::new();
    for (path, bytes) in FILES {
        hash.update((path.len() as u64).to_le_bytes());
        hash.update(path.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_pipe_exact_and_overflow() -> Result<(), String> {
        let flag = AtomicBool::new(false);
        assert_eq!(read_bounded(&b"abcd"[..], 4, &flag)?, b"abcd");
        assert!(!flag.load(Ordering::Acquire));
        assert!(read_bounded(&b"abcde"[..], 4, &flag).is_err());
        assert!(flag.load(Ordering::Acquire));
        Ok(())
    }
    #[test]
    fn prior_stop_precedes_capability_and_spawn() {
        let mut b = crate::doc::source::budget();
        b.cancel();
        assert!(invoke(b"invalid", &mut b).is_err());
        assert_eq!(b.usage(), Default::default());
    }
}
