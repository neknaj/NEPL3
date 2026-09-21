//! Publish the remaining Markdown specification sources until their Doc migration.
use super::{Result, canonical, escape, hash, overview};
use std::{collections::BTreeMap, path::Path};

pub(super) fn route(source: &str) -> Option<String> {
    let name = source.strip_prefix("doc/spec/")?.strip_suffix(".md")?;
    // The specification inventory uses numbered, portable filenames.
    if name.len() < 4
        || !name.as_bytes()[..2].iter().all(u8::is_ascii_digit)
        || name.as_bytes()[2] != b'-'
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    Some(format!("docs/spec/{name}.html"))
}

pub(super) fn generate(
    root: &Path,
    registry: &canonical::Registry,
    base: &str,
    commit: &str,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let tracked = crate::command(
        root,
        "git",
        &["ls-files", "-z", "--", ":(glob)doc/spec/[0-9][0-9]-*.md"],
    )?;
    let tracked = std::str::from_utf8(&tracked)?;
    let mut files = BTreeMap::new();
    let mut records = Vec::new();
    for source in tracked.split('\0').filter(|p| !p.is_empty()) {
        if registry.pages.iter().any(|p| p.projection == source) {
            continue; // Never render a generated Markdown projection as canonical.
        }
        let route = route(source).ok_or("unsupported Markdown specification path")?;
        if records.len() >= 256 {
            return Err("Markdown specification count limit".into());
        }
        let bytes = canonical::bounded(root, source, 256 * 1024)?;
        let content = overview::render(
            root,
            std::str::from_utf8(&bytes)?,
            source,
            registry,
            base,
            commit,
        )?;
        let html = format!(
            "<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self'; base-uri 'none'; form-action 'none'\"><title>{}</title><link rel=\"stylesheet\" href=\"{}assets/site.css\"></head><body><main><nav aria-label=\"文書\"><a href=\"{}docs/index.html\">仕様書一覧</a></nav>{content}<p>現在の正本: <a href=\"https://github.com/neknaj/NEPL3/blob/{commit}/{}\">Markdown原文</a>。NEPL3dへの移行は別途進めています。</p></main></body></html>",
            escape(source),
            escape(base),
            escape(base),
            escape(source)
        );
        super::insert(&mut files, &route, html.into_bytes())?;
        records.push(serde_json::json!({"source":source,"route":route,"sha256":hash(&bytes)}));
    }
    super::insert(
        &mut files,
        "markdown-manifest.json",
        serde_json::to_vec_pretty(&serde_json::json!({
            "version":1,"source_commit":commit,"renderer":"pulldown-cmark/0.13.4","pages":records
        }))?,
    )?;
    Ok(files)
}

/// Select only registry-declared references. The payload is the exact HTML
/// already generated for this site, while its input digest is the Markdown.
pub(super) fn reference_projections(
    root: &Path,
    registry: &canonical::Registry,
    files: &BTreeMap<String, Vec<u8>>,
    base: &str,
    commit: &str,
) -> Result<Vec<canonical::references::Projection>> {
    let mut projections = Vec::new();
    let receipt: serde_json::Value = serde_json::from_slice(
        files
            .get("markdown-manifest.json")
            .ok_or("missing Markdown receipt")?,
    )?;
    for reference in &registry.files {
        let Some(route) = route(&reference.source) else {
            continue;
        };
        let bytes = files
            .get(&route)
            .ok_or("missing Markdown reference projection")?;
        let source = canonical::bounded(root, &reference.source, 256 * 1024)?;
        let source_sha256 = hash(&source);
        let record = receipt["pages"]
            .as_array()
            .ok_or("missing Markdown records")?
            .iter()
            .find(|record| record["source"].as_str() == Some(&reference.source))
            .ok_or("missing Markdown source receipt")?;
        if record["sha256"].as_str() != Some(&source_sha256)
            || record["route"].as_str() != Some(&route)
        {
            return Err("Markdown reference source changed".into());
        }
        projections.push(canonical::references::Projection {
            source: reference.source.clone(),
            source_sha256,
            route,
            renderer: "nepl3-tools.site-markdown/1; pulldown-cmark/0.13.4".into(),
            context: serde_json::to_string(
                &serde_json::json!({"base":base,"source_commit":commit}),
            )?,
            bytes: bytes.clone(),
        });
    }
    Ok(projections)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_only_unmigrated_sources_and_resolves_relative_links() -> Result<()> {
        let fixture = crate::testing::Fixture::new()?;
        crate::command(fixture.root(), "git", &["init", "-q"])?;
        fixture.write("doc/spec/03-reader.md", "# Reader\n\n[Grammar](04-grammar.md) [Moved](05-document.md) [Other](../other.md)\n\n<script>bad</script>")?;
        fixture.write("doc/spec/04-grammar.md", "# Grammar\n\n日本語の本文")?;
        fixture.write("doc/spec/05-document.md", "GENERATED MUST NOT BE RENDERED")?;
        fixture.write("doc/other.md", "Other")?;
        fixture.write("doc/spec/doc-signatures.md", "Generated signature table")?;
        crate::command(fixture.root(), "git", &["add", "."])?;
        let registry = serde_json::from_str(
            r#"{"version":1,"pages":[{"id":"doc","source":"doc.nepld","projection":"doc/spec/05-document.md","aliases":"aliases.json","route":"docs/moved.html","renderer":"test"}],"files":[{"id":"reader","source":"doc/spec/03-reader.md","route":"sources/03-reader.md"}]}"#,
        )?;
        for base in ["/NEPL3/", "/acceptance/project/"] {
            let files = generate(fixture.root(), &registry, base, "fixture")?;
            assert_eq!(files.len(), 3);
            assert!(!files.contains_key("docs/spec/05-document.html"));
            let html = std::str::from_utf8(&files["docs/spec/03-reader.html"])?;
            assert!(html.contains(&format!("{base}docs/spec/04-grammar.html")));
            assert!(html.contains(&format!("{base}docs/moved.html")));
            assert!(html.contains("blob/fixture/doc/other.md"));
            assert!(html.contains("&lt;script&gt;") && !html.contains("<script>"));
            let selected =
                reference_projections(fixture.root(), &registry, &files, base, "fixture")?;
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].bytes, files["docs/spec/03-reader.html"]);
            assert_eq!(selected[0].source, "doc/spec/03-reader.md");
            let mut missing = files.clone();
            missing.remove("docs/spec/03-reader.html");
            assert!(
                reference_projections(fixture.root(), &registry, &missing, base, "fixture")
                    .is_err()
            );
        }
        fixture.write("doc/spec/03-reader.md", "[escape](../../../outside)")?;
        assert!(generate(fixture.root(), &registry, "/NEPL3/", "fixture").is_err());
        Ok(())
    }
}
