pub(crate) mod json;

use crate::{Result, command};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// References must resolve inside the repository, including through symlinks.
pub(crate) fn local_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.contains('\\')
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("not a repository-relative path: {relative}").into());
    }
    let full = root.join(path);
    if !full.canonicalize()?.starts_with(root.canonicalize()?) {
        return Err(format!("path escapes repository: {relative}").into());
    }
    Ok(full)
}

fn paths(bytes: &[u8]) -> Result<BTreeSet<String>> {
    bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| Ok(std::str::from_utf8(p)?.to_owned()))
        .collect()
}

pub(crate) fn inventory(root: &Path) -> Result<BTreeSet<String>> {
    paths(&command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?)
}

fn forbidden(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let parts: Vec<_> = lower.split('/').collect();
    parts.iter().any(|part| {
        matches!(
            *part,
            ".tmp" | "target" | "dist" | ".git" | "node_modules" | "__pycache__"
        )
    }) || lower.ends_with(".pyc")
        || parts.last().is_some_and(|name| {
            *name == ".env"
                || (name.starts_with(".env.") && !name.ends_with(".example"))
                || matches!(*name, "id_rsa" | "id_ed25519" | "credentials.json")
                || [".pem", ".key", ".p12", ".pfx"]
                    .iter()
                    .any(|suffix| name.ends_with(suffix))
        })
}

fn text_file(path: &str) -> bool {
    matches!(
        Path::new(path).extension().and_then(|s| s.to_str()),
        Some(
            "md" | "rs"
                | "toml"
                | "lock"
                | "json"
                | "yml"
                | "yaml"
                | "txt"
                | "neplg"
                | "nepld"
                | "neplm"
                | "neplc"
                | "html"
                | "css"
                | "js"
                | "mjs"
                | "ts"
                | "py"
                | "ps1"
                | "sh"
                | "svg"
        )
    ) || Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.starts_with('.') || matches!(s, "LICENSE" | "CODEOWNERS" | "NOTICE"))
}

// Historical evidence remains reconstructible from Git. New executable copies
// belong in managed tests/tools, not another snapshot under results.
const EVIDENCE_BASELINE: &str = "d87ca8c4ba18e74bc71efb0e2958af87ac971e1b";

fn evidence_source(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("conformance/results/") else {
        return false;
    };
    let name = relative.strip_suffix(".fixture").unwrap_or(relative);
    relative
        .split('/')
        .any(|part| matches!(part, "source" | "sources" | "snapshot" | "context"))
        || matches!(
            Path::new(name).extension().and_then(|s| s.to_str()),
            Some("py" | "rs" | "js" | "mjs" | "ts" | "ps1" | "sh")
        )
        || matches!(
            Path::new(name).file_name().and_then(|s| s.to_str()),
            Some(
                "AGENTS.md"
                    | "CODEX.md"
                    | "Cargo.toml"
                    | "Cargo.lock"
                    | "implementation-status.json"
            )
        )
}

pub(crate) fn check(root: &Path) -> Result<usize> {
    let files = paths(&command(
        root,
        "git",
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?)?;
    let historical = paths(&command(
        root,
        "git",
        &[
            "ls-tree",
            "-r",
            "--name-only",
            "-z",
            EVIDENCE_BASELINE,
            "--",
            "conformance/results/",
        ],
    )?)?;
    for name in &files {
        if evidence_source(name) && !historical.contains(name) {
            return Err(format!("new evidence source copy: {name}; put reusable logic in tools/tests and reference its Git revision").into());
        }
        if forbidden(name) {
            return Err(format!("forbidden repository file: {name}").into());
        }
        let path = local_path(root, name)?;
        if fs::symlink_metadata(root.join(name))?
            .file_type()
            .is_symlink()
        {
            return Err(format!("repository symlinks require explicit review: {name}").into());
        }
        if fs::metadata(&path)?.len() > 1024 * 1024 {
            return Err(format!("file exceeds 1 MiB review threshold: {name}").into());
        }
        let bytes = fs::read(&path)?;
        if text_file(name) {
            let text =
                std::str::from_utf8(&bytes).map_err(|e| format!("{name}: invalid UTF-8: {e}"))?;
            if text.starts_with('\u{feff}') && !name.starts_with("conformance/fixtures/source/") {
                return Err(format!("{name}: UTF-8 BOM is not permitted").into());
            }
            if name.ends_with(".json") {
                json::validate(text).map_err(|e| format!("{name}: {e}"))?;
            }
            // Heuristic hygiene guard, not a full credential scanner.
            if text.contains(&["-----BEGIN ", "PRIVATE KEY-----"].concat())
                || text.contains(&["-----BEGIN RSA ", "PRIVATE KEY-----"].concat())
            {
                return Err(format!("private key marker in {name}").into());
            }
        }
    }
    Ok(files.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_data_and_managed_tests_are_separate() {
        for name in [
            "conformance/results/new/seal.py.fixture",
            "conformance/results/new/source/design/tasks.json",
            "conformance/results/new/AGENTS.md",
        ] {
            assert!(evidence_source(name));
        }
        for name in [
            "conformance/results/new/manifest.json",
            "conformance/results/new/stdout.log",
            "tools/evidence/test_runner.py",
            "conformance/fixtures/source/test.rs",
        ] {
            assert!(!evidence_source(name));
        }
    }

    #[test]
    fn nul_paths_keep_spaces_and_non_ascii() -> Result<()> {
        let names = paths("doc/日本 語.md\0Cargo.toml\0".as_bytes())?;
        assert!(names.contains("doc/日本 語.md"));
        assert_eq!(names.len(), 2);
        Ok(())
    }

    #[test]
    fn secret_and_artifact_names_are_rejected() {
        for name in [
            ".tmp/explanation.md",
            "target/output",
            "a/.env.production",
            "keys/private.pem",
            "id_ed25519",
        ] {
            assert!(forbidden(name), "{name}");
        }
        assert!(!forbidden(".env.example"));
        assert!(!forbidden("doc/tmp.md"));
    }

    #[test]
    fn escaping_reference_is_rejected_before_reading() {
        for name in ["../secret", "/absolute", "a/../../b", "C:\\secret", ""] {
            assert!(local_path(Path::new("."), name).is_err(), "{name}");
        }
    }
}
