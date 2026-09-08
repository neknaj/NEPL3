//! Repository document source selection and exact generated-projection checks.
//! This host adapter never infers migration approval from successful rendering.
use crate::{Result, doc::projection::annotated::host, repository};
use serde::Deserialize;
use std::{collections::BTreeSet, fs, io::Read, path::Path};

#[cfg(test)]
mod tests;

use crate::doc::projection::annotated::host::RENDERER;
const MAX_REGISTRY: u64 = 1_048_576;

fn portable_path(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'/' | b'.'))
        && name.split('/').all(|part| {
            let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && !part.ends_with('.')
                && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !((base.starts_with("COM") || base.starts_with("LPT"))
                    && base.len() == 4
                    && matches!(base.as_bytes()[3], b'1'..=b'9'))
        })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    pub pages: Vec<Page>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: String,
    pub source: String,
    pub projection: String,
    pub aliases: String,
    pub route: String,
    pub renderer: String,
}

fn bounded(root: &Path, relative: &str, limit: u64) -> Result<Vec<u8>> {
    // Require real regular repository files; aliases or platform links cannot
    // turn a canonical record into a write/read outside its declared path.
    let path = repository::local_path(root, relative)?;
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(format!("canonical path contains a symlink: {relative}").into());
        }
    }
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(format!("canonical path is not a regular file: {relative}").into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("canonical input exceeds limit: {relative}").into());
    }
    Ok(bytes)
}

/// Validate the source-selection manifest, without claiming semantic approval.
pub fn load(root: &Path, manifest: &str) -> Result<Registry> {
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let text = std::str::from_utf8(&raw)?;
    repository::json::validate(text)?;
    let registry: Registry = serde_json::from_str(text)?;
    if registry.version != 1 || registry.pages.is_empty() || registry.pages.len() > 1000 {
        return Err("unsupported or empty canonical registry".into());
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut routes = BTreeSet::new();
    for page in &registry.pages {
        if page.id.is_empty()
            || !page
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !ids.insert(&page.id)
            || page.renderer != RENDERER
        {
            return Err("invalid canonical page identity or renderer".into());
        }
        for (name, suffix) in [
            (&page.source, ".nepld"),
            (&page.projection, ".md"),
            (&page.aliases, ".json"),
        ] {
            if !portable_path(name)
                || !name.starts_with("doc/")
                || !name.ends_with(suffix)
                || !paths.insert(name.to_ascii_lowercase())
            {
                return Err(format!("invalid or repeated canonical file path: {name}").into());
            }
        }
        // Routes are portable URL paths, not host filesystem paths.
        if !portable_path(&page.route)
            || !page.route.ends_with(".html")
            || !routes.insert(page.route.to_ascii_lowercase())
        {
            return Err("invalid or repeated canonical page route".into());
        }
    }
    Ok(registry)
}

/// Regenerate from the Doc source and compare exact bytes; never adopt or write
/// a changed Markdown projection as a new expectation.
pub fn check(root: &Path, manifest: &str) -> Result<()> {
    let registry = load(root, manifest)?;
    let compiled = super::source::compiled()?;
    for page in registry.pages {
        let source = bounded(root, &page.source, super::export::MAX_SOURCE_BYTES)?;
        let aliases = bounded(root, &page.aliases, MAX_REGISTRY)?;
        repository::json::validate(std::str::from_utf8(&aliases)?)?;
        let expected = host::generate(
            &compiled,
            &page.source,
            std::str::from_utf8(&source)?,
            &aliases,
        )?;
        let actual = bounded(root, &page.projection, 2_097_152)?;
        if actual != expected.as_bytes() {
            return Err(format!(
                "stale generated Markdown: {} (edit {})",
                page.projection, page.source
            )
            .into());
        }
        println!("Canonical Doc projection is current: {}", page.id);
    }
    Ok(())
}

/// Export the registered Doc sources through the same prepared page-set backend.
/// The existing Markdown path remains the logical namespace for relative links;
/// the physical source and its digest are recorded separately by the generator.
pub fn html(root: &Path, manifest: &str, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let registry = load(root, manifest)?;
    let mut inputs = Vec::new();
    let mut total = 0u64;
    for page in registry.pages {
        let bytes = bounded(root, &page.source, super::export::MAX_SOURCE_BYTES)?;
        total = total.checked_add(bytes.len() as u64).ok_or("SourceLimit")?;
        if total > super::export::MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        let source = String::from_utf8(bytes)?;
        inputs.push((
            super::export::pages::Entry {
                id: page.id,
                source: page.projection,
                input: Some(page.source),
                route: page.route,
            },
            source,
        ));
    }
    let generated = super::export::pages::generate(&super::source::compiled()?, &inputs)?;
    super::export::pages::write_generated(generated, output)
}
