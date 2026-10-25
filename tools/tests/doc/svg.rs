use std::{fs, process::Command};
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 0L10 0L5 10Z' fill='none' stroke='#000'/></svg>";
fn run(
    root: &std::path::Path,
    css: &str,
    mode: &str,
    out: &str,
) -> std::io::Result<std::process::Output> {
    Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
        .args(["doc-html", "svg", "--css", css, "--svg", mode])
        .arg(root.join("input.nepld"))
        .arg(root.join("assets.json"))
        .arg(root.join(out))
        .output()
}
#[test]
fn svg_modes_and_input_failures() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-svg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let source = "article ja \"SVG\" body cons image asset \"triangle\" none \"{[三角形/さんかくけい]/triangle}\" some \"図1\" cons paragraph cons sentence cons image asset \"triangle\" none \"文中の図\" nil nil nil";
    let source = format!(
        "{}cons paragraph cons code Doc article en \"Guest\" body nil cons sentence cons math Math frac 1 2 nil cons display Math frac 3 4 cons sentence cons link external \"https://example.org/figure?q=1&view=2\" text \"Source\" nil nil nil",
        source.strip_suffix("nil").ok_or("article end")?
    );
    fs::write(root.join("input.nepld"), source)?;
    fs::write(root.join("figure.svg"), SVG)?;
    let spec = r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#;
    fs::write(root.join("assets.json"), spec)?;
    for css in ["external", "inline"] {
        for mode in ["external", "embedded"] {
            let dir = format!("{css}-{mode}");
            let result = run(&root, css, mode, &dir)?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let html = fs::read_to_string(root.join(&dir).join("document.html"))?;
            assert_eq!(html.matches("<img ").count(), 3);
            assert!(html.contains("href=\"https://example.org/figure?q=1&amp;view=2\""));
            assert_eq!(html.matches("<mfrac>").count(), 2);
            assert!(html.contains("display=\"inline\""));
            assert!(html.contains("display=\"block\""));
            assert!(html.contains("class=\"nepl-code-marker\""));
            assert!(html.contains("alt=\"三角形\""));
            assert!(html.contains("<figcaption"));
            assert_eq!(html.matches("<details ").count(), 1);
            assert!(html.contains("<summary>Full size / 縮小制限を解除</summary>"));
            assert!(html.contains("nepl-image-original"));
            assert!(html.contains("</figure><details class=\"nepl-image-details\">"));
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(&dir).join("manifest.json"))?)?;
            assert_eq!(manifest["resources"][0]["mode"], mode);
            if mode == "external" {
                let path = format!(
                    "assets/{}.svg",
                    manifest["resources"][0]["sha256"]
                        .as_str()
                        .ok_or("digest")?
                );
                assert_eq!(fs::read(root.join(&dir).join(&path))?, SVG.as_bytes());
                assert!(html.contains(&format!("src=\"{path}\"")));
                assert!(html.contains("img-src 'self';"));
                assert!(!html.contains("img-src data:"));
            } else {
                assert!(html.contains("img-src data:;"));
                assert!(!html.contains("img-src 'self'"));
                assert!(html.contains("PHN2ZyB4bWxucz0naHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmcnIHZpZXdCb3g9JzAgMCAxMCAxMCc+PHBhdGggZD0nTTAgMEwxMCAwTDUgMTBaJyBmaWxsPSdub25lJyBzdHJva2U9JyMwMDAnLz48L3N2Zz4="));
                assert!(
                    !manifest["files"]
                        .as_array()
                        .ok_or("files")?
                        .iter()
                        .any(|f| f["mime"] == "image/svg+xml")
                );
            }
            assert!(!run(&root, css, mode, &dir)?.status.success());
        }
    }
    for (n, bad) in [
        spec.replace("image/svg+xml", "text/plain"),
        spec.replace("figure.svg", "../figure.svg"),
        spec.replace("triangle", "wrong"),
        spec.replace("\"version\":1", "\"version\":2"),
        spec.replace("figure.svg", "missing.svg"),
    ]
    .iter()
    .enumerate()
    {
        fs::write(root.join("assets.json"), bad)?;
        assert!(
            !run(&root, "inline", "embedded", &format!("fail{n}"))?
                .status
                .success()
        );
        assert!(!root.join(format!("fail{n}")).exists());
    }
    fs::write(root.join("assets.json"), spec)?;
    fs::write(
        root.join("figure.svg"),
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><script/></svg>",
    )?;
    assert!(
        !run(&root, "inline", "embedded", "bad-svg")?
            .status
            .success()
    );
    assert!(!root.join("bad-svg").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn conditional_disclosure_uses_intrinsic_dimensions_in_both_css_modes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-svg-size-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    fs::write(
        root.join("input.nepld"),
        "article ja \"SVG\" body cons image asset \"triangle\" none \"図\" none nil",
    )?;
    fs::write(
        root.join("assets.json"),
        r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#,
    )?;
    for (i, (dimensions, threshold)) in [
        ("width='96pt' height='270pt'", Some("128px")),
        ("width='100px' height='360px'", Some("100px")),
        ("width='100' height='361'", None),
        ("width='100'", None),
        ("", None),
    ]
    .iter()
    .enumerate()
    {
        fs::write(
            root.join("figure.svg"),
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10' {dimensions}><path d='M0 0L10 10'/></svg>"
            ),
        )?;
        for css in ["external", "inline"] {
            for mode in ["external", "embedded"] {
                let out = format!("{i}-{css}-{mode}");
                let result = run(&root, css, mode, &out)?;
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let html = fs::read_to_string(root.join(&out).join("document.html"))?;
                let stylesheet = if css == "external" {
                    fs::read_to_string(root.join(&out).join("assets/doc.css"))?
                } else {
                    html.split_once("<style>")
                        .ok_or("style")?
                        .1
                        .split_once("</style>")
                        .ok_or("style end")?
                        .0
                        .to_owned()
                };
                assert_eq!(
                    stylesheet.contains("@container nepl-image"),
                    threshold.is_some()
                );
                if let Some(width) = threshold {
                    assert!(stylesheet.contains(&format!("min-width:{width}")));
                }
                let manifest: serde_json::Value =
                    serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
                let hash = nepl3_core::source::Digest::of(stylesheet.as_bytes())
                    .0
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                assert_eq!(manifest["stylesheet"]["sha256"], hash);
                assert!(html.contains("data-nepl-id=\"image-"));
                if css == "external" {
                    assert!(
                        manifest["files"]
                            .as_array()
                            .ok_or("files")?
                            .iter()
                            .any(|f| f["path"] == "assets/doc.css" && f["sha256"] == hash)
                    );
                }
            }
        }
    }
    fs::write(
        root.join("input.nepld"),
        "article ja \"SVG\" body cons paragraph cons sentence cons image asset \"triangle\" none \"図\" nil nil nil",
    )?;
    fs::write(
        root.join("figure.svg"),
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100' viewBox='0 0 10 10'><path d='M0 0L10 10'/></svg>",
    )?;
    let result = run(&root, "inline", "embedded", "inline-image")?;
    assert!(result.status.success());
    let html = fs::read_to_string(root.join("inline-image/document.html"))?;
    assert!(!html.contains("@container nepl-image"));
    assert!(!html.contains("<details"));
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn local_path_glyphs_export_in_every_asset_mode() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-glyphs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let glyphs = "<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='100' height='60' viewBox='0 0 100 60'><defs><path id='g' d='M0 0L10 0L10 20Z'/></defs><g transform='matrix(1 0 0 1 10 10)'><use href='#g' x='5' y='7'/><use xlink:href='#g' x='40' y='7'/></g></svg>";
    fs::write(
        root.join("input.nepld"),
        "article ja \"SVG\" body cons image asset \"glyphs\" none \"文字\" none nil",
    )?;
    fs::write(root.join("figure.svg"), glyphs)?;
    fs::write(
        root.join("assets.json"),
        r#"{"version":1,"assets":[{"id":"glyphs","source":"figure.svg","mime":"image/svg+xml"}]}"#,
    )?;
    for css in ["external", "inline"] {
        for mode in ["external", "embedded"] {
            let dir = format!("{css}-{mode}");
            let result = run(&root, css, mode, &dir)?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let html = fs::read_to_string(root.join(&dir).join("document.html"))?;
            assert_eq!(html.matches("<img ").count(), 2);
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(&dir).join("manifest.json"))?)?;
            assert_eq!(manifest["resources"][0]["mode"], mode);
            if mode == "external" {
                let digest = manifest["resources"][0]["sha256"]
                    .as_str()
                    .ok_or("digest")?;
                assert_eq!(
                    fs::read_to_string(root.join(&dir).join(format!("assets/{digest}.svg")))?,
                    glyphs
                );
            } else {
                let encoded = html
                    .split("src=\"data:image/svg+xml;base64,")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .ok_or("embedded SVG")?;
                let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                let mut decoded = Vec::new();
                let mut buffer = 0u32;
                let mut bits = 0u32;
                for byte in encoded.bytes().take_while(|byte| *byte != b'=') {
                    let value = alphabet
                        .iter()
                        .position(|candidate| *candidate == byte)
                        .ok_or("base64 digit")?;
                    buffer = (buffer << 6) | value as u32;
                    bits += 6;
                    if bits >= 8 {
                        bits -= 8;
                        decoded.push((buffer >> bits) as u8);
                        buffer &= (1 << bits) - 1;
                    }
                }
                assert_eq!(decoded, glyphs.as_bytes());
            }
        }
    }
    fs::write(root.join("figure.svg"), glyphs.replace("#g", "other.svg#g"))?;
    assert!(
        !run(&root, "inline", "embedded", "invalid")?
            .status
            .success()
    );
    assert!(!root.join("invalid").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn sentence_alt_and_new_mixed_cases_export_with_explicit_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-svg-alt-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    fs::write(root.join("figure.svg"), SVG)?;
    fs::write(
        root.join("assets.json"),
        r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#,
    )?;
    let source = r#"article ja "SVG" body cons image asset "triangle" none Sentence "{[漢/かん]/note}" some "caption" cons paragraph cons sentence cons image asset "triangle" none Sentence "inline & <alt>" nil cons code Doc article en "guest" body nil cons sentence cons math Math frac 1 2 nil nil nil"#;
    fs::write(root.join("input.nepld"), source)?;
    for css in ["external", "inline"] {
        for mode in ["external", "embedded"] {
            let out = format!("{css}-{mode}");
            let result = run(&root, css, mode, &out)?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let html = fs::read_to_string(root.join(&out).join("document.html"))?;
            assert!(html.contains("alt=\"漢\""));
            assert!(html.contains("alt=\"inline &amp; &lt;alt&gt;\""));
            assert!(!html.contains("かん") && !html.contains("alt=\"note\""));
            assert!(html.contains("<figcaption") && html.contains("caption"));
            assert!(html.contains("nepl-code-marker") && html.contains("<mfrac>"));
            assert!(!html.contains("<script"));
            assert!(root.join(&out).join("manifest.json").is_file());
            let manifest: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(root.join(&out).join("manifest.json"))?)?;
            assert_eq!(manifest["sentence_alt"]["policy"], "BaseOnly");
            assert_eq!(manifest["sentence_alt"]["owners"], 2);
            assert!(
                manifest["scope"]
                    .as_str()
                    .ok_or("scope")?
                    .contains("restricted to image alt")
            );
            assert_eq!(manifest["options"]["svg"], mode);

            assert!(!run(&root, css, mode, &out)?.status.success());
        }
    }
    for (index, mixed) in [
        source.replace("article ja \"SVG\"", "article ja Sentence \"SVG\""),
        source.replace("some \"caption\"", "some Sentence \"caption\""),
        source.replace("cons paragraph", "cons paragraph cons parallel cons variant en Sentence \"body\" cons variant ja \"旧文\" nil nil cons paragraph"),
        source.replace(
            "cons paragraph",
            "cons paragraph cons Sentence \"body\" nil cons paragraph",
        ),
    ]
    .iter()
    .enumerate()
    {
        fs::write(root.join("input.nepld"), mixed)?;
        let out = format!("mixed-{index}");
        let result = run(&root, "inline", "embedded", &out)?;
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let manifest: serde_json::Value = serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
        assert_eq!(manifest["sentence_svg"]["contract"], "nepl3.local-doc-svg.sentence/1");
        assert!(manifest["sentence_svg"]["display_occurrences"].as_u64().ok_or("display occurrences")? > 0);
    }
    let unresolved = source.replace(
        "Sentence \"{[漢/かん]/note}\"",
        "Sentence sentence cons math frac 1 2 nil",
    );
    fs::write(root.join("input.nepld"), unresolved)?;
    let result = run(&root, "inline", "embedded", "unresolved")?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Unresolved"));
    assert!(!root.join("unresolved").exists());
    fs::write(root.join("input.nepld"), source)?;
    fs::write(root.join("figure.svg"), "<svg><script/></svg>")?;
    assert!(
        !run(&root, "inline", "embedded", "invalid-svg")?
            .status
            .success()
    );
    assert!(!root.join("invalid-svg").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mixed_sentence_svg_cli_uses_real_math_and_code_in_all_file_modes()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-mixed-svg-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    fs::write(root.join("figure.svg"), SVG)?;
    fs::write(
        root.join("assets.json"),
        r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#,
    )?;
    let source = r#"article en Sentence "Title" body cons image asset "triangle" none Sentence "{[漢/かん]/note}" some Sentence "Caption" cons paragraph cons sentence cons image asset "triangle" none Sentence "inline & <alt>" nil cons code Doc article en "guest" body nil cons sentence cons math Math frac 1 2 nil cons Sentence sentence cons text "Body " cons math frac 3 4 nil cons Sentence sentence cons document math Math frac 5 6 nil cons Sentence sentence cons math frac 3 4 nil nil nil"#;
    fs::write(root.join("input.nepld"), source)?;
    for css in ["external", "inline"] {
        for mode in ["external", "embedded"] {
            let out = format!("{css}-{mode}");
            let result = run(&root, css, mode, &out)?;
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let html = fs::read_to_string(root.join(&out).join("document.html"))?;
            assert!(html.contains("Title") && html.contains("Caption") && html.contains("Body"));
            assert!(
                html.contains("alt=\"漢\"") && html.contains("alt=\"inline &amp; &lt;alt&gt;\"")
            );
            assert!(html.contains("nepl-code-marker") && html.contains("<mfrac>"));
            assert_eq!(html.matches("<math").count(), 4);
            assert_eq!(html.matches("<mfrac>").count(), 4);
            assert!(!html.contains("<script") && !html.contains("かん"));
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(&out).join("manifest.json"))?)?;
            assert_eq!(
                m["sentence_svg"]["contract"],
                "nepl3.local-doc-svg.sentence/1"
            );
            assert_eq!(m["sentence_svg"]["assets"], "root Article only");
            assert_eq!(m["sentence_alt"]["owners"], 2);
            assert!(
                m["scope"]
                    .as_str()
                    .ok_or("scope")?
                    .contains("planned typed Sentence")
            );
            assert_eq!(m["options"]["parallel"], "Rows");
            assert_eq!(m["options"]["svg"], mode);
            assert!(m.get("display_scope").is_none());
            assert!(!run(&root, css, mode, &out)?.status.success());
        }
    }
    // A mixed document with only legacy alt text still has explicit mixed scope.
    fs::write(
        root.join("input.nepld"),
        source
            .replace("Sentence \"{[漢/かん]/note}\"", "\"Alt\"")
            .replace("Sentence \"inline & <alt>\"", "\"inline\""),
    )?;
    let result = run(&root, "inline", "embedded", "legacy-alt")?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("legacy-alt/manifest.json"))?)?;
    assert_eq!(m["sentence_alt"]["owners"], 0);
    assert!(
        m["scope"]
            .as_str()
            .ok_or("scope")?
            .contains("planned typed Sentence")
    );
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn mixed_sentence_svg_failures_never_publish_an_output_directory()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-mixed-svg-failures-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let base = r#"article en Sentence "Title" body cons image asset "triangle" none Sentence "Alt" none nil"#;
    let spec = r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#;
    let append = |tail: &str| -> Result<String, Box<dyn std::error::Error>> {
        Ok(format!(
            "{}{}",
            base.strip_suffix("nil").ok_or("body")?,
            tail
        ))
    };
    let duplicate_spec = r#"{"version":1,"assets":[{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"},{"id":"triangle","source":"figure.svg","mime":"image/svg+xml"}]}"#;
    let cases: Vec<(String, String, String, &str)> = vec![
        (
            base.into(),
            r#"{"version":1,"assets":[]}"#.into(),
            SVG.into(),
            "Missing",
        ),
        (base.into(), duplicate_spec.into(), SVG.into(), "Duplicate"),
        (
            base.into(),
            spec.replace("triangle", "unused"),
            SVG.into(),
            "Unused",
        ),
        (
            base.into(),
            spec.into(),
            "<svg><script/></svg>".into(),
            "InvalidSvg",
        ),
        (
            base.replace(
                "asset \"triangle\" none",
                &format!("asset \"triangle\" some \"{}\"", "0".repeat(64)),
            ),
            spec.into(),
            SVG.into(),
            "Digest",
        ),
        (
            base.replace(
                "Sentence \"Alt\"",
                "Sentence sentence cons math frac 1 2 nil",
            ),
            spec.into(),
            SVG.into(),
            "Unresolved",
        ),
        (
            append(
                r#"cons paragraph cons Sentence sentence cons document image asset "triangle" none "guest" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "Missing",
        ),
        (
            append(
                r#"cons paragraph cons sentence cons link external "javascript:alert(1)" text "bad" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "InvalidExternalUri",
        ),
        (
            append(
                r#"cons paragraph cons sentence cons link page "other" none text "bad" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "Unsupported",
        ),
        (
            append(
                r#"cons paragraph cons sentence cons link relative "other.nepld" none text "bad" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "Unsupported",
        ),
        (
            append(
                r#"cons paragraph cons Sentence sentence cons document ref absent text "bad" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "UnresolvedLabel",
        ),
        (
            append(
                r#"cons paragraph cons Sentence sentence cons document anchor dup text "first" nil cons Sentence sentence cons document anchor dup text "second" nil nil nil"#,
            )?,
            spec.into(),
            SVG.into(),
            "DuplicateLabel",
        ),
    ];
    for (index, (source, manifest, svg, expected)) in cases.into_iter().enumerate() {
        fs::write(root.join("input.nepld"), source)?;
        fs::write(root.join("assets.json"), manifest)?;
        fs::write(root.join("figure.svg"), svg)?;
        let out = format!("failure-{index}");
        let result = run(&root, "inline", "embedded", &out)?;
        assert!(!result.status.success(), "{index}");
        let message = String::from_utf8_lossy(&result.stderr);
        assert!(
            message.contains(expected),
            "{index}: expected {expected}, got {message}"
        );
        assert!(!root.join(&out).exists(), "{index}: partial output");
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
