use super::*;
use nepl3_tools::doc::export::{
    math::Renderer,
    pages::{self, Entry, resources::PhaseLimits},
};

fn input(id: &str, source: &str) -> (Entry, String) {
    (
        Entry {
            id: id.into(),
            source: format!("{id}.nepld"),
            route: format!("{id}/index.html"),
            input: None,
        },
        source.into(),
    )
}

#[test]
fn math_pages_bind_policy_occurrences_links_and_output_hashes() -> Result<(), String> {
    let c = compiled()?;
    let inputs = [
        input(
            "a",
            r#"article en "A" body cons paragraph cons sentence cons link page "b" some "target" text "B" cons math Math frac 1 0 nil nil cons code Math frac 1 0 cons display Math 2 nil"#,
        ),
        input(
            "b",
            r#"article en "B" body cons section target "Target" body cons paragraph cons sentence cons link page "a" none text "A" nil nil cons display Math label 3 Sentence "{[字/じ]/character}" nil nil"#,
        ),
    ];
    let default = pages::generate(&c, &inputs)?;
    let explicit = pages::generate_with_resources_and_math(
        &c,
        &inputs,
        &[],
        PhaseLimits::default(),
        &mut budget(),
        Renderer::MathmlOnly,
    )?;
    assert_eq!(default.files, explicit.files);
    let a = core::str::from_utf8(&default.files["a/index.html"]).map_err(err)?;
    assert_eq!(a.matches("<math").count(), 2);
    assert!(a.contains("<mfrac>") && a.contains("<code>") && a.contains("../b/index.html#"));
    let b_html = core::str::from_utf8(&default.files["b/index.html"]).map_err(err)?;
    for required in [
        "class=\"nepl-ruby\"",
        "class=\"nepl-anno\"",
        "字",
        "じ",
        "character",
        "../a/index.html",
    ] {
        assert!(b_html.contains(required), "missing {required}: {b_html}");
    }
    let m: serde_json::Value = serde_json::from_str(&default.manifest).map_err(err)?;
    let e: serde_json::Value = serde_json::from_str(&explicit.manifest).map_err(err)?;
    assert_eq!(m["identity"], e["identity"]);
    assert_eq!(m["execution_identity"], e["execution_identity"]);
    assert_ne!(
        m["render_execution"]["identity"],
        e["render_execution"]["identity"]
    );
    assert_eq!(m["math_pages"][0]["id"], "a");
    assert_eq!(m["math_pages"][1]["source"], "b.nepld");
    let occurrences = m["math_pages"][0]["math"]["occurrences"]
        .as_array()
        .ok_or("occurrences")?;
    assert_eq!(occurrences.len(), 2);
    assert_eq!(occurrences[0]["guest_occurrence"], 0);
    assert_eq!(occurrences[1]["guest_occurrence"], 2); // Code consumes ordinal1.
    assert_eq!(
        m["math_pages"][1]["math"]["occurrences"][0]["guest_occurrence"],
        0
    );
    assert_eq!(occurrences[0]["fallback"], "katex-adapter-unavailable");
    assert_eq!(
        e["math_pages"][0]["math"]["occurrences"][0]["fallback"],
        serde_json::Value::Null
    );
    for row in m["files"].as_array().ok_or("files")? {
        let path = row["path"].as_str().ok_or("path")?;
        let digest = nepl3_core::source::Digest::of(&default.files[path]);
        let hex = digest
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(row["sha256"], hex);
    }
    Ok(())
}

#[test]
fn math_pages_policy_is_explicit_and_diagnostics_are_shared() -> Result<(), String> {
    let c = compiled()?;
    let inputs = [input(
        "a",
        r#"article en "A" body cons display Math frac 1 0 nil"#,
    )];
    let mut limits = budget().limits();
    limits.diagnostics = 0;
    let mut b = Budget::new(limits);
    assert!(
        pages::generate_with_resources_and_math(
            &c,
            &inputs,
            &[],
            PhaseLimits::default(),
            &mut b,
            Renderer::KatexPreferred
        )
        .is_err()
    );
    assert_eq!(b.poll(), Err(StopReason::DiagnosticLimit));
    let only = pages::generate_with_resources_and_math(
        &c,
        &inputs,
        &[],
        PhaseLimits::default(),
        &mut Budget::new(limits),
        Renderer::MathmlOnly,
    )?;
    assert_eq!(only.math_reports[0].occurrences.len(), 1);
    let default: pages::Manifest =
        serde_json::from_str(r#"{"version":1,"pages":[]}"#).map_err(err)?;
    assert_eq!(default.math_renderer, Renderer::KatexPreferred);
    assert!(
        serde_json::from_str::<pages::Manifest>(
            r#"{"version":1,"pages":[],"math_renderer":"unknown"}"#
        )
        .is_err()
    );
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn math_pages_cli_writes_complete_receipt_and_rejects_invalid_later_page() -> Result<(), String> {
    use std::{fs, process::Command};
    let root = std::env::temp_dir().join(format!("nepl3-pages-math-{}", std::process::id()));
    fs::create_dir(&root).map_err(err)?;
    fs::write(
        root.join("a.nepld"),
        r#"article en "A" body cons display Math frac 1 0 nil"#,
    )
    .map_err(err)?;
    fs::write(
        root.join("bad.nepld"),
        r#"article en "Bad" body cons image asset "missing" none "Alt" none nil"#,
    )
    .map_err(err)?;
    for (index, policy, bad) in [
        (0, None, false),
        (1, Some("mathml-only"), false),
        (2, Some("unknown"), false),
        (3, None, true),
    ] {
        let mut manifest = serde_json::json!({"version":1,"pages":[{"id":"a","source":"a.nepld","route":"a.html"}]});
        if let Some(policy) = policy {
            manifest["math_renderer"] = policy.into();
        }
        if bad {
            manifest["pages"]
                .as_array_mut()
                .ok_or("pages")?
                .push(serde_json::json!({"id":"bad","source":"bad.nepld","route":"bad.html"}));
        }
        let path = root.join(format!("input-{index}.json"));
        fs::write(&path, serde_json::to_vec(&manifest).map_err(err)?).map_err(err)?;
        let output = root.join(format!("out-{index}"));
        let run = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
            .args(["doc-html", "pages"])
            .arg(&path)
            .arg(&output)
            .output()
            .map_err(err)?;
        if index >= 2 {
            assert!(!run.status.success());
            assert!(!output.exists());
        } else {
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            assert!(output.join("manifest.json").exists());
            assert!(
                fs::read_to_string(output.join("a.html"))
                    .map_err(err)?
                    .contains("<mfrac>")
            );
            assert_eq!(
                String::from_utf8_lossy(&run.stderr).contains("MathRendererUnavailable"),
                index == 0
            );
        }
    }
    fs::remove_dir_all(&root).map_err(err)?;
    Ok(())
}
