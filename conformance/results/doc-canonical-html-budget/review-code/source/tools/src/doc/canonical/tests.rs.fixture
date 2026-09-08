use super::*;
use crate::testing::Fixture;
use serde_json::json;

fn registry() -> serde_json::Value {
    json!({"version":1,"pages":[{"id":"sample","source":"doc/sample.nepld",
        "projection":"doc/sample.md","aliases":"doc/aliases.json",
        "route":"docs/sample.html","renderer":RENDERER}]})
}

#[test]
fn source_and_projection_changes_are_rejected_without_writing() -> Result<()> {
    let fixture = Fixture::new()?;
    let source = r#"article ja "[文/ぶん]" body cons paragraph cons "[本文/ほんぶん]。" nil nil"#;
    fixture.json("doc/canonical.json", &registry())?;
    fixture.write("doc/sample.nepld", source)?;
    fixture.write("doc/aliases.json", b"[]")?;
    let compiled = super::super::source::compiled()?;
    let view = host::generate(&compiled, "doc/sample.nepld", source, b"[]")?;
    let visible: String = pulldown_cmark::Parser::new(&view)
        .filter_map(|event| match event {
            pulldown_cmark::Event::Text(text) => Some(text.into_string()),
            _ => None,
        })
        .collect();
    assert_eq!(visible, "文[ぶん]本文[ほんぶん]。");
    assert!(view.starts_with("<!-- Generated from doc/sample.nepld;"));
    fixture.write("doc/sample.md", &view)?;
    check(fixture.root(), "doc/canonical.json")?;

    let edited = view.clone() + "hand edit\n";
    fixture.write("doc/sample.md", &edited)?;
    assert!(check(fixture.root(), "doc/canonical.json").is_err());
    assert_eq!(
        fs::read_to_string(fixture.root().join("doc/sample.md"))?,
        edited
    );
    fixture.write("doc/sample.md", &view)?;
    fixture.write("doc/sample.nepld", source.replace("本文", "変更"))?;
    assert!(check(fixture.root(), "doc/canonical.json").is_err());
    assert_eq!(
        fs::read_to_string(fixture.root().join("doc/sample.md"))?,
        view
    );
    fixture.write("doc/sample.nepld", source)?;
    fixture.write("doc/aliases.json", b"[ ]")?;
    // Alias bytes, not merely their interpreted values, are explicitly bound.
    assert!(check(fixture.root(), "doc/canonical.json").is_err());
    fixture.write("doc/aliases.json", b"[]")?;
    fixture.write("doc/sample.md", view.replace('\n', "\r\n"))?;
    assert!(check(fixture.root(), "doc/canonical.json").is_err());
    Ok(())
}

#[test]
fn registry_rejects_ambiguous_paths_versions_and_identities() -> Result<()> {
    let fixture = Fixture::new()?;
    let original = registry();
    for (field, value) in [
        ("source", "../sample.nepld"),
        ("source", "doc/../sample.nepld"),
        ("source", "doc/sample.md"),
        ("source", "doc/stream:name.nepld"),
        ("source", "doc/real./input.nepld"),
        ("source", "doc/real /input.nepld"),
        ("source", "doc/hash#name.nepld"),
        ("source", "doc/percent%20.nepld"),
        ("source", "doc/CON.nepld"),
        ("source", "doc/Lpt1/input.nepld"),
        ("source", "doc/文.nepld"),
        ("projection", "doc/sample\n.md"),
        ("aliases", "doc//aliases.json"),
        ("route", "/sample.html"),
        ("route", "../sample.html"),
        ("route", "https://example.com/sample.html"),
        ("route", "docs/sample.html?x"),
        ("route", "docs/real./sample.html"),
        ("route", "docs/aux.html"),
        ("renderer", "new-renderer"),
        ("id", "Sample"),
    ] {
        let mut value_json = original.clone();
        value_json["pages"][0][field] = json!(value);
        fixture.json("doc/canonical.json", &value_json)?;
        assert!(
            load(fixture.root(), "doc/canonical.json").is_err(),
            "{field}: {value}"
        );
    }
    for extra in [
        json!({"version":2,"pages":original["pages"]}),
        json!({"version":1,"pages":[]}),
        json!({"version":1,"pages":original["pages"],"unknown":true}),
    ] {
        fixture.json("doc/canonical.json", &extra)?;
        assert!(load(fixture.root(), "doc/canonical.json").is_err());
    }
    for field in ["id", "source", "projection", "aliases", "route"] {
        let mut value = original.clone();
        let mut second = json!({"id":"second","source":"doc/second.nepld","projection":"doc/second.md",
            "aliases":"doc/second.json","route":"docs/second.html","renderer":RENDERER});
        second[field] = value["pages"][0][field].clone();
        value["pages"]
            .as_array_mut()
            .ok_or("pages missing")?
            .push(second);
        fixture.json("doc/canonical.json", &value)?;
        assert!(
            load(fixture.root(), "doc/canonical.json").is_err(),
            "duplicate {field}"
        );
    }
    fixture.write(
        "doc/canonical.json",
        br#"{"version":1,"version":1,"pages":[]}"#,
    )?;
    assert!(load(fixture.root(), "doc/canonical.json").is_err());
    Ok(())
}

#[test]
fn canonical_file_access_is_bounded_and_regular() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write("doc/value", b"abcd")?;
    assert_eq!(bounded(fixture.root(), "doc/value", 4)?, b"abcd");
    assert!(bounded(fixture.root(), "doc/value", 3).is_err());
    assert!(bounded(fixture.root(), "doc", 4).is_err());
    assert!(bounded(fixture.root(), "doc/missing", 4).is_err());
    assert!(bounded(fixture.root(), "../outside", 4).is_err());
    Ok(())
}

#[test]
fn registered_file_parents_are_rejected_before_staging_any_output() -> Result<()> {
    let f = Fixture::new()?;
    for (field, path) in [
        ("projection", "doc/sample.md/child.md"),
        ("projection", "doc/SAMPLE.md/child.md"),
        ("source", "doc/sample.nepld/child.nepld"),
        ("aliases", "doc/aliases.json/child.json"),
    ] {
        let mut value = registry();
        let mut second = json!({"id":"second","source":"doc/second.nepld",
            "projection":"doc/second.md","aliases":"doc/second.json",
            "route":"docs/second.html","renderer":RENDERER});
        second[field] = json!(path);
        value["pages"].as_array_mut().ok_or("pages")?.push(second);
        // This sorts between the parent projection and its child; checking
        // only adjacent sorted names would miss the actual conflict.
        value["pages"].as_array_mut().ok_or("pages")?.push(json!({
            "id":"third","source":"doc/third.nepld","aliases":"doc/third.json",
            "projection":"doc/sample.md.other.md","route":"docs/third.html","renderer":RENDERER}));
        f.json("doc/canonical.json", &value)?;
        let output = f.root().join("stage");
        let error = markdown(f.root(), "doc/canonical.json", &output)
            .err()
            .ok_or("expected failure")?;
        assert!(
            error
                .to_string()
                .starts_with("canonical file path used as directory:"),
            "{field}: {error}"
        );
        assert!(!output.exists());
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn canonical_file_access_rejects_even_repository_internal_symlinks() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.write("doc/value", b"abcd")?;
    std::os::unix::fs::symlink("value", fixture.root().join("doc/link"))?;
    assert!(bounded(fixture.root(), "doc/link", 4).is_err());
    Ok(())
}

#[test]
fn html_uses_canonical_doc_and_never_overwrites_existing_output() -> Result<()> {
    let fixture = Fixture::new()?;
    fixture.json("doc/canonical.json", &registry())?;
    fixture.write(
        "doc/sample.nepld",
        r#"article ja "[文/ぶん]" body cons paragraph cons "本文。" nil nil"#,
    )?;
    let output = fixture.root().join("site");
    html(fixture.root(), "doc/canonical.json", &output)?;
    let page = fs::read_to_string(output.join("docs/sample.html"))?;
    assert!(page.contains("nepl-ruby"));
    assert!(page.contains("本文。"));
    assert!(!page.contains("<script"));
    let manifest = fs::read(output.join("manifest.json"))?;
    // HTML comes from the canonical Doc, not a separately maintained MD file.
    assert!(!fixture.root().join("doc/sample.md").exists());
    assert!(html(fixture.root(), "doc/canonical.json", &output).is_err());
    assert_eq!(fs::read(output.join("manifest.json"))?, manifest);
    fixture.write("doc/sample.nepld", "not a document")?;
    let invalid = fixture.root().join("invalid-site");
    assert!(html(fixture.root(), "doc/canonical.json", &invalid).is_err());
    assert!(!invalid.exists());
    Ok(())
}

#[test]
fn html_output_allowance_is_explicit_independent_and_recorded() -> Result<()> {
    let f = Fixture::new()?;
    let original = registry();
    f.json("doc/canonical.json", &original)?;
    f.write(
        "doc/sample.nepld",
        r#"article ja "文" body cons paragraph cons "本文。" nil nil"#,
    )?;
    f.write("doc/aliases.json", "[]")?;
    let omitted = f.root().join("omitted");
    html(f.root(), "doc/canonical.json", &omitted)?;
    let defaults =
        serde_json::to_value(super::super::export::pages::resources::OutputLimits::default())?;
    let mut explicit = original.clone();
    explicit["html_output_limits"] = defaults.clone();
    // A stopped Markdown budget must not replace the independent HTML budget.
    explicit["output_limits"] = defaults.clone();
    explicit["output_limits"]["work"] = json!(0);
    f.json("doc/canonical.json", &explicit)?;
    let selected = f.root().join("selected");
    html(f.root(), "doc/canonical.json", &selected)?;
    for path in ["docs/sample.html", "docs/assets/doc.css", "manifest.json"] {
        assert_eq!(
            fs::read(omitted.join(path))?,
            fs::read(selected.join(path))?
        );
    }
    let md = f.root().join("stopped-markdown");
    assert!(markdown(f.root(), "doc/canonical.json", &md).is_err());
    assert!(!md.exists());

    explicit["html_output_limits"]["work"] = json!(200_000_000);
    f.json("doc/canonical.json", &explicit)?;
    let higher = f.root().join("higher");
    html(f.root(), "doc/canonical.json", &higher)?;
    let before: serde_json::Value =
        serde_json::from_slice(&fs::read(selected.join("manifest.json"))?)?;
    let after: serde_json::Value =
        serde_json::from_slice(&fs::read(higher.join("manifest.json"))?)?;
    assert_eq!(after["output_budget"]["limits"]["work"], 200_000_000);
    assert_eq!(after["output_budget"]["initial_usage"]["work"], 0);
    assert_ne!(before["execution_identity"], after["execution_identity"]);
    assert_eq!(before["identity"], after["identity"]);
    assert_eq!(
        fs::read(selected.join("docs/sample.html"))?,
        fs::read(higher.join("docs/sample.html"))?
    );

    for (field, reason) in [
        ("work", "WorkLimit"),
        ("nodes", "NodeLimit"),
        ("depth", "DepthLimit"),
        ("allocation_units", "AllocationLimit"),
        ("output_bytes", "OutputLimit"),
        ("source_bytes", "SourceLimit"),
    ] {
        let mut limited = original.clone();
        limited["html_output_limits"] = defaults.clone();
        limited["html_output_limits"][field] = json!(0);
        f.json("doc/canonical.json", &limited)?;
        let destination = f.root().join(format!("stopped-{field}"));
        let error = html(f.root(), "doc/canonical.json", &destination)
            .err()
            .ok_or("expected stop")?;
        assert!(error.to_string().contains(reason), "{field}: {error}");
        assert!(!destination.exists());
    }
    // HTML configuration cannot stop an independently selected Markdown run.
    let md = f.root().join("markdown-only");
    markdown(f.root(), "doc/canonical.json", &md)?;
    assert!(md.join("manifest.json").is_file());
    Ok(())
}

#[test]
fn malformed_html_allowances_fail_before_any_output() -> Result<()> {
    let f = Fixture::new()?;
    let defaults =
        serde_json::to_value(super::super::export::pages::resources::OutputLimits::default())?;
    let mut negative = defaults.clone();
    negative["work"] = json!(-1);
    let mut unknown = defaults.clone();
    unknown["unlimited"] = json!(true);
    for bad in [
        serde_json::Value::Null,
        json!({"work":1}),
        json!("unlimited"),
        negative,
        unknown,
    ] {
        let mut value = registry();
        value["html_output_limits"] = bad;
        f.json("doc/canonical.json", &value)?;
        let output = f.root().join("invalid");
        assert!(html(f.root(), "doc/canonical.json", &output).is_err());
        assert!(!output.exists());
    }
    Ok(())
}

fn mixed(fixture: &Fixture) -> Result<serde_json::Value> {
    let mut value = registry();
    value["pages"][0]["renderer"] = json!(projection::RENDERER);
    value["pages"].as_array_mut().ok_or("pages")?.push(json!({
        "id":"target","source":"doc/target.nepld","projection":"doc/sub/target.md",
        "aliases":"doc/target.json","route":"docs/nested/target.html","renderer":RENDERER
    }));
    fixture.json("doc/canonical.json", &value)?;
    fixture.write("doc/aliases.json", b"[]")?;
    fixture.write(
        "doc/target.json",
        br#"[{"section":"use","name":"old-use"}]"#,
    )?;
    fixture.write(
        "doc/sample.nepld",
        r#"article en "A" body cons paragraph cons sentence
        cons link page "target" some "use" text "target"
        cons text " / " cons link relative "sample.md" none text "self" nil nil nil"#,
    )?;
    fixture.write(
        "doc/target.nepld",
        r#"article en "B" body
        cons section use "Use" body cons paragraph cons "Target." nil nil nil"#,
    )?;
    Ok(value)
}

#[test]
fn mixed_context_preserves_legacy_bytes_and_real_markdown_and_html_links() -> Result<()> {
    let f = Fixture::new()?;
    mixed(&f)?;
    let generated = projection::generate(
        f.root(),
        "doc/canonical.json",
        &mut super::super::source::budget(),
    )?;
    let source = fs::read_to_string(f.root().join("doc/target.nepld"))?;
    let aliases = fs::read(f.root().join("doc/target.json"))?;
    assert_eq!(
        generated.files[1].1,
        host::generate(
            &super::super::source::compiled()?,
            "doc/target.nepld",
            &source,
            &aliases
        )?
    );
    let links: Vec<_> = pulldown_cmark::Parser::new(&generated.files[0].1)
        .filter_map(|event| {
            if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) = event
            {
                Some(dest_url.into_string())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(links, ["sub/target.md#n-757365", "sample.md"]);
    assert!(generated.files[1].1.contains("<a name=\"n-757365\"></a>"));
    assert!(generated.files[1].1.contains("<a name=\"old-use\"></a>"));
    for (path, text) in &generated.files {
        f.write(path, text)?;
    }
    check(f.root(), "doc/canonical.json")?;
    let output = f.root().join("staged");
    markdown(f.root(), "doc/canonical.json", &output)?;
    for (path, text) in &generated.files {
        assert_eq!(fs::read(output.join(path))?, text.as_bytes());
    }
    assert!(markdown(f.root(), "doc/canonical.json", &output).is_err());
    let html_output = f.root().join("html");
    html(f.root(), "doc/canonical.json", &html_output)?;
    let html = fs::read_to_string(html_output.join("docs/sample.html"))?;
    assert!(html.contains("nested/target.html#n-757365"));
    Ok(())
}

#[test]
fn context_binds_all_raw_inputs_paths_and_registration_order() -> Result<()> {
    let f = Fixture::new()?;
    let original = mixed(&f)?;
    let before = projection::generate(
        f.root(),
        "doc/canonical.json",
        &mut super::super::source::budget(),
    )?;
    for (path, text) in &before.files {
        f.write(path, text)?;
    }
    let aliases = fs::read(f.root().join("doc/target.json"))?;
    f.write("doc/target.json", [aliases.as_slice(), b" "].concat())?;
    assert!(check(f.root(), "doc/canonical.json").is_err());
    let after = projection::generate(
        f.root(),
        "doc/canonical.json",
        &mut super::super::source::budget(),
    )?;
    assert_ne!(before.files[0].1, after.files[0].1);
    // The visible body is stable; exact input bytes still invalidate metadata.
    assert_eq!(
        before.files[0].1.split_once("\n\n").map(|v| v.1),
        after.files[0].1.split_once("\n\n").map(|v| v.1)
    );
    f.write("doc/target.json", &aliases)?;
    for field in ["source", "aliases", "projection", "route", "renderer"] {
        let mut changed = original.clone();
        let replacement = match field {
            "source" => "doc/moved.nepld",
            "aliases" => "doc/moved.json",
            "projection" => "doc/moved.md",
            "route" => "docs/moved.html",
            _ => projection::RENDERER,
        };
        if matches!(field, "source" | "aliases") {
            let old = original["pages"][1][field].as_str().ok_or("path")?;
            f.write(replacement, fs::read(f.root().join(old))?)?;
        }
        changed["pages"][1][field] = json!(replacement);
        f.json("doc/canonical.json", &changed)?;
        let after = projection::generate(
            f.root(),
            "doc/canonical.json",
            &mut super::super::source::budget(),
        )?;
        assert_ne!(before.files[0].1, after.files[0].1, "{field}");
    }
    let mut changed = original.clone();
    changed["pages"].as_array_mut().ok_or("pages")?.reverse();
    f.json("doc/canonical.json", &changed)?;
    let after = projection::generate(
        f.root(),
        "doc/canonical.json",
        &mut super::super::source::budget(),
    )?;
    assert_ne!(before.files[0].1, after.files[1].1);
    assert_eq!(before.files[1].1, after.files[0].1);
    f.json("doc/canonical.json", &original)?;
    f.write(
        "doc/other-registry.json",
        fs::read(f.root().join("doc/canonical.json"))?,
    )?;
    let after = projection::generate(
        f.root(),
        "doc/other-registry.json",
        &mut super::super::source::budget(),
    )?;
    assert_ne!(before.files[0].1, after.files[0].1);
    Ok(())
}

#[test]
fn grouped_failures_and_sticky_limits_never_publish_partial_outputs() -> Result<()> {
    use nepl3_core::budget::Budget;
    let f = Fixture::new()?;
    let registry = mixed(&f)?;
    let mut complete = super::super::source::budget();
    projection::generate(f.root(), "doc/canonical.json", &mut complete)?;
    let used = complete.usage();
    for kind in ["work", "allocation", "output", "nodes", "cancel"] {
        let mut limits = complete.limits();
        match kind {
            // The receipt includes selected limit values. Changing their digit
            // counts changes serialization work/bytes; leave a gap larger than
            // that bounded metadata difference instead of copying an exact
            // counter as an invariant across different configurations.
            "work" => limits.work = used.work - 1024,
            "allocation" => limits.allocation_units = used.allocation_units - 1,
            "output" => limits.output_bytes = used.output_bytes - 1024,
            "nodes" => limits.nodes = used.nodes - 1,
            _ => (),
        }
        let mut b = Budget::new(limits);
        if kind == "cancel" {
            b.cancel();
        }
        assert!(
            projection::generate(f.root(), "doc/canonical.json", &mut b).is_err(),
            "{kind}"
        );
        let stopped = b.poll();
        assert!(stopped.is_err(), "{kind}");
        assert!(projection::generate(f.root(), "doc/canonical.json", &mut b).is_err());
        assert_eq!(b.poll(), stopped);
    }
    f.write(
        "doc/target.nepld",
        r#"article en "B" body cons paragraph cons parallel
        cons variant en "a" cons variant ja "b" nil nil nil"#,
    )?;
    let out = f.root().join("invalid");
    assert!(markdown(f.root(), "doc/canonical.json", &out).is_err());
    assert!(!out.exists());
    // An ambient Markdown file is not an implicitly registered Doc target.
    f.write("doc/sub/target.md", "# Use\n")?;
    let mut missing = registry;
    missing["pages"].as_array_mut().ok_or("pages")?.pop();
    f.json("doc/canonical.json", &missing)?;
    assert!(markdown(f.root(), "doc/canonical.json", &out).is_err());
    assert!(!out.exists());
    Ok(())
}

#[test]
fn real_architecture_draft_links_to_canonical_extensions_with_legacy_bytes_intact() -> Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository")?;
    let f = Fixture::new()?;
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(repository.join("doc/canonical.json"))?)?;
    let architecture_source = value["pages"]
        .as_array()
        .ok_or("pages")?
        .iter()
        .find(|p| p["id"] == "architecture")
        .and_then(|p| p["source"].as_str())
        .unwrap_or("doc/migration/authored/01-architecture.nepld")
        .to_owned();
    // Once the page is cut over, read its explicitly registered source rather
    // than retaining a second handwritten copy solely to satisfy this test.
    value["pages"]
        .as_array_mut()
        .ok_or("pages")?
        .retain(|p| p["id"] != "architecture");
    for page in value["pages"].as_array().ok_or("pages")? {
        for field in ["source", "aliases", "projection"] {
            let path = page[field].as_str().ok_or("path")?;
            f.write(path, fs::read(repository.join(path))?)?;
        }
    }
    value["pages"].as_array_mut().ok_or("pages")?.push(json!({
        "id":"architecture","source":"doc/spec/01-architecture.nepld",
        "projection":"doc/spec/01-architecture.md","aliases":"doc/architecture.json",
        "route":"docs/spec/01-architecture.html","renderer":projection::RENDERER
    }));
    f.json("doc/canonical.json", &value)?;
    f.write("doc/architecture.json", b"[]")?;
    f.write(
        "doc/spec/01-architecture.nepld",
        fs::read(repository.join(architecture_source))?,
    )?;
    // Four real pages, including legacy body revalidation, share one output
    // allowance chosen before execution. No retry on a stopped budget.
    let limits = super::super::export::pages::resources::OutputLimits {
        work: 300_000_000,
        allocation_units: 750_000_000,
        ..Default::default()
    };
    value["output_limits"] = serde_json::to_value(limits)?;
    f.json("doc/canonical.json", &value)?;
    let mut budget = limits.budget();
    let result = projection::generate(f.root(), "doc/canonical.json", &mut budget)?;
    eprintln!("real four-page output usage: {:?}", budget.usage());
    for (path, text) in &result.files[..result.files.len() - 1] {
        if value["pages"]
            .as_array()
            .ok_or("pages")?
            .iter()
            .any(|p| p["projection"] == *path && p["renderer"] == RENDERER)
        {
            assert_eq!(text.as_bytes(), fs::read(repository.join(path))?, "{path}");
        }
    }
    let architecture = &result.files.last().ok_or("architecture")?.1;
    let links: Vec<_> = pulldown_cmark::Parser::new(architecture)
        .filter_map(|event| {
            if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) = event
            {
                Some(dest_url.into_string())
            } else {
                None
            }
        })
        .collect();
    assert!(links.iter().any(|l| l == "22-external-extensions.md"));
    Ok(())
}

#[test]
fn staging_rejects_aggregate_alias_and_page_limits_before_creating_output() -> Result<()> {
    let f = Fixture::new()?;
    let value = mixed(&f)?;
    // Each file is individually within its limit; the combined input is not.
    let padded = " ".repeat(600_000) + "[]";
    f.write("doc/aliases.json", &padded)?;
    f.write("doc/target.json", &padded)?;
    let output = f.root().join("over-limit");
    assert!(markdown(f.root(), "doc/canonical.json", &output).is_err());
    assert!(!output.exists());
    let pages: Vec<_> = (0..129)
        .map(|i| {
            json!({
                "id":format!("p{i}"),"source":format!("doc/p{i}.nepld"),
                "projection":format!("doc/p{i}.md"),"aliases":format!("doc/p{i}.json"),
                "route":format!("docs/p{i}.html"),"renderer":projection::RENDERER
            })
        })
        .collect();
    f.json(
        "doc/canonical.json",
        &json!({"version":value["version"],"pages":pages}),
    )?;
    let error = markdown(f.root(), "doc/canonical.json", &output)
        .err()
        .ok_or("expected failure")?;
    assert_eq!(error.to_string(), "PageCountLimit");
    assert!(!output.exists());
    Ok(())
}

#[test]
fn batch_limits_are_selected_before_public_generation_and_recorded() -> Result<()> {
    let f = Fixture::new()?;
    let mut value = mixed(&f)?;
    let limits = super::super::export::pages::resources::OutputLimits {
        work: 0,
        ..Default::default()
    };
    value["output_limits"] = serde_json::to_value(limits)?;
    f.json("doc/canonical.json", &value)?;
    let failed = f.root().join("failed");
    assert!(markdown(f.root(), "doc/canonical.json", &failed).is_err());
    assert!(!failed.exists());
    let limits = super::super::export::pages::resources::OutputLimits {
        work: 200_000_000,
        ..Default::default()
    };
    value["output_limits"] = serde_json::to_value(limits)?;
    f.json("doc/canonical.json", &value)?;
    let output = f.root().join("complete");
    markdown(f.root(), "doc/canonical.json", &output)?;
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("manifest.json"))?)?;
    assert_eq!(
        receipt["output_budget"]["limits"],
        serde_json::to_value(limits)?
    );
    assert_eq!(receipt["output_budget"]["initial_usage"]["work"], 0);
    for page in value["pages"].as_array().ok_or("pages")? {
        let path = page["projection"].as_str().ok_or("projection")?;
        f.write(path, fs::read(output.join(path))?)?;
    }
    check(f.root(), "doc/canonical.json")?;
    Ok(())
}

#[test]
fn registration_copy_work_is_admitted_before_its_allocation() -> Result<()> {
    let f = Fixture::new()?;
    f.write("doc/sample.nepld", r#"article en "A" body nil"#)?;
    f.write("doc/aliases.json", b"[]")?;
    let mut observed = Vec::new();
    for width in [16, 2048] {
        let mut value = registry();
        value["pages"][0]["id"] = json!("a".repeat(width));
        value["pages"][0]["renderer"] = json!(projection::RENDERER);
        f.json("doc/canonical.json", &value)?;
        let limits = nepl3_core::budget::Limits {
            work: 1,
            ..super::super::source::budget().limits()
        };
        let mut budget = nepl3_core::budget::Budget::new(limits);
        assert!(projection::generate(f.root(), "doc/canonical.json", &mut budget).is_err());
        assert_eq!(
            budget.poll(),
            Err(nepl3_core::budget::StopReason::WorkLimit)
        );
        assert_eq!(budget.usage().work, 1);
        observed.push(budget.usage().allocation_units);
    }
    // Increasing a registration string cannot allocate its copy after Work
    // has run out. The original bug grew this counter by the ID length delta.
    assert_eq!(observed[0], observed[1]);
    Ok(())
}

#[test]
fn synchronized_context_spec_drafts_parse_lower_and_check_labels() -> Result<()> {
    use nepl3_core::source::{SourceAdmission, SourceStore};
    use nepl3_wire::foundation::FoundationCodec;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository")?;
    let compiled = super::super::source::compiled()?;
    // Whole authoring drafts are larger than the small canonical pages. These
    // finite per-phase allowances are selected before parsing, not on a retry.
    let phase_limits = nepl3_core::budget::Limits {
        work: 600_000_000,
        allocation_units: 1_500_000_000,
        ..super::super::source::budget().limits()
    };
    for name in ["16-doc-migration", "21-doc-pages"] {
        let text =
            fs::read_to_string(repository.join(format!("doc/migration/authored/{name}.nepld")))?;
        super::super::source::with_named_input_limits(
            true,
            &compiled,
            &text,
            name,
            "Article",
            phase_limits,
            |tree, profile, parse_budget, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(super::super::source::err)?;
                let mut lower_budget = nepl3_core::budget::Budget::new(phase_limits);
                let document = nepl3_doc_core::lower::document(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    nepl3_doc_core::check::Category::Article,
                    profile.registry(),
                    &mut lower_budget,
                    &mut codec,
                )
                .map_err(super::super::source::err)?;
                let mut check_budget = nepl3_core::budget::Budget::new(phase_limits);
                nepl3_doc_core::labels::check(
                    &document,
                    profile.registry(),
                    &mut check_budget,
                    &mut admission,
                )
                .map_err(super::super::source::err)?;
                eprintln!(
                    "{name}: parse={:?}; lower={:?}; labels={:?}",
                    parse_budget.usage(),
                    lower_budget.usage(),
                    check_budget.usage()
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}
