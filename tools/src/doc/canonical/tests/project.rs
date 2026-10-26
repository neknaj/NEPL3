use super::*;

#[test]
fn development_entry_registration_requires_exact_root_pairs() -> Result<()> {
    for name in ["CONTRIBUTING", "CODEX"] {
        let source = format!("doc/migration/authored/project/{name}.nepld");
        let projection = format!("{name}.md");
        let mut value = registry();
        value["pages"][0]["source"] = json!(source);
        value["pages"][0]["projection"] = json!(projection);
        parse_registry(&serde_json::to_vec(&value)?)?;
        for (field, invalid) in [
            ("source", source.to_lowercase()),
            ("source", format!("{source}.bak")),
            (
                "source",
                format!("doc/migration/authored/project/../project/{name}.nepld"),
            ),
            ("source", format!("{name}.nepld")),
            ("source", "doc/other.nepld".into()),
            (
                "source",
                if name == "CODEX" {
                    "doc/migration/authored/project/CONTRIBUTING.nepld"
                } else {
                    "doc/migration/authored/project/CODEX.nepld"
                }
                .into(),
            ),
            ("projection", projection.to_lowercase()),
            ("projection", format!("{projection}.bak")),
            ("projection", "AGENTS.md".into()),
            ("projection", "README.md".into()),
            ("aliases", "aliases.json".into()),
        ] {
            let mut invalid_value = value.clone();
            invalid_value["pages"][0][field] = json!(invalid);
            assert!(
                parse_registry(&serde_json::to_vec(&invalid_value)?).is_err(),
                "{name} {field}: {invalid}"
            );
        }
        // Normal doc/ projections retain their existing general contract.
        let mut doc_projection = value.clone();
        doc_projection["pages"][0]["projection"] = json!(format!("doc/{name}.md"));
        parse_registry(&serde_json::to_vec(&doc_projection)?)?;
        let mut duplicate = value.clone();
        let mut other = value["pages"][0].clone();
        other["id"] = json!("other");
        other["route"] = json!("docs/other.html");
        duplicate["pages"]
            .as_array_mut()
            .ok_or("pages")?
            .push(other);
        assert!(parse_registry(&serde_json::to_vec(&duplicate)?).is_err());
    }
    Ok(())
}

#[test]
fn development_entry_passive_references_are_exact_and_inert() -> Result<()> {
    for (source, route) in [
        ("AGENTS.md", "sources/agents.md"),
        ("LICENSE", "sources/license.txt"),
    ] {
        let mut value = registry();
        value["files"] = json!([{"id":"reference","source":source,"route":route}]);
        parse_registry(&serde_json::to_vec(&value)?)?;
    }
    for (source, route) in [
        ("agents.md", "sources/agents.md"),
        ("AGENTS.md.bak", "sources/agents.md"),
        ("doc/../AGENTS.md", "sources/agents.md"),
        ("AGENTS.md", "sources/AGENTS.md"),
        ("AGENTS.md", "sources/other.md"),
        ("AGENTS.md", "sources/agents.html"),
        ("AGENTS.md", "agents.md"),
        ("README.md", "sources/agents.md"),
        ("CODEX.md", "sources/agents.md"),
        ("license", "sources/license.txt"),
        ("LICENSE.bak", "sources/license.txt"),
        ("doc/../LICENSE", "sources/license.txt"),
        ("LICENSE", "sources/LICENSE.txt"),
        ("LICENSE", "sources/other.txt"),
        ("LICENSE", "sources/license.html"),
        ("LICENSE", "sources/license.md"),
        (".env", "sources/license.txt"),
        ("Cargo.toml", "sources/license.txt"),
    ] {
        let mut value = registry();
        value["files"] = json!([{"id":"reference","source":source,"route":route}]);
        assert!(
            parse_registry(&serde_json::to_vec(&value)?).is_err(),
            "{source} -> {route}"
        );
    }
    // The separate footnotes/SVG-only profile must not acquire this permission.
    for (source, route) in [
        ("AGENTS.md", "sources/agents.md"),
        ("LICENSE", "sources/license.txt"),
    ] {
        let mut value = registry();
        value["pages"][0]["renderer"] = json!(projection::FOOTNOTES_RENDERER);
        value["pages"][0]["route"] = value["pages"][0]["projection"].clone();
        value["files"] = json!([{"id":"reference","source":source,"route":route}]);
        assert!(parse_registry(&serde_json::to_vec(&value)?).is_err());
    }
    Ok(())
}

#[test]
fn development_entry_drafts_export_root_links_and_preserve_passive_bytes() -> Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository")?;
    let f = Fixture::new()?;
    let mut pages = Vec::new();
    for (name, id, title) in [
        ("CONTRIBUTING", "contributing", "開発への参加"),
        ("CODEX", "codex", "実装作業の入口"),
    ] {
        let source = format!("doc/migration/authored/project/{name}.nepld");
        f.write(&source, fs::read(repository.join(&source))?)?;
        f.write(
            &format!("{name}.md"),
            fs::read(repository.join(format!("{name}.md")))?,
        )?;
        let aliases = format!("doc/{id}-aliases.json");
        f.json(&aliases, &json!([{"section":null,"name":title}]))?;
        pages.push(json!({"id":id,"source":source,"projection":format!("{name}.md"),"aliases":aliases,"route":format!("docs/project/{id}.html"),"renderer":projection::RENDERER}));
    }
    let files = json!([
        {"id":"agents","source":"AGENTS.md","route":"sources/agents.md"},
        {"id":"license","source":"LICENSE","route":"sources/license.txt"},
        {"id":"tasks","source":"design/tasks.json","route":"sources/tasks.json"},
        {"id":"status","source":"implementation-status.json","route":"sources/status.json"},
        {"id":"acceptance","source":"design/acceptance.json","route":"sources/acceptance.json"},
        {"id":"registry","source":"doc/canonical.json","route":"sources/canonical.json"},
        {"id":"index","source":"doc/README.md","route":"sources/doc-index.md"},
        {"id":"development","source":"doc/development.md","route":"sources/development.md"}
    ]);
    for path in [
        "AGENTS.md",
        "LICENSE",
        "design/tasks.json",
        "implementation-status.json",
        "design/acceptance.json",
        "doc/README.md",
        "doc/development.md",
    ] {
        f.write(path, fs::read(repository.join(path))?)?;
    }
    let complete = json!({"version":1,"pages":pages,"files":files});
    let mut missing = complete.clone();
    missing["files"]
        .as_array_mut()
        .ok_or("files")?
        .retain(|file| file["source"] != "doc/README.md");
    f.json("doc/canonical.json", &missing)?;
    let incomplete_output = f.root().join("missing-reference");
    let error = markdown(f.root(), "doc/canonical.json", &incomplete_output)
        .err()
        .ok_or("unregistered ambient reference accepted")?;
    assert!(error.to_string().contains("MissingPage"), "{error}");
    assert!(!incomplete_output.exists());
    f.json("doc/canonical.json", &complete)?;
    let first = f.root().join("first");
    let second = f.root().join("second");
    markdown(f.root(), "doc/canonical.json", &first)?;
    markdown(f.root(), "doc/canonical.json", &second)?;
    for (name, title) in [
        ("CONTRIBUTING", "開発への参加"),
        ("CODEX", "実装作業の入口"),
    ] {
        let path = format!("{name}.md");
        let bytes = fs::read(first.join(&path))?;
        assert_eq!(bytes, fs::read(second.join(&path))?);
        assert_eq!(
            fs::read(f.root().join(&path))?,
            fs::read(repository.join(&path))?
        );
        let text = std::str::from_utf8(&bytes)?;
        assert!(text.contains(&format!("<a name=\"{title}\"></a>")));
        let links: Vec<_> = pulldown_cmark::Parser::new(text)
            .filter_map(|e| match e {
                pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) => {
                    Some(dest_url.to_string())
                }
                _ => None,
            })
            .collect();
        let expected = if name == "CONTRIBUTING" {
            vec![
                "doc/migration/authored/project/CONTRIBUTING.nepld",
                "doc/README.md",
                "AGENTS.md",
                "doc/development.md",
                "design/tasks.json",
                "implementation-status.json",
                "LICENSE",
            ]
        } else {
            vec![
                "doc/migration/authored/project/CODEX.nepld",
                "AGENTS.md",
                "doc/README.md",
                "doc/development.md",
                "implementation-status.json",
                "design/tasks.json",
                "doc/canonical.json",
                "design/acceptance.json",
            ]
        };
        assert_eq!(links, expected);
    }
    let html = generate_html(f.root(), "doc/canonical.json")?;
    for (id, title) in [
        ("contributing", "開発への参加"),
        ("codex", "実装作業の入口"),
    ] {
        let text = std::str::from_utf8(
            html.files
                .get(&format!("docs/project/{id}.html"))
                .ok_or("page")?,
        )?;
        assert!(text.contains(&format!("id=\"{title}\"")));
        for target in [
            "../../sources/doc-index.md",
            "../../sources/development.md",
            "../../sources/agents.md",
            "../../sources/tasks.json",
            "../../sources/status.json",
        ] {
            assert!(
                text.contains(&format!("href=\"{target}\"")),
                "{id}: {target}"
            );
        }
    }
    assert!(
        std::str::from_utf8(
            html.files
                .get("docs/project/contributing.html")
                .ok_or("contributing")?
        )?
        .contains("href=\"../../sources/license.txt\"")
    );
    for (source, route) in [
        ("AGENTS.md", "sources/agents.md"),
        ("LICENSE", "sources/license.txt"),
        ("doc/README.md", "sources/doc-index.md"),
        ("doc/development.md", "sources/development.md"),
    ] {
        assert_eq!(
            html.files.get(route).ok_or("passive")?,
            &fs::read(repository.join(source))?
        );
    }
    assert!(markdown(f.root(), "doc/canonical.json", &first).is_err());
    Ok(())
}

#[cfg(unix)]
#[test]
fn development_entry_passive_reference_symlinks_cannot_publish() -> Result<()> {
    for (source, route) in [
        ("AGENTS.md", "sources/agents.md"),
        ("LICENSE", "sources/license.txt"),
    ] {
        let f = Fixture::new()?;
        f.write("original.txt", "unchanged reference")?;
        std::os::unix::fs::symlink("original.txt", f.root().join(source))?;
        let mut value = registry();
        value["files"] = json!([{"id":"reference","source":source,"route":route}]);
        f.json("doc/canonical.json", &value)?;
        let out = f.root().join("output");
        let error = markdown(f.root(), "doc/canonical.json", &out)
            .err()
            .ok_or("symlink accepted")?;
        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(!out.exists());
    }
    Ok(())
}

#[test]
fn development_entry_passive_references_refuse_host_html_substitution() -> Result<()> {
    for (source, route) in [
        ("AGENTS.md", "sources/agents.md"),
        ("LICENSE", "sources/license.txt"),
    ] {
        let bytes = b"original reference bytes\n".to_vec();
        let mut inputs = vec![(
            super::super::super::export::pages::Entry {
                id: "reference".into(),
                source: source.into(),
                route: route.into(),
                input: None,
            },
            bytes.clone(),
        )];
        let replacement = references::Projection {
            source: source.into(),
            source_sha256: references::hash(&bytes),
            route: "docs/replacement.html".into(),
            renderer: "test/1".into(),
            context: "test".into(),
            bytes: b"<p>replacement</p>".to_vec(),
        };
        assert!(references::apply(&mut inputs, &[replacement]).is_err());
        assert_eq!(inputs[0].0.source, source);
        assert_eq!(inputs[0].0.route, route);
        assert_eq!(inputs[0].1, bytes);
    }
    Ok(())
}
