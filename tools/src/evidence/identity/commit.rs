//! Read immutable Git objects, never check out or execute historical source.
use super::{Inputs, Snapshot, included};
use crate::{Result, repository, task::TaskFile};
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

const MAX_BLOB: usize = 1024 * 1024;
const MAX_TREE: usize = 8 * 1024 * 1024;
const MAX_FILES: usize = 16_384;

fn object_id(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Bound stdout while reading; a failed/oversized read terminates and reaps Git.
/// These commands require no credential, filter, hook, or network operation.
fn git(root: &Path, args: &[&str], limit: usize) -> Result<Vec<u8>> {
    let mut child = Command::new("git")
        .args([
            "--no-replace-objects",
            "--no-optional-locks",
            "--no-lazy-fetch",
        ])
        .args(args)
        .env("GIT_NO_LAZY_FETCH", "1")
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let result = (|| -> Result<Vec<u8>> {
        let stdout = child.stdout.take().ok_or("Git stdout unavailable")?;
        let mut bytes = Vec::new();
        stdout
            .take(u64::try_from(limit)? + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            return Err("Git object output exceeds its byte limit".into());
        }
        Ok(bytes)
    })();
    if result.is_err() {
        // An already exited process may reject kill; wait still reaps it.
        let _termination = child.kill();
    }
    let status = child.wait()?;
    let bytes = result?;
    if !status.success() {
        return Err(format!(
            "Git object read failed ({status}); Git must support --no-lazy-fetch and required objects must exist locally"
        )
        .into());
    }
    Ok(bytes)
}

fn paths(raw: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut entries = BTreeMap::new();
    for record in raw
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let (header, path) = std::str::from_utf8(record)?
            .split_once('\t')
            .ok_or("malformed Git tree record")?;
        let fields: Vec<_> = header.split(' ').collect();
        if fields.len() != 3
            || !object_id(fields[2])
            || path.is_empty()
            || path.contains('\\')
            || path.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err("invalid Git tree entry".into());
        }
        if !included(path) {
            continue;
        }
        if !matches!(fields[0], "100644" | "100755") || fields[1] != "blob" {
            return Err(format!("identity input is not a regular Git blob: {path}").into());
        }
        if entries.len() >= MAX_FILES
            || entries
                .insert(path.to_owned(), fields[2].to_owned())
                .is_some()
        {
            return Err("duplicate path or excessive Git input inventory".into());
        }
    }
    Ok(entries)
}

fn blob(root: &Path, id: &str) -> Result<Vec<u8>> {
    let size = git(root, &["cat-file", "-s", id], 32)?;
    let size: usize = std::str::from_utf8(&size)?.trim().parse()?;
    if size > MAX_BLOB {
        return Err("identity input exceeds 1 MiB".into());
    }
    let bytes = git(root, &["cat-file", "blob", id], size)?;
    if bytes.len() != size {
        return Err("Git blob length mismatch".into());
    }
    Ok(bytes)
}

pub(crate) fn committed(root: &Path, commit: &str) -> Result<Snapshot> {
    if !object_id(commit) {
        return Err(
            "identity commit must be an immutable 40-character lowercase hexadecimal commit ID"
                .into(),
        );
    }
    if std::str::from_utf8(&git(root, &["cat-file", "-t", commit], 32)?)?.trim() != "commit" {
        return Err("identity revision must identify a commit, not a tag/tree/blob".into());
    }
    let entries = paths(&git(
        root,
        &["ls-tree", "-r", "-z", "--full-tree", commit],
        MAX_TREE,
    )?)?;
    let tasks = entries
        .get("design/tasks.json")
        .ok_or("committed task definition missing")?;
    let task_bytes = blob(root, tasks)?;
    repository::json::validate(std::str::from_utf8(&task_bytes)?)?;
    let task_file: TaskFile = serde_json::from_slice(&task_bytes)?;
    let mut inputs = Inputs::new();
    for (path, id) in entries {
        inputs.add(&path, &blob(root, &id)?)?;
    }
    Ok(inputs.finish(task_file.design))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command, evidence::identity, testing::Fixture};
    use serde_json::json;

    fn fixture() -> Result<Fixture> {
        let files = Fixture::new()?;
        files.git()?;
        command(files.root(), "git", &["config", "core.autocrlf", "false"])?;
        files.json(
            "design/tasks.json",
            &json!({"design":"fixture-r1","tasks":[]}),
        )?;
        files.write("src/source.txt", b"a\r\n\0\xef\xbb\xbf")?;
        files.write("doc/spec/source.md", "a𠮷b\r\n文書\n")?;
        files.write("implementation-status.json", "excluded\n")?;
        Ok(files)
    }

    fn commit_index(files: &Fixture) -> Result<String> {
        let tree = command(files.root(), "git", &["write-tree"])?;
        let tree = std::str::from_utf8(&tree)?.trim();
        let commit = command(
            files.root(),
            "git",
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit-tree",
                tree,
                "-m",
                "Synthetic identity test only",
            ],
        )?;
        Ok(std::str::from_utf8(&commit)?.trim().to_owned())
    }

    fn save(files: &Fixture) -> Result<String> {
        command(files.root(), "git", &["add", "--all"])?;
        commit_index(files)
    }

    #[test]
    fn committed_and_clean_checkout_profiles_match_without_byte_normalization() -> Result<()> {
        let files = fixture()?;
        let revision = save(&files)?;
        let checkout = identity(files.root())?;
        let snapshot = committed(files.root(), &revision)?;
        assert_eq!(snapshot.design_revision, checkout.design_revision);
        assert_eq!(snapshot.identity, checkout.identity);
        // Independent Python hashlib vectors over the literal fixture bytes.
        assert_eq!(
            snapshot.identity.source_sha256,
            "495d9f28928c3bd73ffd248498adea02615f11ff4f0a4c64a7ace8b59be57621"
        );
        assert_eq!(
            snapshot.identity.spec_sha256,
            "7a8349dd9f561ca4ecf21dbb7cbf4b5bf2c198f9ab2aa14788fd6abcac39e4c4"
        );
        // Neither dirty files nor new untracked inputs replace committed bytes.
        files.write("src/source.txt", "dirty\n")?;
        files.write("src/untracked.txt", "untracked\n")?;
        assert_eq!(
            committed(files.root(), &revision)?.identity,
            snapshot.identity
        );
        assert_ne!(identity(files.root())?.identity, snapshot.identity);
        assert_eq!(fs_bytes(files.root(), "src/source.txt")?, b"dirty\n");
        Ok(())
    }

    fn fs_bytes(root: &Path, path: &str) -> Result<Vec<u8>> {
        Ok(std::fs::read(root.join(path))?)
    }

    #[test]
    fn only_existing_exclusions_preserve_the_identity_between_commits() -> Result<()> {
        let files = fixture()?;
        let first = committed(files.root(), &save(&files)?)?;
        files.write("implementation-status.json", "changed excluded state\n")?;
        files.write("conformance/results/result.json", "excluded evidence\n")?;
        files.write("tasks/T01.md", "excluded generated task\n")?;
        let second = committed(files.root(), &save(&files)?)?;
        assert_eq!(first.identity, second.identity);
        files.write("src/source.txt", "actual source change\n")?;
        let third = committed(files.root(), &save(&files)?)?;
        assert_ne!(second.identity.source_sha256, third.identity.source_sha256);
        assert_eq!(second.identity.spec_sha256, third.identity.spec_sha256);
        files.write("doc/spec/source.md", "actual spec change\n")?;
        let fourth = committed(files.root(), &save(&files)?)?;
        assert_ne!(third.identity.source_sha256, fourth.identity.source_sha256);
        assert_ne!(third.identity.spec_sha256, fourth.identity.spec_sha256);
        Ok(())
    }

    #[test]
    fn mutable_refs_noncommit_ids_and_replacement_objects_cannot_select_inputs() -> Result<()> {
        let files = fixture()?;
        let original = save(&files)?;
        let expected = committed(files.root(), &original)?;
        assert!(!object_id(&"A".repeat(40)));
        assert!(object_id(&"a".repeat(40)));
        for bad in [
            "HEAD".to_owned(),
            original[..12].to_owned(),
            "A".repeat(40),
            "0".repeat(40),
        ] {
            assert!(committed(files.root(), &bad).is_err(), "{bad}");
        }
        let tree = command(files.root(), "git", &["write-tree"])?;
        assert!(committed(files.root(), std::str::from_utf8(&tree)?.trim()).is_err());
        files.json(
            "design/tasks.json",
            &json!({"design":"replacement-r2","tasks":[]}),
        )?;
        let replacement = save(&files)?;
        command(files.root(), "git", &["replace", &original, &replacement])?;
        assert_eq!(
            committed(files.root(), &original)?.identity,
            expected.identity
        );
        assert_eq!(
            committed(files.root(), &original)?.design_revision,
            "fixture-r1"
        );
        Ok(())
    }

    #[test]
    fn linked_oversized_and_malformed_committed_inputs_fail() -> Result<()> {
        let files = fixture()?;
        let _ = save(&files)?;
        let blob_id = command(files.root(), "git", &["rev-parse", ":src/source.txt"])?;
        let blob_id = std::str::from_utf8(&blob_id)?.trim();
        // Set a symlink index entry without requiring OS symlink privilege.
        command(
            files.root(),
            "git",
            &[
                "update-index",
                "--cacheinfo",
                &format!("120000,{blob_id},src/source.txt"),
            ],
        )?;
        assert!(committed(files.root(), &commit_index(&files)?).is_err());
        files.write("src/source.txt", vec![b'x'; MAX_BLOB + 1])?;
        assert!(committed(files.root(), &save(&files)?).is_err());
        files.write("src/source.txt", "normal\n")?;
        files.write(
            "design/tasks.json",
            r#"{"design":"one","design":"two","tasks":[]}"#,
        )?;
        assert!(committed(files.root(), &save(&files)?).is_err());
        Ok(())
    }

    #[test]
    fn missing_promisor_blobs_fail_without_fetching_from_the_local_fixture_remote() -> Result<()> {
        let files = fixture()?;
        let revision = save(&files)?;
        command(
            files.root(),
            "git",
            &["update-ref", "refs/heads/main", &revision],
        )?;
        command(
            files.root(),
            "git",
            &["config", "uploadpack.allowFilter", "true"],
        )?;
        command(
            files.root(),
            "git",
            &[
                "clone",
                "--no-local",
                "--filter=blob:none",
                "--no-checkout",
                "--branch",
                "main",
                ".",
                "partial",
            ],
        )?;
        let partial = files.root().join("partial");
        let id = command(
            files.root(),
            "git",
            &["rev-parse", &format!("{revision}:design/tasks.json")],
        )?;
        let id = std::str::from_utf8(&id)?.trim();
        assert!(git(&partial, &["cat-file", "-e", id], 1).is_err());
        assert!(committed(&partial, &revision).is_err());
        assert!(
            git(&partial, &["cat-file", "-e", id], 1).is_err(),
            "missing blob was fetched"
        );
        // Positive control: the same local remote really can supply this object
        // when normal Git lazy fetching is permitted. No external host is used.
        assert_eq!(
            command(&partial, "git", &["cat-file", "blob", id])?,
            fs_bytes(files.root(), "design/tasks.json")?
        );
        assert!(git(&partial, &["cat-file", "-e", id], 1).is_ok());
        Ok(())
    }

    #[test]
    fn malformed_tree_rows_and_git_output_overflow_fail() -> Result<()> {
        let id = "a".repeat(40);
        for row in [
            format!("100644 blob {id}\t../escape\0"),
            format!("160000 commit {id}\tsubmodule\0"),
            "100644 blob bad\tfile\0".to_owned(),
            format!("100644 blob {id}\tfile\0").repeat(2),
            format!("100644 blob {id}\tbad\\path\0"),
        ] {
            assert!(paths(row.as_bytes()).is_err());
        }
        let files = fixture()?;
        assert!(git(files.root(), &["--version"], 1).is_err());
        Ok(())
    }
}
