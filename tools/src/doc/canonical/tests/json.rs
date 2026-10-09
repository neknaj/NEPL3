use super::*;

#[test]
fn passive_json_registration_has_narrow_source_and_matching_route_scope() -> Result<()> {
    for source in [
        "design/tasks.json",
        "design/nested/contracts.json",
        "implementation-status.json",
        "doc/history/web-tea-import.json",
    ] {
        let mut value = registry();
        value["files"] =
            json!([{"id":"data", "source":source, "route":"sources/nested/data.json"}]);
        parse_registry(&serde_json::to_vec(&value)?)?;
    }
    for (source, route) in [
        ("other.json", "sources/data.json"),
        ("doc/history/other.json", "sources/data.json"),
        ("doc/history/Web-tea-import.json", "sources/data.json"),
        ("doc/history/web-tea-import.json.bak", "sources/data.json"),
        (
            "doc/history/../history/web-tea-import.json",
            "sources/data.json",
        ),
        ("doc/history/web-tea-import.json", "sources/data.html"),
        ("doc/data.json", "sources/data.json"),
        ("designish/data.json", "sources/data.json"),
        ("design/../data.json", "sources/data.json"),
        ("design/data.JSON", "sources/data.json"),
        ("Implementation-status.json", "sources/data.json"),
        ("design/a%2fb.json", "sources/data.json"),
        ("design/data.json", "sources/data.md"),
        ("design/data.json", "sources/data.html"),
        ("design/data.json", "data/data.json"),
        ("doc/data.md", "sources/data.json"),
    ] {
        let mut value = registry();
        value["files"] = json!([{"id":"data", "source":source, "route":route}]);
        assert!(
            parse_registry(&serde_json::to_vec(&value)?).is_err(),
            "{source} -> {route}"
        );
    }
    let mut value = registry();
    value["pages"][0]["renderer"] = json!(projection::FOOTNOTES_RENDERER);
    value["pages"][0]["route"] = value["pages"][0]["projection"].clone();
    value["files"] =
        json!([{"id":"data", "source":"design/tasks.json", "route":"sources/tasks.json"}]);
    assert!(parse_registry(&serde_json::to_vec(&value)?).is_err());
    let mut value = registry();
    let mut files: Vec<_> = (0..128).map(|index| json!({"id":format!("f{index}"), "source":format!("design/{index}.json"), "route":format!("sources/{index}.json")})).collect();
    value["files"] = json!(files);
    parse_registry(&serde_json::to_vec(&value)?)?;
    files.push(json!({"id":"extra", "source":"design/extra.json", "route":"sources/extra.json"}));
    value["files"] = json!(files);
    assert!(parse_registry(&serde_json::to_vec(&value)?).is_err());
    for (source, route) in [
        ("design/A.json", "sources/second.json"),
        ("design/second.json", "sources/A.json"),
        ("design/a.json/child.json", "sources/child.json"),
    ] {
        value["files"] = json!([
            {"id":"first", "source":"design/a.json", "route":"sources/a.json"},
            {"id":"second", "source":source, "route":route}
        ]);
        assert!(
            parse_registry(&serde_json::to_vec(&value)?).is_err(),
            "{source} -> {route}"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn passive_json_rejects_source_and_ancestor_symlinks() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write("design/real/data.json", b"{}")?;
    std::os::unix::fs::symlink("real/data.json", fixture.root().join("design/link.json"))?;
    std::os::unix::fs::symlink("real", fixture.root().join("design/linked"))?;
    for source in ["design/link.json", "design/linked/data.json"] {
        let mut value = registry();
        value["files"] = json!([{"id":"data", "source":source, "route":"sources/data.json"}]);
        fixture.json("doc/canonical.json", &value)?;
        let output = fixture.root().join("output");
        let error = markdown(fixture.root(), "doc/canonical.json", &output)
            .err()
            .ok_or("symlink accepted")?;
        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn passive_json_cannot_be_replaced_by_host_html_projection() -> Result<()> {
    let mut inputs = vec![(
        super::super::super::export::pages::Entry {
            id: "data".into(),
            source: "design/tasks.json".into(),
            route: "sources/tasks.json".into(),
            input: None,
        },
        b"{}\n".to_vec(),
    )];
    let projected = references::Projection {
        source: "design/tasks.json".into(),
        source_sha256: references::hash(b"{}\n"),
        route: "docs/tasks.html".into(),
        renderer: "test/1".into(),
        context: "test".into(),
        bytes: b"<p>replacement</p>".to_vec(),
    };
    assert!(references::apply(&mut inputs, &[projected]).is_err());
    assert_eq!(inputs[0].0.route, "sources/tasks.json");
    assert_eq!(inputs[0].1, b"{}\n");
    let mut mixed = vec![
        (
            super::super::super::export::pages::Entry {
                id: "guide".into(),
                source: "doc/guide.md".into(),
                route: "sources/guide.md".into(),
                input: None,
            },
            b"Guide".to_vec(),
        ),
        inputs.remove(0),
    ];
    let projections = [
        references::Projection {
            source: "doc/guide.md".into(),
            source_sha256: references::hash(b"Guide"),
            route: "docs/guide.html".into(),
            renderer: "test/1".into(),
            context: "test".into(),
            bytes: b"<p>Guide</p>".to_vec(),
        },
        references::Projection {
            source: "design/tasks.json".into(),
            source_sha256: references::hash(b"{}\n"),
            route: "docs/tasks.html".into(),
            renderer: "test/1".into(),
            context: "test".into(),
            bytes: b"<p>replacement</p>".to_vec(),
        },
    ];
    assert!(references::apply(&mut mixed, &projections).is_err());
    assert_eq!(mixed[0].0.route, "sources/guide.md");
    assert_eq!(mixed[0].1, b"Guide");
    assert_eq!(mixed[1].0.route, "sources/tasks.json");
    assert_eq!(mixed[1].1, b"{}\n");
    Ok(())
}

#[test]
fn passive_json_uses_existing_shared_input_limits_without_parsing() -> Result<()> {
    let fixture = Fixture::new()?;
    let file = |id: &str, source: &str| ReferenceFile {
        id: id.into(),
        source: source.into(),
        route: format!(
            "sources/{id}.{}",
            if source.ends_with(".md") {
                "md"
            } else {
                "json"
            }
        ),
    };
    let raw = "opaque UTF-8, 日本語\r\n  <script>passive text</script>\r\n";
    fixture.write("design/tasks.json", raw)?;
    let loaded = reference_inputs(fixture.root(), vec![file("tasks", "design/tasks.json")])?;
    assert_eq!(loaded[0].1, raw.as_bytes());
    fixture.write("design/tasks.json", [255])?;
    assert!(reference_inputs(fixture.root(), vec![file("tasks", "design/tasks.json")]).is_err());
    fixture.write("design/tasks.json", vec![b' '; 262_144])?;
    reference_inputs(fixture.root(), vec![file("tasks", "design/tasks.json")])?;
    fixture.write("design/tasks.json", vec![b' '; 262_145])?;
    assert!(reference_inputs(fixture.root(), vec![file("tasks", "design/tasks.json")]).is_err());
    assert!(
        reference_inputs(fixture.root(), vec![file("missing", "design/missing.json")]).is_err()
    );
    fs::create_dir_all(fixture.root().join("design/directory.json"))?;
    assert!(reference_inputs(fixture.root(), vec![file("dir", "design/directory.json")]).is_err());
    let mut inputs = Vec::new();
    for index in 0..8 {
        let source = if index % 2 == 0 {
            format!("design/{index}.json")
        } else {
            format!("doc/{index}.md")
        };
        fixture.write(&source, vec![b' '; 262_144])?;
        inputs.push(file(&format!("f{index}"), &source));
    }
    reference_inputs(fixture.root(), inputs)?;
    let mut inputs = Vec::new();
    for index in 0..8 {
        let source = if index % 2 == 0 {
            format!("design/{index}.json")
        } else {
            format!("doc/{index}.md")
        };
        inputs.push(file(&format!("f{index}"), &source));
    }
    fixture.write("design/extra.json", b" ")?;
    inputs.push(file("extra", "design/extra.json"));
    assert!(reference_inputs(fixture.root(), inputs).is_err());
    Ok(())
}

#[test]
fn passive_json_preserves_bytes_links_and_dependency_identity() -> Result<()> {
    let fixture = Fixture::new()?;
    let mut value = registry();
    value["pages"][0]["renderer"] = json!(projection::RENDERER);
    value["files"] = json!([
        {"id":"tasks", "source":"design/tasks.json", "route":"sources/tasks.json"},
        {"id":"status", "source":"implementation-status.json", "route":"sources/status.json"}
    ]);
    fixture.json("doc/canonical.json", &value)?;
    fixture.write("doc/aliases.json", "[]")?;
    fixture.write("doc/sample.nepld", r#"article ja "Title" body cons paragraph cons sentence cons link relative "../design/tasks.json" none text "Tasks" cons link relative "../implementation-status.json" none text "Status" nil nil nil"#)?;
    let raw = b"{\r\n  \"text\": \"<script>not executed</script>\"\r\n}\r\n";
    fixture.write("design/tasks.json", raw)?;
    fixture.write("implementation-status.json", b"{\"complete\":false}\n")?;
    let first = projection::generate(
        fixture.root(),
        "doc/canonical.json",
        &mut super::super::super::source::budget(),
    )?;
    let links: Vec<_> = pulldown_cmark::Parser::new(&first.files[0].1)
        .filter_map(|event| match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) => {
                Some(dest_url.into_string())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        links,
        [
            "sample.nepld",
            "../design/tasks.json",
            "../implementation-status.json"
        ]
    );
    let generated = generate_html(fixture.root(), "doc/canonical.json")?;
    assert_eq!(generated.files["sources/tasks.json"], raw);
    assert_eq!(
        generated.files["sources/status.json"],
        b"{\"complete\":false}\n"
    );
    let page = std::str::from_utf8(&generated.files["docs/sample.html"])?;
    assert!(page.contains("../sources/tasks.json"));
    assert!(page.contains("../sources/status.json"));
    assert!(!page.contains("<script>"));
    let receipt: serde_json::Value = serde_json::from_str(&generated.manifest)?;
    let files = receipt["files"].as_array().ok_or("files receipt")?;
    let file = files
        .iter()
        .find(|entry| entry["path"] == "sources/tasks.json")
        .ok_or("tasks receipt")?;
    assert_eq!(file["mime"], "application/octet-stream");
    fixture.write("design/tasks.json", b"{\"changed\":true}\n")?;
    let second = projection::generate(
        fixture.root(),
        "doc/canonical.json",
        &mut super::super::super::source::budget(),
    )?;
    assert_eq!(first.files, second.files);
    let before: serde_json::Value = serde_json::from_str(&first.manifest)?;
    let after: serde_json::Value = serde_json::from_str(&second.manifest)?;
    assert_ne!(before["input_context"], after["input_context"]);
    assert_ne!(before["input_pageset"], after["input_pageset"]);
    let revised = generate_html(fixture.root(), "doc/canonical.json")?;
    let revised_receipt: serde_json::Value = serde_json::from_str(&revised.manifest)?;
    assert_ne!(receipt["identity"], revised_receipt["identity"]);
    assert_ne!(
        receipt["execution_identity"],
        revised_receipt["execution_identity"]
    );
    let stage = fixture.root().join("markdown");
    markdown(fixture.root(), "doc/canonical.json", &stage)?;
    assert!(stage.join("doc/sample.md").is_file());
    assert!(!stage.join("sources/tasks.json").exists());
    assert!(!stage.join("design/tasks.json").exists());
    fixture.write("doc/sample.nepld", r#"article ja "Title" body cons paragraph cons sentence cons link relative "../design/tasks.json" some "x" text "Tasks" nil nil nil"#)?;
    let output = fixture.root().join("bad-fragment");
    let error = html(fixture.root(), "doc/canonical.json", &output)
        .err()
        .ok_or("file fragment accepted")?;
    assert!(error.to_string().contains("FileFragment"), "{error}");
    assert!(!output.exists());
    fixture.write("doc/sample.nepld", r#"article ja "Title" body cons paragraph cons sentence cons link relative "../design/unregistered.json" none text "Missing" nil nil nil"#)?;
    fixture.write("design/unregistered.json", b"{\"ambient\":true}\n")?;
    let error = html(fixture.root(), "doc/canonical.json", &output)
        .err()
        .ok_or("ambient file accepted")?;
    assert!(error.to_string().contains("MissingPage"), "{error}");
    assert!(!output.exists());
    fixture.write("doc/sample.nepld", r#"article ja "Title" body cons paragraph cons sentence cons link relative "../design/tasks.json" none text "Tasks" nil nil nil"#)?;
    value["html_output_limits"] = serde_json::to_value(
        super::super::super::export::pages::resources::OutputLimits::default(),
    )?;
    value["html_output_limits"]["output_bytes"] = json!(0);
    fixture.json("doc/canonical.json", &value)?;
    let error = html(fixture.root(), "doc/canonical.json", &output)
        .err()
        .ok_or("output limit ignored")?;
    assert!(error.to_string().contains("OutputLimit"), "{error}");
    assert!(!output.exists());
    Ok(())
}

#[test]
fn historical_receipt_requires_registration_and_keeps_opaque_bytes() -> Result<()> {
    let fixture = Fixture::new()?;
    let source = "doc/history/web-tea-import.json";
    let raw = b"opaque UTF-8\r\n<script>not executed</script>\r\n";
    fixture.write(source, raw)?;
    let mut value = registry();
    let unregistered = parse_registry(&serde_json::to_vec(&value)?)?;
    assert!(reference_inputs(fixture.root(), unregistered.files)?.is_empty());
    value["files"] =
        json!([{"id":"receipt", "source":source, "route":"sources/web-tea-import.json"}]);
    let registered = parse_registry(&serde_json::to_vec(&value)?)?;
    let inputs = reference_inputs(fixture.root(), registered.files)?;
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].0.source, source);
    assert_eq!(inputs[0].0.route, "sources/web-tea-import.json");
    assert_eq!(inputs[0].1, raw);
    Ok(())
}

#[test]
fn historical_receipt_links_do_not_load_embedded_source_paths() -> Result<()> {
    let fixture = Fixture::new()?;
    let mut value = registry();
    value["pages"][0]["renderer"] = json!(projection::RENDERER);
    value["files"] = json!([{"id":"receipt", "source":"doc/history/web-tea-import.json", "route":"sources/web-tea-import.json"}]);
    fixture.json("doc/canonical.json", &value)?;
    fixture.write("doc/aliases.json", "[]")?;
    fixture.write("doc/sample.nepld", r#"article ja "History" body cons paragraph cons sentence cons link relative "history/web-tea-import.json" none text "Receipt" nil nil nil"#)?;
    let raw = b"{\r\n  \"source\": \"private/unavailable.zip\", \"integration_decision\": \"doc/decisions/0003-web-tea-doc-migration.md\"\r\n}\r\n";
    fixture.write("doc/history/web-tea-import.json", raw)?;
    assert!(!fixture.root().join("private/unavailable.zip").exists());
    assert!(
        !fixture
            .root()
            .join("doc/decisions/0003-web-tea-doc-migration.md")
            .exists()
    );
    let markdown = projection::generate(
        fixture.root(),
        "doc/canonical.json",
        &mut super::super::super::source::budget(),
    )?;
    let links: Vec<_> = pulldown_cmark::Parser::new(&markdown.files[0].1)
        .filter_map(|event| match event {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) => {
                Some(dest_url.into_string())
            }
            _ => None,
        })
        .collect();
    assert!(
        links
            .iter()
            .any(|link| link == "history/web-tea-import.json")
    );
    let html = generate_html(fixture.root(), "doc/canonical.json")?;
    assert_eq!(html.files["sources/web-tea-import.json"], raw);
    assert!(
        std::str::from_utf8(&html.files["docs/sample.html"])?
            .contains("../sources/web-tea-import.json")
    );
    assert!(
        html.files
            .keys()
            .all(|path| !path.contains("unavailable.zip"))
    );
    let manifest: serde_json::Value = serde_json::from_str(&html.manifest)?;
    let files = manifest["files"].as_array().ok_or("files")?;
    let receipt = files
        .iter()
        .find(|entry| entry["path"] == "sources/web-tea-import.json")
        .ok_or("receipt")?;
    assert_eq!(receipt["mime"], "application/octet-stream");
    Ok(())
}
