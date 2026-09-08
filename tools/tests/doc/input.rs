use nepl3_tools::doc::{
    export::pages::{self, Entry},
    source::compiled,
};

fn inputs() -> Vec<(Entry, String)> {
    vec![
        (Entry { id:"intro".into(),source:"docs/intro.md".into(),route:"docs/intro/index.html".into(),input:Some("drafts/a.nepld".into()) },
            "article en \"Intro\" body cons paragraph cons sentence cons link relative \"guide.md\" none text \"Guide\" nil nil nil".into()),
        (Entry { id:"guide".into(),source:"docs/guide.md".into(),route:"docs/guide/index.html".into(),input:Some("drafts/b.nepld".into()) },
            "article en \"Guide\" body nil".into()),
    ]
}

#[test]
fn physical_file_names_do_not_rebase_logical_doc_links() -> Result<(), String> {
    let compiled = compiled()?;
    let mut inputs = inputs();
    let first = pages::generate(&compiled, &inputs)?;
    assert!(
        std::str::from_utf8(&first.files["docs/intro/index.html"])
            .map_err(super::err)?
            .contains("href=\"../guide/index.html\"")
    );
    let manifest: serde_json::Value = serde_json::from_str(&first.manifest).map_err(super::err)?;
    assert_eq!(manifest["pages"][0]["source"], "docs/intro.md");
    assert_eq!(manifest["pages"][0]["input"], "drafts/a.nepld");
    for (entry, _) in &mut inputs {
        entry.input = None;
    }
    let legacy = pages::generate(&compiled, &inputs)?;
    let old: serde_json::Value = serde_json::from_str(&legacy.manifest).map_err(super::err)?;
    assert_eq!(legacy.files, first.files);
    assert_eq!(old["identity"], manifest["identity"]);
    assert_eq!(old["execution_identity"], manifest["execution_identity"]);
    assert_eq!(old["pages"][0]["input"], "docs/intro.md");
    for bad in [
        "",
        "/absolute.nepld",
        "../outside.nepld",
        "a/../b",
        "a/./b",
        "C:/outside",
        "a\\b",
        "a?b",
        "a#b",
        "a%b",
        "a\nb",
    ] {
        inputs[0].0.input = Some(bad.into());
        assert!(
            pages::generate(&compiled, &inputs).is_err_and(|e| e == "InvalidInputPath"),
            "{bad:?}"
        );
    }
    inputs[0].0.input = Some("a".repeat(4097));
    assert!(pages::generate(&compiled, &inputs).is_err_and(|e| e == "InvalidInputPath"));
    Ok(())
}

#[cfg(not(target_os = "wasi"))]
#[test]
fn file_export_reads_explicit_inputs_and_preserves_logical_namespace()
-> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    let parent = std::env::temp_dir().join(format!("nepl3-doc-input-paths-{}", std::process::id()));
    // Exclusive creation establishes ownership. Never delete a pre-existing path.
    fs::create_dir(&parent)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let root = parent.join("inputs");
        fs::create_dir(&root)?;
        fs::create_dir(root.join("drafts"))?;
        let inputs = inputs();
        for ((_, source), file) in inputs.iter().zip(["a.nepld", "b.nepld"]) {
            fs::write(root.join("drafts").join(file), source)?;
        }
        // Logical .md files intentionally do not exist and must not be opened.
        let mut manifest = serde_json::json!({"version":1,"pages":[
            {"id":"intro","source":"docs/intro.md","route":"docs/intro/index.html","input":"drafts/a.nepld"},
            {"id":"guide","source":"docs/guide.md","route":"docs/guide/index.html","input":"drafts/b.nepld"}
        ]});
        let input = root.join("manifest.json");
        fs::write(&input, serde_json::to_vec(&manifest)?)?;
        let output = parent.join("output");
        pages::write(&input, &output)?;
        let html = fs::read_to_string(output.join("docs/intro/index.html"))?;
        assert!(html.contains("href=\"../guide/index.html\""));
        assert!(output.join("manifest.json").is_file());
        for bad in ["../outside.nepld", "missing.nepld"] {
            manifest["pages"][0]["input"] = serde_json::json!(bad);
            fs::write(&input, serde_json::to_vec(&manifest)?)?;
            let failed = parent.join("failed");
            assert!(pages::write(&input, &failed).is_err());
            assert!(!failed.exists());
        }
        #[cfg(unix)]
        {
            fs::write(
                parent.join("outside.nepld"),
                "article en \"Outside\" body nil",
            )?;
            std::os::unix::fs::symlink(parent.join("outside.nepld"), root.join("escape.nepld"))?;
            manifest["pages"][0]["input"] = serde_json::json!("escape.nepld");
            fs::write(&input, serde_json::to_vec(&manifest)?)?;
            let failed = parent.join("failed");
            assert!(pages::write(&input, &failed).is_err());
            assert!(!failed.exists());
        }
        Ok(())
    })();
    fs::remove_dir_all(&parent)?;
    result
}
