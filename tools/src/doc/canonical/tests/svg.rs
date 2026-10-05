use super::*;
use nepl3_core::source::Digest;

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10" stroke="#000" fill="none"/></svg>"##;

fn manifest() -> serde_json::Value {
    json!({"version":1,"pages":[
        {"id":"main","source":"main.nepld","projection":"nested/main.md","route":"nested/main.md","aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"},
        {"id":"other","source":"other.nepld","projection":"other.md","route":"other.md","aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"}
    ],"files":[{"id":"figure","source":"figure.svg","route":"assets/figure.svg"}]})
}

fn fixture() -> Result<Fixture> {
    let f = Fixture::new()?;
    f.write("main.nepld", r#"article en "Image" body cons image asset "figure" none "{image/alt note}" some "{caption/note}" nil"#)?;
    f.write(
        "other.nepld",
        r#"article en "Other" body cons paragraph cons "No image here." nil nil"#,
    )?;
    f.write("aliases.json", "[]")?;
    f.write("figure.svg", SVG)?;
    f.json("pages.json", &manifest())?;
    Ok(f)
}

fn hash(bytes: &[u8]) -> String {
    Digest::of(bytes)
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn svg_batch_exports_exact_bytes_and_used_image_identity() -> Result<()> {
    let f = fixture()?;
    let output = f.root().join("first");
    footnotes_manifest(&f.root().join("pages.json"), &output)?;
    let text = fs::read_to_string(output.join("nested/main.md"))?;
    assert!(text.contains("![image](<../assets/figure.svg>)"));
    assert!(!text.contains("alt note"));
    let bytes = fs::read(output.join("assets/figure.svg"))?;
    assert_eq!(bytes, SVG.as_bytes());
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
    let records = receipt["files"].as_array().ok_or("files missing")?;
    assert_eq!(records.len(), 3);
    let asset = records
        .iter()
        .find(|v| v["path"] == "assets/figure.svg")
        .ok_or("asset missing")?;
    assert_eq!(asset["kind"], "static-svg");
    assert_eq!(asset["sha256"], hash(&bytes));
    assert_eq!(asset["bytes"], bytes.len());
    let used = &receipt["page_dependencies"][0]["images"][0];
    assert_eq!(used["asset_id"], "figure");
    assert_eq!(used["route"], "assets/figure.svg");
    assert_eq!(used["sha256"], hash(&bytes));
    assert_eq!(receipt["page_dependencies"][1]["images"], json!([]));
    f.write("figure.svg", SVG.replace("#000", "#123"))?;
    let changed = f.root().join("changed");
    footnotes_manifest(&f.root().join("pages.json"), &changed)?;
    assert_ne!(text, fs::read_to_string(changed.join("nested/main.md"))?);
    assert_eq!(
        fs::read(output.join("other.md"))?,
        fs::read(changed.join("other.md"))?
    );
    assert!(!output.join("main.nepld").exists());
    Ok(())
}

#[test]
fn svg_batch_rejects_path_conflicts_before_output_publication() -> Result<()> {
    let f = fixture()?;
    for route in [
        "nested/main.md/figure.svg",
        "MANIFEST.JSON/figure.svg",
        "assets/figure.svg/child.svg",
    ] {
        let mut m = manifest();
        if route.starts_with("assets/") {
            m["pages"][1]["projection"] = json!("ASSETS/FIGURE.SVG/other.md");
            m["pages"][1]["route"] = json!("ASSETS/FIGURE.SVG/other.md");
            m["files"][0]["route"] = json!("assets/figure.svg");
        } else {
            m["files"][0]["route"] = json!(route);
        }
        f.json("pages.json", &m)?;
        assert!(load(f.root(), "pages.json").is_err());
        assert!(
            footnotes_manifest(&f.root().join("pages.json"), &f.root().join("invalid")).is_err()
        );
        assert!(!f.root().join("invalid").exists());
    }
    Ok(())
}

#[test]
fn svg_batch_rejects_invalid_or_unused_assets_without_output() -> Result<()> {
    let f = fixture()?;
    for invalid in [
        "<svg/>",
        "<!DOCTYPE svg><svg/>",
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><script/></svg>",
    ] {
        f.write("figure.svg", invalid)?;
        assert!(
            footnotes_manifest(&f.root().join("pages.json"), &f.root().join("invalid")).is_err()
        );
        assert!(!f.root().join("invalid").exists());
    }
    fs::write(f.root().join("figure.svg"), [0xff])?;
    assert!(footnotes_manifest(&f.root().join("pages.json"), &f.root().join("invalid")).is_err());
    assert!(!f.root().join("invalid").exists());
    f.write("figure.svg", SVG)?;
    f.write("main.nepld", r#"article en "Empty" body nil"#)?;
    let error = footnotes_manifest(&f.root().join("pages.json"), &f.root().join("unused"))
        .err()
        .ok_or("unused accepted")?;
    assert!(error.to_string().contains("unused registered SVG"));
    assert!(!f.root().join("unused").exists());
    Ok(())
}

#[test]
fn svg_output_route_validation_and_drop_use_bounded_stack() -> Result<()> {
    let mut m = manifest();
    let path = format!("{}p.md", "a/".repeat(2044));
    assert_eq!(path.len(), 4092);
    m["pages"][0]["projection"] = json!(path);
    m["pages"][0]["route"] = m["pages"][0]["projection"].clone();
    let raw = serde_json::to_vec(&m)?;
    let worker = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || parse_registry(&raw).is_ok())?;
    assert!(
        worker
            .join()
            .map_err(|_| "route validation thread panicked")?
    );
    Ok(())
}
