//! Real standalone CLI policy, MathML files, notices and failure boundaries.
use nepl3_tools::doc::{export, source};
#[cfg(not(target_family = "wasm"))]
use std::{fs, process::Command};

#[cfg(not(target_family = "wasm"))]
#[test]
fn math_export_cli_policies_preserve_math_annotations_and_code()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("nepl3-math-export-{}", std::process::id()));
    fs::create_dir(&root)?;
    let input = root.join("math.nepld");
    fs::write(
        &input,
        r#"article ja "Math" body
      cons paragraph cons sentence cons math Math frac 1 0 nil nil
      cons display Math label x Sentence "{[字/じ]/character}"
      cons code Math frac 1 0 nil"#,
    )?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for css in ["external", "inline"] {
        for renderer in ["default", "katex-preferred", "mathml-only"] {
            let output = root.join(format!("{css}-{renderer}"));
            let mut run = Command::new(binary);
            run.args(["doc-html", "export", "--css", css]);
            if renderer != "default" {
                run.args(["--math-renderer", renderer]);
            }
            let run = run.arg(&input).arg(&output).output()?;
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            let html = fs::read_to_string(output.join("document.html"))?;
            assert!(html.contains("<mfrac>"));
            assert!(html.contains("display=\"inline\""));
            assert!(html.contains("display=\"block\""));
            assert!(html.contains("class=\"nepl-ruby\""));
            assert!(html.contains("class=\"nepl-anno\""));
            assert!(html.contains("character"));
            assert!(html.contains("<code") && html.contains("nepl-code-"));
            assert!(html.contains("字") && html.contains("じ"));
            assert!(!html.contains("<script"));
            assert!(html.contains("default-src 'none'"));
            assert_eq!(output.join("assets/doc.css").is_file(), css == "external");
            let report: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
            let policy = if renderer == "default" {
                "katex-preferred"
            } else {
                renderer
            };
            assert_eq!(report["options"]["math_renderer"], policy);
            assert_eq!(report["math"]["preference"], policy);
            assert_eq!(report["math"]["katex_adapter_available"], false);
            let occurrences = report["math"]["occurrences"]
                .as_array()
                .ok_or("occurrences")?;
            assert_eq!(occurrences.len(), 2);
            assert_eq!(occurrences[0]["guest_occurrence"], 0);
            assert_eq!(occurrences[1]["guest_occurrence"], 1);
            assert_eq!(occurrences[0]["display"], "inline");
            assert_eq!(occurrences[1]["display"], "block");
            assert_ne!(occurrences[0]["embed"], occurrences[1]["embed"]);
            for occurrence in occurrences {
                assert_eq!(occurrence["representation"], "mathml");
                if renderer == "mathml-only" {
                    assert!(occurrence["fallback"].is_null());
                } else {
                    assert_eq!(occurrence["fallback"], "katex-adapter-unavailable");
                }
            }
            assert_eq!(
                String::from_utf8_lossy(&run.stderr).contains("MathRendererUnavailable"),
                renderer != "mathml-only"
            );
            for file in report["files"].as_array().ok_or("files")? {
                let bytes = fs::read(output.join(file["path"].as_str().ok_or("path")?))?;
                let digest = nepl3_core::source::Digest::of(&bytes)
                    .0
                    .iter()
                    .map(|x| format!("{x:02x}"))
                    .collect::<String>();
                assert_eq!(file["sha256"], digest);
            }
            let prior = fs::read(output.join("document.html"))?;
            let repeated = Command::new(binary)
                .args(["doc-html", "export"])
                .arg(&input)
                .arg(&output)
                .output()?;
            assert!(!repeated.status.success());
            assert_eq!(fs::read(output.join("document.html"))?, prior);
        }
    }
    // Alternate flag order is accepted, with the same explicit policy.
    let run = Command::new(binary)
        .args([
            "doc-html",
            "export",
            "--math-renderer",
            "mathml-only",
            "--css",
            "inline",
        ])
        .arg(&input)
        .arg(root.join("reverse"))
        .output()?;
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let run = Command::new(binary)
        .args(["doc-html", "export", "--math-renderer", "unknown"])
        .arg(&input)
        .arg(root.join("invalid-policy"))
        .output()?;
    assert!(!run.status.success());
    assert!(!root.join("invalid-policy").exists());
    for (name, bad) in [
        (
            "link",
            r#"article en "x" body cons paragraph cons sentence cons link external "https://example.test/" text "x" nil nil nil"#,
        ),
        (
            "asset",
            r#"article en "x" body cons image asset "missing" none "x" none nil"#,
        ),
        (
            "invalid-math",
            r#"article en "x" body cons display Math vector nil nil"#,
        ),
        (
            "incomplete",
            r#"article en "x" body cons display Math frac 1"#,
        ),
    ] {
        fs::write(&input, bad)?;
        let run = Command::new(binary)
            .args(["doc-html", "export", "--math-renderer", "mathml-only"])
            .arg(&input)
            .arg(root.join(name))
            .output()?;
        assert!(!run.status.success());
        assert!(!root.join(name).exists());
    }
    fs::remove_dir_all(root)?;
    Ok(())
}

#[test]
fn math_export_policies_bind_actual_representation_without_tex_evaluation() -> Result<(), String> {
    let compiled = source::compiled()?;
    let input = r#"article en "Math" body cons display Math frac 1 0 nil"#;
    let preferred = export::generate_with_options(
        &compiled,
        input,
        export::CssMode::Inline,
        export::math::Renderer::KatexPreferred,
    )?;
    let only = export::generate_with_options(
        &compiled,
        input,
        export::CssMode::Inline,
        export::math::Renderer::MathmlOnly,
    )?;
    assert_eq!(preferred.html, only.html);
    assert_eq!(
        preferred.math.as_ref().ok_or("report")?.occurrences.len(),
        1
    );
    assert_eq!(
        only.math.as_ref().ok_or("report")?.occurrences[0].fallback,
        None
    );
    let a: serde_json::Value =
        serde_json::from_str(&preferred.manifest).map_err(|e| e.to_string())?;
    let b: serde_json::Value = serde_json::from_str(&only.manifest).map_err(|e| e.to_string())?;
    assert_eq!(
        a["operations"]["prepare_render_serialize"]["diagnostics"].as_u64(),
        Some(1)
    );
    assert_eq!(
        b["operations"]["prepare_render_serialize"]["diagnostics"].as_u64(),
        Some(0)
    );
    Ok(())
}
