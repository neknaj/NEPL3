use nepl3_tools::doc::{
    export::pages::{self, Entry, resources::PhaseLimits},
    source,
};
fn entry(id: &str, source: &str, route: &str) -> Entry {
    Entry {
        id: id.into(),
        source: source.into(),
        route: route.into(),
        input: None,
    }
}
#[test]
fn fragment_only_doc_source_generates_a_checked_self_href() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = vec![(entry("guide", "doc/guide.md", "docs/guide/index.html"),
        "article en \"Guide\" body cons paragraph cons sentence cons link relative \"\" some \"local\" text \"Here\" cons anchor local text \"Target\" nil nil nil".into())];
    let output = pages::generate(&compiled, &inputs)?;
    let html = std::str::from_utf8(&output.files["docs/guide/index.html"]).map_err(super::err)?;
    assert!(html.contains("href=\"index.html#n-6c6f63616c\""), "{html}");
    assert!(html.contains("id=\"n-6c6f63616c\""), "{html}");
    Ok(())
}
#[test]
fn actual_doc_links_to_exact_registered_file_bytes() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = vec![(entry("guide", "doc/guide.md", "docs/guide/index.html"),
        "article en \"Guide\" body cons paragraph cons sentence cons link relative \"../design/data.json\" none text \"Contract\" nil nil nil".into())];
    let payload = b"{\"version\":1}\r\n".to_vec();
    let resources = vec![(
        entry("data", "design/data.json", "data/data.json"),
        payload.clone(),
    )];
    let output = pages::generate_with_resources(
        &compiled,
        &inputs,
        &resources,
        PhaseLimits::default(),
        &mut source::budget(),
    )?;
    assert_eq!(output.files["data/data.json"], payload);
    let html = std::str::from_utf8(&output.files["docs/guide/index.html"]).map_err(super::err)?;
    assert!(html.contains("href=\"../../data/data.json\""), "{html}");
    let manifest: serde_json::Value = serde_json::from_str(&output.manifest).map_err(super::err)?;
    assert_eq!(
        manifest["registered_files"][0]["source"],
        "design/data.json"
    );
    assert!(
        pages::generate_with_resources(
            &compiled,
            &inputs,
            &[],
            PhaseLimits::default(),
            &mut source::budget()
        )
        .is_err()
    );
    let mut changed = resources;
    changed[0].1.push(b' ');
    let new = pages::generate_with_resources(
        &compiled,
        &inputs,
        &changed,
        PhaseLimits::default(),
        &mut source::budget(),
    )?;
    let other: serde_json::Value = serde_json::from_str(&new.manifest).map_err(super::err)?;
    assert_ne!(manifest["identity"], other["identity"]);
    assert_ne!(manifest["execution_identity"], other["execution_identity"]);
    Ok(())
}
#[test]
fn registered_resources_obey_output_stop_and_reserved_routes() -> Result<(), String> {
    let compiled = source::compiled()?;
    let inputs = vec![(
        entry("guide", "doc/guide.md", "index.html"),
        "article en \"Guide\" body nil".into(),
    )];
    for route in ["index.html", "manifest.json", "assets/doc.css", "data/CON"] {
        let resources = vec![(entry("file", "data/item", route), b"x".to_vec())];
        assert!(
            pages::generate_with_resources(
                &compiled,
                &inputs,
                &resources,
                PhaseLimits::default(),
                &mut source::budget()
            )
            .is_err(),
            "{route}"
        );
    }
    let resources = vec![(entry("file", "data/item", "data/item"), vec![0, 255])];
    let mut limits = source::budget().limits();
    limits.output_bytes = 1;
    let mut budget = nepl3_core::budget::Budget::new(limits);
    let failure = pages::generate_with_resources(
        &compiled,
        &inputs,
        &resources,
        PhaseLimits::default(),
        &mut budget,
    )
    .err()
    .ok_or("unexpected success")?;
    assert!(failure.contains("OutputLimit"), "{failure}");
    assert!(
        pages::generate_with_resources(
            &compiled,
            &inputs,
            &resources,
            PhaseLimits::default(),
            &mut budget
        )
        .is_err()
    );
    Ok(())
}

#[cfg(not(target_os = "wasi"))]
#[test]
fn file_resource_export_reads_only_explicit_contained_inputs()
-> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    let root = std::env::temp_dir().join(format!("nepl3-resource-export-{}", std::process::id()));
    fs::create_dir(&root)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let input = root.join("inputs");
        fs::create_dir(&input)?;
        fs::write(
            input.join("guide.nepld"),
            "article en \"Files\" body cons paragraph cons sentence cons link relative \"../data.bin\" none text \"Download\" nil nil nil",
        )?;
        let bytes = [0, 255, 13, 10, 128];
        fs::write(input.join("payload.bin"), bytes)?;
        let manifest_path = input.join("manifest.json");
        let mut manifest = serde_json::json!({"version":1,"pages":[{"id":"guide","source":"doc/guide.md","input":"guide.nepld","route":"docs/index.html"}],"files":[{"id":"file","source":"data.bin","input":"payload.bin","route":"download/data.bin"}]});
        fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
        let output = root.join("output");
        pages::write(&manifest_path, &output)?;
        assert_eq!(fs::read(output.join("download/data.bin"))?, bytes);
        assert!(
            fs::read_to_string(output.join("docs/index.html"))?
                .contains("href=\"../download/data.bin\"")
        );
        for bad in [
            "missing.bin",
            "../payload.bin",
            "payload.bin/../payload.bin",
        ] {
            manifest["files"][0]["input"] = serde_json::json!(bad);
            fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
            let failed = root.join("failed");
            assert!(pages::write(&manifest_path, &failed).is_err());
            assert!(!failed.exists());
        }
        #[cfg(unix)]
        {
            fs::write(root.join("outside.bin"), bytes)?;
            std::os::unix::fs::symlink(root.join("outside.bin"), input.join("escape.bin"))?;
            manifest["files"][0]["input"] = serde_json::json!("escape.bin");
            fs::write(&manifest_path, serde_json::to_vec(&manifest)?)?;
            assert!(pages::write(&manifest_path, &root.join("escaped")).is_err());
            assert!(!root.join("escaped").exists());
        }
        Ok(())
    })();
    // Only the exclusively created directory belongs to this test.
    fs::remove_dir_all(&root)?;
    result
}
