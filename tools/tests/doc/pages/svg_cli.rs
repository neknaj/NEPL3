//! Actual native CLI tests; no process-spawning assertion runs on WASI.
use std::{fs, process::Command};
fn root(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!(
        "nepl-svg-pages-cli-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&path)?;
    Ok(path)
}
const A: &str = r#"article en Sentence "One" body cons image asset "logo" none Sentence "[漢/かん]" none cons paragraph cons parallel cons variant en Sentence "English display" cons variant ja Sentence "Japanese display" nil cons Sentence sentence cons math frac 1 2 cons document math Math frac 3 4 cons document link page "two" some "target" text "Next" nil cons code Math frac 5 6 nil cons display Math frac 7 8 nil"#;
const B: &str = r#"article en "Two" body cons image asset "logo" none "Two image" none cons section target "Target" body cons paragraph cons sentence cons math Math frac 9 10 nil nil nil nil"#;
#[test]
fn svg_pages_cli_exports_actual_sources_with_explicit_selection()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("modes")?;
    fs::write(root.join("a.nepld"), A)?;
    fs::write(root.join("b.nepld"), B)?;
    for (name, width) in [("a", 100), ("b", 400)] {
        fs::write(
            root.join(format!("{name}.svg")),
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>"
            ),
        )?;
    }
    let manifest = root.join("pages.json");
    for (n, (display, en, ja)) in [
        (serde_json::json!({"mode":"rows"}), true, true),
        (serde_json::json!({"mode":"columns"}), true, true),
        (
            serde_json::json!({"mode":"single","language":"fr","fallbacks":["ja"]}),
            false,
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let value = serde_json::json!({"version":1,"display":display,"pages":[{"id":"one","source":"a.nepld","route":"deep/a.html"},{"id":"two","source":"b.nepld","route":"deep/b.html"}],"assets":[{"page":"one","assets":[{"id":"logo","source":"a.svg","mime":"image/svg+xml"}]},{"page":"two","assets":[{"id":"logo","source":"b.svg","mime":"image/svg+xml"}]}]});
        fs::write(&manifest, serde_json::to_vec(&value)?)?;
        let out = root.join(format!("out-{n}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages-svg", "--math-renderer", "mathml-only"])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let html = fs::read_to_string(out.join("deep/a.html"))?;
        assert!(html.contains("data:image/svg+xml;base64,"));
        assert!(html.contains("img-src data:;"));
        assert!(html.contains("<math"));
        assert!(html.contains("nepl-code-marker"));
        assert_eq!(html.contains("English display"), en);
        assert_eq!(html.contains("Japanese display"), ja);
        let bytes = fs::read(out.join("manifest.json"))?;
        let receipt: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            receipt["source_input"]["math_renderer"],
            "mathml-only-explicit"
        );
        assert_eq!(receipt["katex_generation"], false);
        let math = receipt["source_input"]["math"].as_array().ok_or("math")?;
        assert_eq!(math.len(), 2);
        assert_eq!(math[0].as_array().ok_or("first")?.len(), 3);
        assert_eq!(math[1].as_array().ok_or("second")?.len(), 1);
        for page in math {
            for record in page.as_array().ok_or("page")? {
                assert_eq!(record["selection_reason"], "ExplicitMathMLOnly");
                assert!(record["fallback_reason"].is_null());
            }
        }
        let again = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages-svg", "--math-renderer", "mathml-only"])
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(!again.status.success());
        assert_eq!(fs::read(out.join("manifest.json"))?, bytes);
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
#[test]
fn svg_pages_cli_never_silently_downgrades_or_enables_retry()
-> Result<(), Box<dyn std::error::Error>> {
    let root = root("flags")?;
    fs::write(root.join("one.nepld"), "article en \"No math\" body nil")?;
    let manifest = root.join("pages.json");
    fs::write(
        &manifest,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"one","source":"one.nepld","route":"one.html"}],"assets":[]}),
        )?,
    )?;
    for (n, args) in [
        vec!["doc-html", "pages-svg"],
        vec![
            "doc-html",
            "pages-svg",
            "--math-renderer",
            "katex-preferred",
        ],
        vec![
            "doc-html",
            "pages-svg",
            "--interactive-limits",
            "limits.json",
        ],
        vec!["doc-html", "pages-svg", "--svg", "external"],
    ]
    .into_iter()
    .enumerate()
    {
        let out = root.join(format!("bad-{n}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(args)
            .arg(&manifest)
            .arg(&out)
            .output()?;
        assert!(!run.status.success());
        assert!(!out.exists());
        assert!(String::from_utf8_lossy(&run.stderr).contains("not implemented"));
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
