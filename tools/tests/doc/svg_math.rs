//! Native process test: mixed Math and explicitly registered static SVG assets.
use std::{fs, path::Path, process::Command};
const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='10pt' height='10pt' viewBox='0 0 10 10'><path d='M0 0L10 0L5 10Z' fill='none' stroke='#000'/></svg>";
const DOC: &str = r#"article ja "図と数式" body
 cons image asset "a" none "図" none
 cons paragraph cons sentence cons math Math frac 1 0 nil nil
 cons code Math frac 1 0
 cons display Math label x Sentence "{[字/じ]/character}"
 cons image asset "b" none "図" none
 cons paragraph cons sentence cons image asset "a" none "図" nil nil nil"#;
fn run(
    root: &Path,
    css: &str,
    svg: &str,
    math: &str,
    out: &str,
) -> std::io::Result<std::process::Output> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    c.args(["doc-html", "svg", "--css", css, "--svg", svg]);
    if math != "default" {
        c.args(["--math-renderer", math]);
    }
    c.arg(root.join("input.nepld"))
        .arg(root.join("assets.json"))
        .arg(root.join(out))
        .output()
}
fn hex(bytes: &[u8]) -> String {
    nepl3_core::source::Digest::of(bytes)
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn b64(bytes: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(alphabet[((n >> 18) & 63) as usize] as char);
        out.push(alphabet[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            alphabet[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            alphabet[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
#[test]
fn svg_math_cli_matrix_keeps_hashes_csp_occurrences_and_dedup()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "nepl3-svg-math-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    fs::write(root.join("input.nepld"), DOC)?;
    fs::write(root.join("figure.svg"), SVG)?;
    let spec = serde_json::json!({"version":1,"assets":[
        {"id":"a","source":"figure.svg","mime":"image/svg+xml"},
        {"id":"b","source":"figure.svg","mime":"image/svg+xml"}
    ]});
    fs::write(root.join("assets.json"), serde_json::to_vec(&spec)?)?;
    for css in ["external", "inline"] {
        for svg in ["external", "embedded"] {
            for math in ["default", "katex-preferred", "mathml-only"] {
                let name = format!("{css}-{svg}-{math}");
                let out = root.join(&name);
                let result = run(&root, css, svg, math, &name)?;
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                let html = fs::read_to_string(out.join("document.html"))?;
                let report: serde_json::Value =
                    serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
                assert!(html.contains("<mfrac>") && html.contains("display=\"block\""));
                assert!(
                    html.contains("class=\"nepl-ruby\"") && html.contains("class=\"nepl-anno\"")
                );
                assert!(html.contains("<code") && html.contains("nepl-code-"));
                // Each block image retains its full-size disclosure image; the inline one is single.
                assert_eq!(html.matches("<img ").count(), 5);
                assert!(!html.contains("<script"));
                assert!(html.contains("default-src 'none';"));
                let occurrences = report["math"]["occurrences"]
                    .as_array()
                    .ok_or("occurrences")?;
                assert_eq!(occurrences.len(), 2);
                assert_eq!(occurrences[0]["guest_occurrence"], 0);
                assert_eq!(occurrences[1]["guest_occurrence"], 2); // Code is ordinal 1.
                assert_eq!(occurrences[0]["display"], "inline");
                assert_eq!(occurrences[1]["display"], "block");
                for occurrence in occurrences {
                    assert_eq!(occurrence["representation"], "mathml");
                    if math == "mathml-only" {
                        assert!(occurrence["fallback"].is_null());
                    } else {
                        assert_eq!(occurrence["fallback"], "katex-adapter-unavailable");
                    }
                }
                assert_eq!(report["options"]["svg"], svg);
                assert_eq!(
                    report["options"]["math_renderer"],
                    if math == "default" {
                        "katex-preferred"
                    } else {
                        math
                    }
                );
                assert_eq!(
                    String::from_utf8_lossy(&result.stderr).contains("MathRendererUnavailable"),
                    math != "mathml-only"
                );
                assert_eq!(report["resources"].as_array().ok_or("resources")?.len(), 2);
                assert_eq!(
                    report["resources"][0]["sha256"],
                    report["resources"][1]["sha256"]
                );
                let files = report["files"].as_array().ok_or("files")?;
                assert_eq!(
                    files
                        .iter()
                        .filter(|f| f["mime"] == "image/svg+xml")
                        .count(),
                    usize::from(svg == "external")
                );
                for file in files {
                    let bytes = fs::read(out.join(file["path"].as_str().ok_or("path")?))?;
                    assert_eq!(file["sha256"], hex(&bytes));
                }
                if svg == "embedded" {
                    assert!(html.contains("img-src data:;"));
                    assert!(!html.contains("img-src 'self'"));
                    assert_eq!(
                        html.matches(&format!(
                            "src=\"data:image/svg+xml;base64,{}\"",
                            b64(SVG.as_bytes())
                        ))
                        .count(),
                        5
                    );
                } else {
                    assert!(html.contains("img-src 'self';"));
                    assert!(!html.contains("img-src data:"));
                    assert_eq!(
                        fs::read_to_string(
                            out.join(format!("assets/{}.svg", hex(SVG.as_bytes())))
                        )?,
                        SVG
                    );
                }
                let stylesheet = if css == "inline" {
                    let value = html
                        .split_once("<style>")
                        .ok_or("style")?
                        .1
                        .split_once("</style>")
                        .ok_or("style end")?
                        .0
                        .to_string();
                    assert!(html.contains(&format!(
                        "'sha256-{}'",
                        b64(&nepl3_core::source::Digest::of(value.as_bytes()).0)
                    )));
                    assert!(!out.join("assets/doc.css").exists());
                    value
                } else {
                    fs::read_to_string(out.join("assets/doc.css"))?
                };
                assert_eq!(report["stylesheet"]["sha256"], hex(stylesheet.as_bytes()));
                assert_eq!(report["font"]["bundled"], false);
                let repeated = run(&root, css, svg, math, &name)?;
                assert!(!repeated.status.success());
                assert_eq!(fs::read_to_string(out.join("document.html"))?, html);
            }
        }
    }
    // Alias-only duplication leaves document bytes and charged external output unchanged.
    fs::write(
        root.join("input.nepld"),
        DOC.replace("asset \"b\"", "asset \"a\""),
    )?;
    fs::write(
        root.join("assets.json"),
        serde_json::to_vec(&serde_json::json!({"version":1,"assets":[spec["assets"][0].clone()]}))?,
    )?;
    let result = run(&root, "inline", "external", "mathml-only", "single-id")?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read(root.join("single-id/document.html"))?,
        fs::read(root.join("inline-external-mathml-only/document.html"))?
    );
    let one: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("single-id/manifest.json"))?)?;
    let two: serde_json::Value = serde_json::from_slice(&fs::read(
        root.join("inline-external-mathml-only/manifest.json"),
    )?)?;
    assert_eq!(
        one["operations"]["prepare_render_serialize"]["output_bytes"],
        two["operations"]["prepare_render_serialize"]["output_bytes"]
    );
    fs::write(root.join("input.nepld"), DOC)?;
    for (name, bad) in [
        (
            "missing",
            serde_json::json!({"version":1,"assets":[spec["assets"][0].clone()]}),
        ),
        (
            "duplicate",
            serde_json::json!({"version":1,"assets":[spec["assets"][0].clone(),spec["assets"][0].clone()]}),
        ),
        (
            "unused",
            serde_json::json!({"version":1,"assets":[spec["assets"][0].clone(),spec["assets"][1].clone(),{"id":"c","source":"figure.svg","mime":"image/svg+xml"}]}),
        ),
        (
            "path",
            serde_json::json!({"version":1,"assets":[{"id":"a","source":"../figure.svg","mime":"image/svg+xml"}]}),
        ),
        (
            "mime",
            serde_json::json!({"version":1,"assets":[{"id":"a","source":"figure.svg","mime":"text/plain"}]}),
        ),
    ] {
        fs::write(root.join("assets.json"), serde_json::to_vec(&bad)?)?;
        assert!(
            !run(&root, "inline", "embedded", "mathml-only", name)?
                .status
                .success()
        );
        assert!(!root.join(name).exists());
    }
    fs::write(root.join("assets.json"), serde_json::to_vec(&spec)?)?;
    assert!(
        !run(&root, "inline", "embedded", "invalid", "bad-renderer")?
            .status
            .success()
    );
    assert!(!root.join("bad-renderer").exists());
    fs::write(
        root.join("input.nepld"),
        DOC.replace("frac 1 0", "vector nil"),
    )?;
    assert!(
        !run(
            &root,
            "inline",
            "embedded",
            "katex-preferred",
            "invalid-math"
        )?
        .status
        .success()
    );
    assert!(!root.join("invalid-math").exists());
    fs::write(root.join("input.nepld"), DOC)?;
    fs::write(
        root.join("figure.svg"),
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><script/></svg>",
    )?;
    assert!(
        !run(&root, "inline", "embedded", "mathml-only", "unsafe")?
            .status
            .success()
    );
    assert!(!root.join("unsafe").exists());
    fs::remove_dir_all(root)?;
    Ok(())
}
