//! Host-only repository checks. These checks never execute language operations.

mod contract;
mod dependency;
mod documentation;
mod evidence;
mod repository;
mod task;

#[cfg(test)]
mod testing;

use std::{fs, path::Path, process::Command};

/// Host command errors; these are not the language runtime diagnostic contract.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(crate) fn read(root: &Path, path: &str) -> Result<String> {
    let path = repository::local_path(root, path)?;
    Ok(fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?)
}

pub(crate) fn json<T: serde::de::DeserializeOwned>(root: &Path, path: &str) -> Result<T> {
    let text = read(root, path)?;
    repository::json::validate(&text).map_err(|error| format!("{path}: {error}"))?;
    Ok(serde_json::from_str(&text).map_err(|error| format!("{path}: {error}"))?)
}

pub(crate) fn command(root: &Path, program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "{program} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}

/// Check repository metadata and hygiene, without running language acceptance.
pub fn check(root: &Path) -> Result<()> {
    let files = repository::check(root)?;
    let (tasks, status) = task::load(root)?;
    contract::check(root)?;
    contract::reader::check(root)?;
    contract::engine::check(root)?;
    contract::grammar::check(root)?;
    dependency::check(root, &status.implemented_crates)?;
    task::generate(root, &tasks, &status, false)?;
    documentation::check(root, false)?;
    println!(
        "Repository checks passed: {files} files, {} tasks, {} acceptance status entries. Runtime acceptance was not run.",
        tasks.tasks.len(),
        status.acceptance.len()
    );
    Ok(())
}

/// Verify a snapshot-bound documentation inventory and report current differences.
pub fn doc_inventory(root: &Path, commit: Option<&str>, require_current: bool) -> Result<()> {
    match commit {
        Some(commit) => documentation::write(root, commit),
        None => documentation::check(root, require_current),
    }
}

/// Generate task documents or verify exact bytes against their source data.
pub fn tasks(root: &Path, write: bool) -> Result<()> {
    let (tasks, status) = task::load(root)?;
    task::generate(root, &tasks, &status, write)?;
    println!(
        "Task documents {}.",
        if write { "written" } else { "are current" }
    );
    Ok(())
}

/// Regenerate the foundational package descriptor and verify it using the core registry.
pub fn foundation(root: &Path) -> Result<()> {
    contract::foundation::write(root)
}

/// Generate the production Grammar AST descriptor, whose shape comes from forms.json.
pub fn grammar(root: &Path) -> Result<()> {
    contract::grammar::write(root)
}

/// Regenerate the registered reader envelope descriptor projection.
pub fn reader(root: &Path) -> Result<()> {
    contract::reader::write(root)
}

/// Print a reproducible identity for the current evidence input inventory.
pub fn evidence_identity(root: &Path) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence::identity(root)?)?
    );
    Ok(())
}

/// Regenerate the registered engine package descriptor projection.
pub fn engine(root: &Path) -> Result<()> {
    contract::engine::write(root)
}
pub mod bootstrap;
