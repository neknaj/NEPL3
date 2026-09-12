//! Static host composition of production Doc pages. No deployment side effects.
use crate::{
    Result,
    doc::{canonical, export::pages::GeneratedPages},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read, path::Path};

const MAX_SITE_BYTES: usize = 32 * 1024 * 1024;

fn check_size(payload: usize, manifest: usize) -> Result<()> {
    if payload.checked_add(manifest).ok_or("site size overflow")? > MAX_SITE_BYTES {
        return Err("site output limit".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SiteConfig {
    version: u32,
    base_path: String,
}

impl SiteConfig {
    fn validate(&self) -> Result<()> {
        let base = &self.base_path;
        if self.version != 1 || base.len() > 1024 || !base.starts_with('/') || !base.ends_with('/')
        {
            return Err("invalid SiteConfig version or base path".into());
        }
        if base != "/"
            && base[1..base.len() - 1].split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            })
        {
            return Err("base path must contain plain URL segments".into());
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct FileIdentity {
    path: String,
    bytes: usize,
    sha256: String,
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Serialize)]
struct RendererIdentity {
    executable_sha256: String,
    build_rustc: &'static str,
}

fn renderer_identity() -> Result<RendererIdentity> {
    let mut file = std::fs::File::open(std::env::current_exe()?)?;
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("executable size overflow")?;
        if total > 1024 * 1024 * 1024 {
            return Err("executable identity size limit".into());
        }
        digest.update(&buffer[..count]);
    }
    Ok(RendererIdentity {
        executable_sha256: digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        build_rustc: env!("NEPL3_BUILD_RUSTC"),
    })
}

fn identities(files: &BTreeMap<String, Vec<u8>>) -> Vec<FileIdentity> {
    files
        .iter()
        .map(|(path, bytes)| FileIdentity {
            path: path.clone(),
            bytes: bytes.len(),
            sha256: hash(bytes),
        })
        .collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn insert(files: &mut BTreeMap<String, Vec<u8>>, name: &str, bytes: Vec<u8>) -> Result<()> {
    let folded = name.to_ascii_lowercase();
    if files.keys().any(|path| {
        let path = path.to_ascii_lowercase();
        path == folded
            || path.starts_with(&format!("{folded}/"))
            || folded.starts_with(&format!("{path}/"))
    }) {
        return Err(format!("site route collision: {name}").into());
    }
    files.insert(name.into(), bytes);
    Ok(())
}

fn compose(
    config: &SiteConfig,
    registry: &canonical::Registry,
    mut pages: GeneratedPages,
    commit: &str,
    design: &str,
    renderer: &RendererIdentity,
) -> Result<GeneratedPages> {
    config.validate()?;
    let mut links = String::new();
    let mut entries: Vec<_> = registry.pages.iter().collect();
    entries.sort_by(|a, b| a.route.cmp(&b.route));
    for page in entries {
        if !pages.files.contains_key(&page.route) {
            return Err("missing generated page".into());
        }
        links.push_str(&format!(
            "<li><a href=\"{}{}\">{}</a></li>\n",
            escape(&config.base_path),
            escape(&page.route),
            escape(&page.id)
        ));
    }
    let html = include_str!("../../../site/index.html")
        .replace("{{BASE}}", &escape(&config.base_path))
        .replace("{{PAGES}}", &links);
    insert(&mut pages.files, "index.html", html.as_bytes().to_vec())?;
    insert(&mut pages.files, "docs/index.html", html.into_bytes())?;
    insert(
        &mut pages.files,
        "assets/site.css",
        include_bytes!("../../../site/site.css").to_vec(),
    )?;
    insert(&mut pages.files, ".nojekyll", Vec::new())?;
    insert(
        &mut pages.files,
        "doc-manifest.json",
        pages.manifest.into_bytes(),
    )?;
    let build = serde_json::json!({
        "version":1, "capability":"docs-only", "source_commit":commit,
        "design":design, "base_path":config.base_path,
        "runtime_identity":null, "site_renderer":"nepl3-tools.site/1", "renderer":renderer,
        "source_identity_scope":"input checkout; renderer executable identified separately",
        "files":identities(&pages.files),
        "scope":"Registered canonical Doc pages; no Playground, deploy receipt or full T19/T20 acceptance"
    });
    insert(
        &mut pages.files,
        "build.json",
        serde_json::to_vec_pretty(&build)?,
    )?;
    let size = pages
        .files
        .values()
        .try_fold(0usize, |n, b| n.checked_add(b.len()))
        .ok_or("site size overflow")?;
    pages.manifest = serde_json::to_string_pretty(&serde_json::json!({
        "version":1,"files":identities(&pages.files),"source_commit":commit
    }))?;
    check_size(size, pages.manifest.len())?;
    Ok(pages)
}

/// Build one checked static artifact. Output must be fresh; no deploy occurs.
pub fn build(root: &Path, config: &str, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let config_path = config;
    let config_bytes = canonical::bounded(root, config_path, 16 * 1024)?;
    let config_text = std::str::from_utf8(&config_bytes)?;
    crate::repository::json::validate(config_text)?;
    let config: SiteConfig = serde_json::from_str(config_text)?;
    config.validate()?;
    crate::command(
        root,
        "git",
        &["ls-files", "--error-unmatch", "--", config_path],
    )?;
    for (path, embedded) in [
        (
            "site/index.html",
            include_bytes!("../../../site/index.html").as_slice(),
        ),
        (
            "site/site.css",
            include_bytes!("../../../site/site.css").as_slice(),
        ),
        (
            "crates/languages/doc/html/assets/doc.css",
            nepl3_doc_html::STYLESHEET.as_bytes(),
        ),
    ] {
        if canonical::bounded(root, path, 1024 * 1024)? != embedded {
            return Err(
                format!("embedded asset differs from checkout; rebuild tools: {path}").into(),
            );
        }
    }
    let commit = String::from_utf8(crate::command(root, "git", &["rev-parse", "HEAD"])?)?
        .trim()
        .to_owned();
    crate::command(root, "git", &["diff", "--quiet", "HEAD", "--"])?;
    let renderer = renderer_identity()?;
    let tasks: serde_json::Value = crate::json(root, "design/tasks.json")?;
    let design = tasks
        .get("design")
        .and_then(|v| v.as_str())
        .ok_or("missing design identity")?;
    let registry = canonical::load(root, "doc/canonical.json")?;
    let inputs = [
        "doc/canonical.json",
        "design/tasks.json",
        "site/index.html",
        "site/site.css",
        "crates/languages/doc/html/assets/doc.css",
    ];
    for input in inputs
        .into_iter()
        .chain(registry.pages.iter().map(|page| page.source.as_str()))
    {
        crate::command(root, "git", &["ls-files", "--error-unmatch", "--", input])?;
    }
    let generated = canonical::generate_html(root, "doc/canonical.json")?;
    let generated = compose(&config, &registry, generated, &commit, design, &renderer)?;
    let after = String::from_utf8(crate::command(root, "git", &["rev-parse", "HEAD"])?)?;
    if after.trim() != commit {
        return Err("source commit changed during site generation".into());
    }
    crate::command(root, "git", &["diff", "--quiet", "HEAD", "--"])?;
    crate::doc::export::pages::write_generated(generated, output)
}

#[cfg(test)]
mod tests;
