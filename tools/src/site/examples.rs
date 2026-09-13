//! Static example publication. Source bytes are never parsed or executed here.
#[cfg(test)]
mod tests;
use super::{Result, escape, hash, insert};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    profile: String,
    examples: Vec<Entry>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source: String,
    language: String,
    category: String,
}

fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b".-_".contains(&v))
        && value != "."
        && value != ".."
}

pub(super) fn generate(root: &Path, base: &str, commit: &str) -> Result<BTreeMap<String, Vec<u8>>> {
    let raw = crate::doc::canonical::bounded(root, "site/examples.json", 128 * 1024)?;
    crate::repository::json::validate(std::str::from_utf8(&raw)?)?;
    let catalog: Catalog = serde_json::from_slice(&raw)?;
    if catalog.version != 1 || catalog.examples.is_empty() || catalog.examples.len() > 1000 {
        return Err("invalid example catalog".into());
    }
    let profile = crate::doc::canonical::bounded(root, &catalog.profile, 128 * 1024)?;
    crate::repository::json::validate(std::str::from_utf8(&profile)?)?;
    let profile_value: serde_json::Value = serde_json::from_slice(&profile)?;
    let aliases = profile_value["aliases"]
        .as_object()
        .ok_or("profile aliases")?;
    let profile_id = profile_value["id"].as_str().ok_or("profile id")?;
    let mut paths = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut files = BTreeMap::new();
    let mut entries = Vec::new();
    let mut sections = String::new();
    let mut total = 0usize;
    for entry in catalog.examples {
        if !segment(&entry.id)
            || !ids.insert(entry.id.clone())
            || !paths.insert(entry.source.clone())
            || !entry.source.starts_with("examples/")
            || !entry.source.split('/').all(segment)
            || !segment(&entry.language)
            || !segment(&entry.category)
        {
            return Err("invalid or duplicate example identity/path".into());
        }
        let alias = aliases
            .get(&entry.language)
            .and_then(|v| v.as_str())
            .ok_or("example language absent from source profile")?;
        if alias != format!("{}/{}", entry.language, entry.category) {
            return Err("example category differs from source profile".into());
        }
        let data = crate::doc::canonical::bounded(root, &entry.source, 1024 * 1024)?;
        let source = std::str::from_utf8(&data)?;
        // Always publish source as text, including a future guest whose source
        // extension is .html/.svg/.js. Viewing an example must not execute it.
        let artifact = format!("examples/sources/{}.txt", entry.id);
        total = total
            .checked_add(data.len())
            .ok_or("example size overflow")?;
        if total > 4 * 1024 * 1024 {
            return Err("example source limit".into());
        }
        sections.push_str(&format!(
            "<section id=\"{}\"><h2>{}</h2><p>{} / {}</p><p><a download href=\"{}{}\">原文を取得</a></p><pre><code>{}</code></pre></section>\n",
            escape(&entry.id), escape(&entry.id), escape(&entry.language), escape(&entry.category), escape(base), escape(&artifact), escape(source)
        ));
        entries.push(
            serde_json::json!({"id":entry.id,"language":entry.language,"category":entry.category,
            "source":entry.source,"path":artifact,"bytes":data.len(),"sha256":hash(&data),"revision":commit,
            "required_source_profile":profile_id,"execution_available":false}),
        );
        insert(&mut files, &artifact, data)?;
    }
    for path in ["site/examples.json", &catalog.profile]
        .into_iter()
        .chain(paths.iter().map(String::as_str))
    {
        crate::command(root, "git", &["ls-files", "--error-unmatch", "--", path])?;
    }
    let html = format!(
        "<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self'; base-uri 'none'; form-action 'none'\"><title>NEPL3 実例</title><link rel=\"stylesheet\" href=\"{}assets/site.css\"></head><body><main><h1>NEPL3 実例</h1><p><a href=\"{}index.html\">ドキュメント</a></p><p>repositoryの原文です。ここでは実行しません。実行には各言語の処理系と解決済みProfileが必要です。</p>{sections}</main></body></html>\n",
        escape(base),
        escape(base)
    );
    insert(&mut files, "examples/index.html", html.into_bytes())?;
    insert(
        &mut files,
        "examples/manifest.json",
        serde_json::to_vec_pretty(&serde_json::json!({
            "version":1,"capability":"source-view","source_commit":commit,
            "source_profile":{"path":catalog.profile,"id":profile_id,"sha256":hash(&profile),"resolved_runtime":false},
            "catalog_sha256":hash(&raw),"examples":entries
        }))?,
    )?;
    Ok(files)
}
