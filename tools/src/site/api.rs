//! Build and include the current workspace's rustdoc, preserving its assets.
use super::{Result, escape, insert};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

#[derive(serde::Serialize)]
struct ApiRoot {
    name: String,
    route: String,
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

fn collect(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut directories = vec![root.to_path_buf()];
    let mut bytes = 0usize;
    let mut entries = 0usize;
    while let Some(directory) = directories.pop() {
        if linked(&fs::symlink_metadata(&directory)?) {
            return Err("linked rustdoc directory".into());
        }
        for entry in fs::read_dir(directory)? {
            entries += 1;
            if entries > 8192 {
                return Err("rustdoc entry limit".into());
            }
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if linked(&metadata) {
                return Err("linked rustdoc entry".into());
            }
            if metadata.is_dir() {
                directories.push(path);
                continue;
            }
            if !metadata.is_file() {
                return Err("non-regular rustdoc file".into());
            }
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("non-UTF-8 rustdoc path")?
                .replace('\\', "/");
            if relative == ".lock" {
                continue;
            } // Cargo's output lock, not published content.
            if files.len() >= 4000 {
                return Err("rustdoc file limit".into());
            }
            let data =
                super::canonical::bounded(root, &relative, (super::MAX_SITE_BYTES - bytes) as u64)?;
            bytes = bytes
                .checked_add(data.len())
                .ok_or("rustdoc size overflow")?;
            insert(&mut files, &format!("api/rust/{relative}"), data)?;
        }
    }
    Ok(files)
}

pub(super) fn generate(
    root: &Path,
    scratch: &Path,
    base: &str,
    commit: &str,
) -> Result<BTreeMap<String, Vec<u8>>> {
    // Cargo doc preserves old outputs. Never admit an existing target directory.
    fs::create_dir(scratch)?;
    let target = fs::canonicalize(scratch)?;
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "doc",
            "--workspace",
            "--no-deps",
            "--locked",
            "--target-dir",
        ])
        .arg(&target)
        .env("RUSTDOCFLAGS", "-D warnings")
        .env_remove("CARGO_ENCODED_RUSTDOCFLAGS")
        .status()?;
    if !status.success() {
        return Err("workspace rustdoc failed".into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&crate::command(
        root,
        "cargo",
        &["metadata", "--no-deps", "--locked", "--format-version", "1"],
    )?)?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing workspace members")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or("missing workspace packages")?;
    let mut files = collect(&target.join("doc"))?;
    let mut roots = Vec::new();
    for package in packages.iter().filter(|p| members.contains(&p["id"])) {
        for target in package["targets"]
            .as_array()
            .ok_or("missing package targets")?
        {
            if !target["kind"]
                .as_array()
                .ok_or("missing target kind")?
                .iter()
                .any(|k| k == "lib")
            {
                continue;
            }
            let name = target["name"].as_str().ok_or("missing crate name")?;
            let route = format!("api/rust/{name}/index.html");
            if !files.contains_key(&route) {
                return Err(format!("missing rustdoc crate: {name}").into());
            }
            roots.push(ApiRoot {
                name: name.into(),
                route,
            });
        }
    }
    if roots.is_empty() {
        return Err("no workspace Rust API roots".into());
    }
    roots.sort_by(|a, b| a.name.cmp(&b.name));
    let links = roots
        .iter()
        .map(|r| {
            format!(
                "<li><a href=\"{}{}\">{}</a></li>",
                escape(base),
                escape(&r.route),
                escape(&r.name)
            )
        })
        .collect::<String>();
    let html = format!(
        "<!doctype html><html lang=\"ja\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>NEPL3 Rust API</title><link rel=\"stylesheet\" href=\"{}assets/site.css\"><main><h1>Rust API</h1><p>同じcommitのworkspaceから生成したAPI文書です。検索にはJavaScriptを使用します。</p><ul>{links}</ul><a href=\"{}docs/index.html\">仕様書一覧</a></main></html>",
        escape(base),
        escape(base)
    );
    insert(&mut files, "api/rust/index.html", html.into_bytes())?;
    let rustdoc = String::from_utf8(crate::command(root, "rustdoc", &["--version"])?)?;
    insert(
        &mut files,
        "api/rust/manifest.json",
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":1,"source_commit":commit,"rustdoc":rustdoc.trim(),"command":["cargo","doc","--workspace","--no-deps","--locked"],"roots":roots}),
        )?,
    )?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_assets_and_source_bytes_but_not_cargo_lock() -> Result<()> {
        let fixture = crate::testing::Fixture::new()?;
        fixture.write("crate/index.html", "<script src=\"../search.js\"></script>")?;
        fixture.write("search.js", "const value = 1;")?;
        fixture.write(".lock", "")?;
        let files = collect(fixture.root())?;
        assert_eq!(files.len(), 2);
        assert_eq!(files["api/rust/search.js"], b"const value = 1;");
        Ok(())
    }
}
