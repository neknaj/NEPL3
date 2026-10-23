//! Output-only legacy Markdown endpoints. Never semantic PageSet registrations.
use super::{OutputRoutes, Registry, Result, portable_path, projection, references};
use crate::doc::export::pages::GeneratedPages;
use serde::Deserialize;
use std::collections::BTreeSet;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    pub page: String,
    pub route: String,
}

pub(super) fn validate(registry: &Registry) -> Result<()> {
    if registry.compatibility.len() > 128 {
        return Err("CompatibilityCountLimit".into());
    }
    let mut output = OutputRoutes::default();
    output.insert("manifest.json")?;
    for page in &registry.pages {
        output.insert(&page.route)?;
    }
    for file in &registry.files {
        output.insert(&file.route)?;
    }
    let mut seen = BTreeSet::new();
    for copy in &registry.compatibility {
        let page = registry
            .pages
            .iter()
            .find(|p| p.id == copy.page)
            .ok_or("unknown compatibility page")?;
        if page.renderer != projection::RENDERER
            || !portable_path(&copy.route)
            || !copy.route.starts_with("sources/")
            || !copy.route.ends_with(".md")
            || !seen.insert(&copy.route)
        {
            return Err("invalid compatibility copy".into());
        }
        output.insert(&copy.route)?;
    }
    Ok(())
}

pub(super) fn append(
    registry: &Registry,
    markdown: projection::Generated,
    generated: &mut GeneratedPages,
) -> Result<()> {
    validate(registry)?;
    if markdown.manifest.len() > 8 * 1024 * 1024 || generated.manifest.len() > 8 * 1024 * 1024 {
        return Err("CompatibilityReceiptLimit".into());
    }
    let mut output = OutputRoutes::default();
    output.insert("manifest.json")?;
    for path in generated.files.keys() {
        output.insert(path)?;
    }
    let mut copies = Vec::new();
    let mut records = Vec::new();
    let mut total = 0usize;
    for copy in &registry.compatibility {
        let page = registry
            .pages
            .iter()
            .find(|p| p.id == copy.page)
            .ok_or("unknown compatibility page")?;
        let (_, bytes) = markdown
            .files
            .iter()
            .find(|(path, _)| path == &page.projection)
            .ok_or("missing generated compatibility projection")?;
        if bytes.len() > 262_144 {
            return Err("CompatibilityFileLimit".into());
        }
        total = total.checked_add(bytes.len()).ok_or("CompatibilityLimit")?;
        if total > 2_097_152 {
            return Err("CompatibilityLimit".into());
        }
        output.insert(&copy.route)?;
        records.push(serde_json::json!({
            "page": copy.page, "source": page.source,
            "projection": page.projection, "route": copy.route,
            "renderer": page.renderer, "bytes": bytes.len(),
            "sha256": references::hash(bytes.as_bytes()),
        }));
        copies.push((copy.route.clone(), bytes.as_bytes().to_vec()));
    }
    let mut receipt: serde_json::Value = serde_json::from_str(&generated.manifest)?;
    let files = receipt["files"]
        .as_array_mut()
        .ok_or("missing HTML file receipt")?;
    for (route, bytes) in &copies {
        files.push(serde_json::json!({
            "path": route, "bytes": bytes.len(), "sha256": references::hash(bytes),
            "mime": "text/markdown; charset=utf-8",
        }));
    }
    receipt["compatibility_markdown"] = serde_json::json!({
        "scope": "Output-only fresh canonical Markdown, not semantic pages or raw source. Relative links remain repository-relative. Bounded host snapshot/copy work is separate from renderer runtime Usage.",
        "snapshot_limits": {"pages":128,"passive_files":128,"source_bytes":10_000_000,"alias_bytes":1_048_576,"passive_bytes":2_097_152},
        "copy_limits": {"files":128,"file_bytes":262_144,"total_bytes":2_097_152},
        "copies": records,
        "markdown_receipt": serde_json::from_str::<serde_json::Value>(&markdown.manifest)?,
    });
    let manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    if manifest.len() > 8 * 1024 * 1024 {
        return Err("CompatibilityReceiptLimit".into());
    }
    // Publish only after all routes, sizes and receipt serialization succeeded.
    generated.files.extend(copies);
    generated.manifest = manifest;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;
    use serde_json::json;

    fn registry() -> serde_json::Value {
        json!({"version":1,"pages":[{"id":"sample","source":"doc/sample.nepld",
            "projection":"doc/sample.md","aliases":"doc/aliases.json",
            "route":"docs/sample.html","renderer":projection::RENDERER}],
            "compatibility":[{"page":"sample","route":"sources/sample.md"}]})
    }
    fn fixture(value: &serde_json::Value) -> Result<Fixture> {
        let f = Fixture::new()?;
        f.json("doc/canonical.json", value)?;
        f.write(
            "doc/sample.nepld",
            "article en \"Title\" body cons paragraph cons \"Current body.\" cons sentence cons link relative \"sample.md\" none text \"Canonical\" nil nil nil",
        )?;
        f.write("doc/aliases.json", "[]")?;
        Ok(f)
    }
    #[test]
    fn compatibility_copy_is_fresh_without_repository_projection() -> Result<()> {
        let f = fixture(&registry())?;
        let first = super::super::generate_html(f.root(), "doc/canonical.json")?;
        let bytes = first.files.get("sources/sample.md").ok_or("missing copy")?;
        let html = std::str::from_utf8(first.files.get("docs/sample.html").ok_or("missing HTML")?)?;
        assert!(html.contains("href=\"sample.html\""));
        assert!(std::str::from_utf8(bytes)?.contains(r"Current body\."));
        f.write("doc/sample.md", "deliberately stale and untrusted")?;
        let second = super::super::generate_html(f.root(), "doc/canonical.json")?;
        assert_eq!(second.files.get("sources/sample.md"), Some(bytes));
        let receipt: serde_json::Value = serde_json::from_str(&second.manifest)?;
        assert_eq!(
            receipt["compatibility_markdown"]["copies"][0]["sha256"],
            references::hash(bytes)
        );
        assert_eq!(
            receipt["registered_files"].as_array().ok_or("files")?.len(),
            0
        );
        assert_eq!(receipt["pages"].as_array().ok_or("pages")?.len(), 1);
        assert!(
            receipt["files"]
                .as_array()
                .ok_or("outputs")?
                .iter()
                .any(|x| x["path"] == "sources/sample.md"
                    && x["mime"] == "text/markdown; charset=utf-8")
        );
        Ok(())
    }
    #[test]
    fn compatibility_registration_rejects_wrong_page_renderer_and_routes() -> Result<()> {
        for route in [
            "../sample.md",
            "sources/../sample.md",
            "sources/a.md/child.md",
            "manifest.json",
            "docs/sample.md",
            "sources/sample.html",
        ] {
            let mut value = registry();
            value["compatibility"][0]["route"] = json!(route);
            if route == "sources/a.md/child.md" {
                value["files"] = json!([{"id":"raw","source":"doc/raw.md","route":"sources/a.md"}]);
            }
            assert!(
                super::super::parse_registry(&serde_json::to_vec(&value)?).is_err(),
                "{route}"
            );
        }
        for field in ["page", "route"] {
            let mut value = registry();
            value["compatibility"][0][field] = json!("missing");
            assert!(super::super::parse_registry(&serde_json::to_vec(&value)?).is_err());
        }
        let mut value = registry();
        value["compatibility"]
            .as_array_mut()
            .ok_or("copies")?
            .push(json!({"page":"sample","route":"sources/SAMPLE.md"}));
        assert!(super::super::parse_registry(&serde_json::to_vec(&value)?).is_err());
        let mut value = registry();
        value["pages"][0]["renderer"] = json!(super::super::RENDERER);
        assert!(super::super::parse_registry(&serde_json::to_vec(&value)?).is_err());
        Ok(())
    }
    #[test]
    fn compatibility_failures_do_not_create_output() -> Result<()> {
        for phase in ["output_limits", "html_output_limits"] {
            let mut value = registry();
            let mut limits = serde_json::to_value(
                crate::doc::export::pages::resources::OutputLimits::default(),
            )?;
            limits["work"] = json!(0);
            value[phase] = limits;
            let f = fixture(&value)?;
            let output = f.root().join("out");
            assert!(super::super::html(f.root(), "doc/canonical.json", &output).is_err());
            assert!(!output.exists());
        }
        Ok(())
    }
    #[test]
    fn compatibility_append_rejects_collision_and_limits_atomically() -> Result<()> {
        let value = registry();
        let reg = super::super::parse_registry(&serde_json::to_vec(&value)?)?;
        for (collision, size) in [(true, 1), (false, 262_145)] {
            let mut generated = GeneratedPages {
                files: Default::default(),
                manifest: "{\"files\":[]}".into(),
                provenance: None,
            };
            if collision {
                generated
                    .files
                    .insert("sources/SAMPLE.md".into(), b"existing".to_vec());
            }
            let before = generated.files.clone();
            let receipt = generated.manifest.clone();
            let markdown = projection::Generated {
                files: vec![("doc/sample.md".into(), "x".repeat(size))],
                manifest: "{}".into(),
            };
            assert!(append(&reg, markdown, &mut generated).is_err());
            assert_eq!(generated.files, before);
            assert_eq!(generated.manifest, receipt);
        }
        Ok(())
    }
    #[test]
    fn compatibility_aggregate_copy_limit_is_not_per_file_only() -> Result<()> {
        let mut value = registry();
        value["compatibility"] = (0..9)
            .map(|i| json!({"page":"sample","route":format!("sources/{i}.md")}))
            .collect();
        let reg = super::super::parse_registry(&serde_json::to_vec(&value)?)?;
        let mut generated = GeneratedPages {
            files: Default::default(),
            manifest: "{\"files\":[]}".into(),
            provenance: None,
        };
        let markdown = projection::Generated {
            files: vec![("doc/sample.md".into(), "x".repeat(262_144))],
            manifest: "{}".into(),
        };
        assert!(append(&reg, markdown, &mut generated).is_err());
        assert!(generated.files.is_empty());
        assert_eq!(generated.manifest, "{\"files\":[]}");
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn compatibility_input_symlink_is_rejected_without_output() -> Result<()> {
        let f = fixture(&registry())?;
        std::fs::rename(
            f.root().join("doc/sample.nepld"),
            f.root().join("doc/actual.nepld"),
        )?;
        std::os::unix::fs::symlink("actual.nepld", f.root().join("doc/sample.nepld"))?;
        let output = f.root().join("out");
        assert!(super::super::html(f.root(), "doc/canonical.json", &output).is_err());
        assert!(!output.exists());
        Ok(())
    }
    #[test]
    fn compatibility_markdown_keeps_passive_identity_in_mixed_site() -> Result<()> {
        let mut value = registry();
        value["files"] = json!([{"id":"guide","source":"doc/guide.md","route":"sources/guide.md"}]);
        let f = fixture(&value)?;
        f.write("doc/guide.md", "Original passive guide")?;
        f.write("doc/sample.nepld","article en \"Title\" body cons paragraph cons sentence cons link relative \"guide.md\" none text \"Guide\" nil nil nil")?;
        let projection = references::Projection {
            source: "doc/guide.md".into(),
            source_sha256: references::hash(b"Original passive guide"),
            route: "docs/guide.html".into(),
            renderer: "fixture".into(),
            context: "fixture".into(),
            bytes: b"<h1>Projected guide</h1>".to_vec(),
        };
        let out = super::super::generate_html_with_projections(
            f.root(),
            "doc/canonical.json",
            &[projection],
        )?;
        let copy = std::str::from_utf8(out.files.get("sources/sample.md").ok_or("copy")?)?;
        assert!(copy.contains(r"guide\.md"));
        assert!(!copy.contains("guide.html"));
        let html = std::str::from_utf8(out.files.get("docs/sample.html").ok_or("html")?)?;
        assert!(html.contains("href=\"guide.html\""));
        assert_eq!(
            out.files.get("docs/guide.html"),
            Some(&b"<h1>Projected guide</h1>".to_vec())
        );
        let receipt: serde_json::Value = serde_json::from_str(&out.manifest)?;
        assert_eq!(
            receipt["registered_files"][0]["source_sha256"],
            references::hash(b"Original passive guide")
        );
        assert_eq!(
            receipt["registered_files"][0]["sha256"],
            references::hash(b"<h1>Projected guide</h1>")
        );
        assert!(
            receipt["compatibility_markdown"]["markdown_receipt"]["page_dependencies"]
                .to_string()
                .contains("doc/guide.md")
        );
        Ok(())
    }
    #[test]
    fn compatibility_count_and_receipt_limits_are_atomic() -> Result<()> {
        let mut value = registry();
        value["compatibility"] = (0..129)
            .map(|i| json!({"page":"sample","route":format!("sources/{i}.md")}))
            .collect();
        assert!(super::super::parse_registry(&serde_json::to_vec(&value)?).is_err());
        let reg = super::super::parse_registry(&serde_json::to_vec(&registry())?)?;
        for oversized in [true, false] {
            let receipt = if oversized {
                "{\"files\":[]}".into()
            } else {
                serde_json::to_string(&json!({"files":[],"padding":"x".repeat(6*1024*1024)}))?
            };
            let md_receipt = if oversized {
                serde_json::to_string(&json!({"padding": "x".repeat(8 * 1024 * 1024)}))?
            } else {
                serde_json::to_string(&json!({"padding":"x".repeat(3*1024*1024)}))?
            };
            let mut generated = GeneratedPages {
                files: Default::default(),
                manifest: receipt.clone(),
                provenance: None,
            };
            let markdown = projection::Generated {
                files: vec![("doc/sample.md".into(), "body".into())],
                manifest: md_receipt,
            };
            let error = append(&reg, markdown, &mut generated)
                .err()
                .ok_or("expected receipt size failure")?;
            assert_eq!(error.to_string(), "CompatibilityReceiptLimit");
            assert!(generated.files.is_empty());
            assert_eq!(generated.manifest, receipt);
        }
        Ok(())
    }
}
