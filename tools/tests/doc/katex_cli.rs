//! Real pinned native host acceptance. CI installs the fixed npm closure first.
use std::{fs, process::Command};

#[test]
#[ignore = "requires Node >=24 and locked tools/audit/math npm packages"]
fn pinned_native_katex_doc_and_pages() -> Result<(), Box<dyn std::error::Error>> {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repo")?;
    let packages = repo.join("tools/audit/math/node_modules").canonicalize()?;
    let root = std::env::temp_dir().join(format!("nepl3-native-katex-{}", std::process::id()));
    fs::create_dir(&root)?;
    let source = root.join("input.nepld");
    fs::write(
        &source,
        "article en \"Math\" body cons paragraph cons sentence cons math Math frac 1 2 nil nil cons display Math add x y nil",
    )?;
    let binary = env!("CARGO_BIN_EXE_nepl3-tools");
    for mode in ["inline", "external"] {
        let out = root.join(mode);
        let result = Command::new(binary)
            .env("NEPL3_KATEX_NODE_MODULES", &packages)
            .args(["doc-html", "export", "--css", mode])
            .arg(&source)
            .arg(&out)
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let html = fs::read_to_string(out.join("document.html"))?;
        assert!(html.contains("katex"));
        assert!(html.contains("<math"));
        assert!(html.contains("aria-hidden=\"true\""));
        assert!(!html.contains("<script"));
        assert!(html.contains("style-src-attr 'none'"));
        let css = if mode == "inline" {
            html
        } else {
            fs::read_to_string(out.join("assets/doc.css"))?
        };
        assert!(css.contains("data:font/woff2;base64,"));
        assert!(css.contains("Permission is hereby granted"));
        assert!(!css.contains("url(fonts/"));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        assert_eq!(
            manifest["katex"]["visuals"]
                .as_array()
                .ok_or("visuals")?
                .len(),
            2
        );
        assert_eq!(
            manifest["katex"]["identity"]["execution_sha256"]
                .as_str()
                .ok_or("identity")?
                .len(),
            64
        );
    }
    // Three distinct occurrences/pages share admitted resources, not budgets.
    for i in 0..3 {
        fs::write(
            root.join(format!("p{i}.nepld")),
            format!("article en \"Page {i}\" body cons display Math frac 1 2 nil"),
        )?;
    }
    let pages = root.join("pages.json");
    fs::write(
        &pages,
        serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":(0..3).map(|i|serde_json::json!({"id":format!("p{i}"),"source":format!("p{i}.nepld"),"route":format!("p{i}/index.html")})).collect::<Vec<_>>()}),
        )?,
    )?;
    let out = root.join("pages");
    let result = Command::new(binary)
        .env("NEPL3_KATEX_NODE_MODULES", &packages)
        .args(["doc-html", "pages"])
        .arg(&pages)
        .arg(&out)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
    let generated = manifest["katex"].as_array().ok_or("pages")?;
    assert_eq!(generated.len(), 3);
    let scopes: std::collections::BTreeSet<_> = generated
        .iter()
        .map(|v| v["visuals"][0]["scope"].as_str().ok_or("scope"))
        .collect::<Result<_, _>>()?;
    assert_eq!(scopes.len(), 3);
    for i in 0..3 {
        assert!(fs::read_to_string(out.join(format!("p{i}/index.html")))?.contains("<math"));
    }
    // Missing configured assets is explicit optional absence, never success.
    let result = Command::new(binary)
        .env("NEPL3_KATEX_NODE_MODULES", root.join("absent"))
        .args(["doc-html", "export"])
        .arg(&source)
        .arg(root.join("missing"))
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("missing/manifest.json"))?)?;
    assert!(manifest["katex"].is_null());
    assert!(manifest.to_string().contains("KaTeX"));
    // Leading parse fallback must not retransmit unused assets on every page.
    for i in 0..3 {
        fs::write(
            root.join(format!("p{i}.nepld")),
            format!("article en \"Fallback {i}\" body cons display Math text \"🙂\" nil"),
        )?;
    }
    for mode in ["all-fallback", "fallback-then-visual"] {
        if mode == "fallback-then-visual" {
            fs::write(
                root.join("p2.nepld"),
                "article en \"Visual\" body cons display Math frac 1 2 nil",
            )?;
        }
        let out = root.join(mode);
        let result = Command::new(binary)
            .env("NEPL3_KATEX_NODE_MODULES", &packages)
            .args(["doc-html", "pages"])
            .arg(&pages)
            .arg(&out)
            .output()?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(out.join("manifest.json"))?)?;
        assert!(manifest["katex"][0].is_null());
        assert!(manifest["katex"][1].is_null());
        assert_eq!(manifest["katex"][2].is_null(), mode == "all-fallback");
        assert!(manifest.to_string().contains("KaTeXRendererParseError"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mocks = root.join("mock-bin");
        fs::create_dir(&mocks)?;
        let node = mocks.join("node");
        let mut paths = vec![mocks];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").ok_or("PATH")?,
        ));
        let path = std::env::join_paths(paths)?;
        fs::write(
            &source,
            format!(
                "article en \"Large\" body cons display Math text \"{}\" nil",
                "a".repeat(20000)
            ),
        )?;
        for (name, script, success) in [
            ("old-node", "exit 78", true),
            ("bad-old-node", "printf x; exit 78", false),
            (
                "wrong-identity",
                "cat >/dev/null; printf '%s' '{\"identity\":\"wrong\",\"kind\":\"unavailable\"}'",
                false,
            ),
            ("bad-json", "cat >/dev/null; printf '{'", false),
            (
                "stdout-overflow",
                "cat >/dev/null; exec head -c 9000000 /dev/zero",
                false,
            ),
            (
                "stderr-overflow",
                "cat >/dev/null; exec head -c 9000 /dev/zero >&2",
                false,
            ),
            ("deadline", "exec sleep 120", false),
        ] {
            fs::write(&node, format!("#!/bin/sh\n{script}\n"))?;
            fs::set_permissions(&node, fs::Permissions::from_mode(0o700))?;
            let out = root.join(name);
            let result = Command::new(binary)
                .env("PATH", &path)
                .env("NEPL3_KATEX_NODE_MODULES", &packages)
                .args(["doc-html", "export"])
                .arg(&source)
                .arg(&out)
                .output()?;
            assert_eq!(
                result.status.success(),
                success,
                "{name}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(out.exists(), success, "partial publication: {name}");
        }
    }
    fs::remove_dir_all(root)?;
    Ok(())
}
