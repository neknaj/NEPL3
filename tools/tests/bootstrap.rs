//! Exercise the host importer on the executable main thread, whose Windows
//! stack differs from Rust's test-worker stack. No stack size override is used.
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
#[test]
fn seed_import_runs_on_main_thread_for_complete_sources_and_rejects_bad_input()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root")?;
    for path in [
        "examples/grammar/angle-tag.neplg",
        "languages/grammar/syntax.neplg",
        "languages/doc/syntax.neplg",
        "languages/math/syntax.neplg",
        "languages/circuit/syntax.neplg",
    ] {
        let seed = Command::new("python")
            .current_dir(root)
            .args([
                "tools/bootstrap/grammar.py",
                path,
                "--source-id",
                path,
                "--uri",
                "memory:seed-regression",
            ])
            .env("PYTHONIOENCODING", "utf-8")
            .output()?;
        assert!(
            seed.status.success(),
            "{}",
            String::from_utf8_lossy(&seed.stderr)
        );
        let mut child = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["bootstrap", "seed-check"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        child.stdin.take().ok_or("stdin")?.write_all(&seed.stdout)?;
        let result = child.wait_with_output()?;
        assert!(
            result.status.success(),
            "{path}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = String::from_utf8(result.stdout)?;
        assert!(stdout.starts_with("seed imported: "));
        // The complete Angle fixture contains 36 semantic constructors. List
        // cons/nil and builtin literals are retained positions, not AST nodes.
        if path == "examples/grammar/angle-tag.neplg" {
            assert!(stdout.starts_with("seed imported: 36 nodes;"));
        }
    }
    // A malformed top-level shape must return through the host result rather
    // than aborting the process.
    let mut child = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["bootstrap", "seed-check"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child.stdin.take().ok_or("stdin")?.write_all(b"{}")?;
    let result = child.wait_with_output()?;
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stderr)?.contains("seed import: Shape"));
    let mut child = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["bootstrap", "seed-check"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("stdin")?
        .write_all(&vec![b' '; 16 * 1024 * 1024 + 1])?;
    let result = child.wait_with_output()?;
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8(result.stderr)?.contains("seed JSON exceeds 16 MiB"));
    Ok(())
}
