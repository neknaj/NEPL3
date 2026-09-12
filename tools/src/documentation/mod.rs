mod markdown;

use crate::{
    Result, command, json,
    repository::{inventory, local_path},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const OUTPUT: &str = "doc/migration/generated/doc-inventory.json";
const CONTRACTS: &[&str] = &[
    "design/forms.json",
    "design/markup.json",
    "interfaces/model.json",
    "interfaces/contracts.json",
    "languages/doc/syntax.neplg",
];

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema: String,
    baseline_commit: String,
    scope: String,
    extractor: String,
    inline_projection: String,
    parser_options: Vec<String>,
    capability_contracts: Vec<File>,
    pages: Vec<Page>,
    rustdoc_sources: Vec<File>,
    exclusions: Vec<Exclusion>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct File {
    path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Exclusion {
    path: String,
    reason: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Page {
    file: File,
    classification: String,
    canonical_source: String,
    migration_action: String,
    stable_page_id: Option<String>,
    published_url: Option<String>,
    identity_status: String,
    structure: markdown::Structure,
}

#[derive(Debug, Serialize, PartialEq)]
struct Delta {
    added: Vec<String>,
    changed: Vec<String>,
    removed: Vec<String>,
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn file(path: &str, bytes: &[u8]) -> Result<File> {
    Ok(File {
        path: path.into(),
        sha256: digest(bytes),
        bytes: u64::try_from(bytes.len())?,
    })
}

fn commit_is_valid(commit: &str) -> bool {
    commit.len() == 40
        && commit
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn snapshot_paths(root: &Path, commit: &str) -> Result<BTreeSet<String>> {
    if !commit_is_valid(commit) {
        return Err(
            "inventory commit must be an immutable 40-character lowercase hexadecimal commit ID"
                .into(),
        );
    }
    let resolved = command(
        root,
        "git",
        &["rev-parse", "--verify", &format!("{commit}^{{commit}}")],
    )?;
    if std::str::from_utf8(&resolved)?.trim() != commit {
        return Err("inventory commit did not resolve exactly".into());
    }
    let bytes = command(root, "git", &["ls-tree", "-r", "-z", commit])?;
    bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| {
            let record = std::str::from_utf8(p)?;
            let (header, path) = record.split_once('\t').ok_or("malformed Git tree record")?;
            let fields: Vec<_> = header.split_whitespace().collect();
            if fields.len() != 3 || !matches!(fields[0], "100644" | "100755") || fields[1] != "blob"
            {
                return Err(format!(
                    "inventory baseline requires regular tracked files; unsupported mode at {path}"
                )
                .into());
            }
            Ok(path.to_owned())
        })
        .collect()
}

fn blob(root: &Path, commit: &str, path: &str) -> Result<Vec<u8>> {
    let bytes = command(root, "git", &["show", &format!("{commit}:{path}")])?;
    if bytes.len() > 1024 * 1024 {
        return Err(format!("inventory input exceeds 1 MiB: {path}").into());
    }
    Ok(bytes)
}

fn classification(path: &str) -> (&'static str, String, &'static str) {
    if path.starts_with("tasks/") {
        (
            "generated-task-projection",
            "design/tasks.json + implementation-status.json".into(),
            "Keep JSON as canonical input; migrate generator to emit Doc and required Markdown projections without hand editing generated pages",
        )
    } else if path.starts_with("doc/spec/") && path.ends_with("-signatures.md") {
        (
            "declared-signature-projection",
            "design/forms.json".into(),
            "Declared derived from forms, but a checked signature-table generator is not currently implemented; retain form source and implement reproducible Doc/Markdown projections",
        )
    } else if path.starts_with(".github/") {
        (
            "github-platform-template",
            path.into(),
            "Keep operational Markdown template; account for checkboxes and links in inventory, not a duplicated specification page",
        )
    } else if !path.contains('/') {
        (
            "authored-github-entry",
            path.into(),
            "Preserve current Markdown entry; switch to generated platform projection only after reviewed Doc source exists",
        )
    } else {
        (
            "authored-document",
            path.into(),
            "Migrate page after capability design and semantic review; history README is evolving prose, not immutable evidence",
        )
    }
}

fn exclusion(path: &str) -> &'static str {
    if path.starts_with("doc/history/") {
        "Immutable historical non-Markdown input: preserve original bytes; not authored prose or executable acceptance evidence"
    } else if path == "LICENSE" {
        "License text retained as legal artifact; do not rewrite as authored Doc prose"
    } else if path.starts_with("examples/") || path.starts_with("languages/") {
        "Canonical language/example input; link by identity and preserve source rather than copy into authored documentation"
    } else if path == OUTPUT {
        "Generated inventory metadata; excluded from its own digest input to avoid self-reference"
    } else {
        "Machine configuration/schema/tool input or asset; not authored Markdown; relevant contracts separately fingerprinted"
    }
}

fn generate(root: &Path, commit: &str) -> Result<Inventory> {
    let paths = snapshot_paths(root, commit)?;
    let mut result = Inventory {
        schema: "nepl3.doc-inventory/2".into(),
        baseline_commit: commit.into(),
        scope:
            "immutable-commit-baseline; no claim of current completeness or migration acceptance"
                .into(),
        extractor: "pulldown-cmark/0.13.4".into(),
        inline_projection: "nepl3.markdown-inline-projection/1".into(),
        parser_options: markdown::option_names(),
        capability_contracts: Vec::new(),
        pages: Vec::new(),
        rustdoc_sources: Vec::new(),
        exclusions: Vec::new(),
    };
    for path in &paths {
        if path.starts_with("conformance/results/") {
            continue;
        }
        if path.ends_with(".md") {
            let bytes = blob(root, commit, path)?;
            let source =
                std::str::from_utf8(&bytes).map_err(|e| format!("{path}: invalid UTF-8: {e}"))?;
            let (class, canonical, action) = classification(path);
            result.pages.push(Page { file: file(path, &bytes)?, classification: class.into(), canonical_source: canonical, migration_action: action.into(), stable_page_id: None, published_url: None, identity_status: "Unassigned: inventory path is a source locator, not a stable public page ID or deployed URL; old renderer anchors require explicit mapping".into(), structure: markdown::extract(source)? });
        } else if path.ends_with(".rs") {
            result
                .rustdoc_sources
                .push(file(path, &blob(root, commit, path)?)?);
        } else {
            result.exclusions.push(Exclusion {
                path: path.clone(),
                reason: exclusion(path).into(),
            });
        }
    }
    for path in CONTRACTS {
        if !paths.contains(*path) {
            return Err(format!("baseline lacks audited Doc contract: {path}").into());
        }
        result
            .capability_contracts
            .push(file(path, &blob(root, commit, path)?)?);
    }
    Ok(result)
}

fn scoped_path(path: &str) -> bool {
    !path.starts_with("conformance/results/")
        && (path.ends_with(".md") || path.ends_with(".rs") || CONTRACTS.contains(&path))
}

fn delta(root: &Path, baseline: &Inventory) -> Result<Delta> {
    let mut before = BTreeMap::new();
    for file in baseline
        .pages
        .iter()
        .map(|p| &p.file)
        .chain(&baseline.rustdoc_sources)
        .chain(&baseline.capability_contracts)
    {
        before.insert(file.path.clone(), file.sha256.clone());
    }
    let mut after = BTreeMap::new();
    for path in inventory(root)?.iter().filter(|p| scoped_path(p)) {
        if !root.join(path).exists() {
            continue;
        }
        let full = local_path(root, path)?;
        if fs::symlink_metadata(&full)?.file_type().is_symlink() || !full.is_file() {
            return Err(format!("inventory current input must be a regular file: {path}").into());
        }
        if fs::metadata(&full)?.len() > 1024 * 1024 {
            return Err(format!("inventory current input exceeds 1 MiB: {path}").into());
        }
        after.insert(path.clone(), digest(&fs::read(full)?));
    }
    Ok(compare(&before, &after))
}

fn compare(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Delta {
    Delta {
        added: after
            .keys()
            .filter(|p| !before.contains_key(*p))
            .cloned()
            .collect(),
        changed: after
            .iter()
            .filter(|(p, hash)| before.get(*p).is_some_and(|old| old != *hash))
            .map(|(p, _)| p.clone())
            .collect(),
        removed: before
            .keys()
            .filter(|p| !after.contains_key(*p))
            .cloned()
            .collect(),
    }
}

pub(crate) fn write(root: &Path, commit: &str) -> Result<()> {
    let output = generate(root, commit)?;
    for parent in ["doc", "doc/migration", "doc/migration/generated"] {
        local_path(root, parent)?;
        let metadata = fs::symlink_metadata(root.join(parent))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("inventory output parent must be a real directory".into());
        }
    }
    match fs::symlink_metadata(root.join(OUTPUT)) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("inventory output must be a regular file".into());
            }
            local_path(root, OUTPUT)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::write(
        root.join(OUTPUT),
        format!("{}\n", serde_json::to_string_pretty(&output)?),
    )?;
    println!(
        "Wrote immutable documentation baseline {commit}: {} Markdown pages, {} Rust source files. This is not a current-tree or Doc migration acceptance claim.",
        output.pages.len(),
        output.rustdoc_sources.len()
    );
    Ok(())
}

pub(crate) fn check(root: &Path, require_current: bool) -> Result<()> {
    let saved: Inventory = json(root, OUTPUT)?;
    let expected = generate(root, &saved.baseline_commit)?;
    if saved != expected {
        return Err("documentation inventory differs from its immutable baseline; regenerate and independently review omissions/changes".into());
    }
    let changes = delta(root, &saved)?;
    let stale =
        !changes.added.is_empty() || !changes.changed.is_empty() || !changes.removed.is_empty();
    println!(
        "Documentation inventory baseline {} verified: {} Markdown pages, {} Rust-owned API sources. Current delta: {} added, {} changed, {} removed; {}.",
        saved.baseline_commit,
        saved.pages.len(),
        saved.rustdoc_sources.len(),
        changes.added.len(),
        changes.changed.len(),
        changes.removed.len(),
        if stale {
            "current completeness NOT established"
        } else {
            "current source coverage matches baseline, semantic migration remains unvalidated"
        }
    );
    if stale {
        println!("{}", serde_json::to_string(&changes)?);
    }
    if require_current && stale {
        return Err("current documentation inventory is stale; commit and audit a new baseline before claiming current coverage".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;

    #[test]
    fn historical_review_source_is_not_a_documentation_migration_target() {
        assert!(!scoped_path("conformance/results/review/AGENTS.md"));
        assert!(!scoped_path("conformance/results/review/source/lib.rs"));
        assert!(scoped_path("doc/development.md"));
        assert!(scoped_path("crates/foundation/core/src/lib.rs"));
    }

    fn fixture() -> Result<(Fixture, String)> {
        let files = Fixture::new()?;
        files.git()?;
        files.write("README.md", "# Root\n\n[Guide](doc/guide.md)\n")?;
        files.write(
            "doc/guide.md",
            "# Guide\n\n| A | B |\n| - | - |\n| 1 | 2 |\n",
        )?;
        files.write(
            "tools/src/lib.rs",
            "//! Rust documentation stays owned by Rust source.\n",
        )?;
        for path in CONTRACTS {
            files.write(path, "{}\n")?;
        }
        command(files.root(), "git", &["add", "--all"])?;
        command(
            files.root(),
            "git",
            &[
                "-c",
                "user.name=Inventory Test",
                "-c",
                "user.email=inventory@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "synthetic inventory baseline",
            ],
        )?;
        let commit = String::from_utf8(command(files.root(), "git", &["rev-parse", "HEAD"])?)?
            .trim()
            .to_owned();
        fs::create_dir_all(files.root().join("doc/migration/generated"))?;
        write(files.root(), &commit)?;
        Ok((files, commit))
    }

    #[test]
    fn real_checker_rejects_omitted_page_digest_and_structure_tampering() -> Result<()> {
        let (files, _) = fixture()?;
        check(files.root(), true)?;
        let original: serde_json::Value = json(files.root(), OUTPUT)?;
        for mutation in 0..3 {
            let mut value = original.clone();
            match mutation {
                0 => {
                    value["pages"]
                        .as_array_mut()
                        .ok_or("fixture pages missing")?
                        .remove(0);
                }
                1 => value["pages"][0]["file"]["sha256"] = serde_json::json!("0".repeat(64)),
                _ => value["pages"][0]["structure"]["elements"]["heading"] = serde_json::json!(999),
            }
            files.json(OUTPUT, &value)?;
            assert!(check(files.root(), false).is_err());
        }
        Ok(())
    }

    #[test]
    fn real_checker_reports_added_changed_removed_and_rejects_current_claim() -> Result<()> {
        let (files, _) = fixture()?;
        let saved: Inventory = json(files.root(), OUTPUT)?;
        files.write("doc/new.md", "# New\n")?;
        files.write("README.md", "# Changed\n")?;
        fs::remove_file(files.root().join("doc/guide.md"))?;
        assert_eq!(
            delta(files.root(), &saved)?,
            Delta {
                added: vec!["doc/new.md".into()],
                changed: vec!["README.md".into()],
                removed: vec!["doc/guide.md".into()]
            }
        );
        check(files.root(), false)?;
        assert!(check(files.root(), true).is_err());
        Ok(())
    }

    #[test]
    fn commit_reference_and_missing_history_are_not_silently_accepted() -> Result<()> {
        let files = Fixture::new()?;
        files.git()?;
        for bad in [
            "HEAD",
            "--help",
            "25a096b",
            "0000000000000000000000000000000000000000",
        ] {
            assert!(snapshot_paths(files.root(), bad).is_err());
        }
        Ok(())
    }

    #[test]
    fn git_symlink_mode_is_rejected_without_creating_an_os_symlink() -> Result<()> {
        let (files, _) = fixture()?;
        files.write("link-target", "README.md")?;
        let hash = String::from_utf8(command(
            files.root(),
            "git",
            &["hash-object", "-w", "link-target"],
        )?)?
        .trim()
        .to_owned();
        command(
            files.root(),
            "git",
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "120000",
                &hash,
                "doc/link.md",
            ],
        )?;
        command(
            files.root(),
            "git",
            &[
                "-c",
                "user.name=Inventory Test",
                "-c",
                "user.email=inventory@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "synthetic symlink tree entry",
            ],
        )?;
        let commit = String::from_utf8(command(files.root(), "git", &["rev-parse", "HEAD"])?)?
            .trim()
            .to_owned();
        assert!(generate(files.root(), &commit).is_err());
        Ok(())
    }

    #[test]
    fn nonregular_output_and_oversized_current_input_are_rejected() -> Result<()> {
        let (files, commit) = fixture()?;
        fs::remove_file(files.root().join(OUTPUT))?;
        fs::create_dir(files.root().join(OUTPUT))?;
        assert!(write(files.root(), &commit).is_err());
        fs::remove_dir(files.root().join(OUTPUT))?;
        write(files.root(), &commit)?;
        files.write("doc/huge.md", vec![b'x'; 1024 * 1024 + 1])?;
        assert!(check(files.root(), false).is_err());
        Ok(())
    }
}
