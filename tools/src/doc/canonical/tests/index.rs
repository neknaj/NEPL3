use super::*;

// Independent enumeration of the twelve additional index exceptions. The
// existing design JSON and doc Markdown categories need no new permission.
const REFERENCES: &[(&str, &str)] = &[
    ("README.md", "sources/readme.md"),
    ("tasks/README.md", "sources/tasks/readme.md"),
    (
        "conformance/extensions/hello/README.md",
        "sources/conformance/hello/readme.md",
    ),
    ("conformance/cases.json", "sources/conformance/cases.json"),
    (
        "interfaces/contracts.json",
        "sources/interfaces/contracts.json",
    ),
    ("interfaces/doc.json", "sources/interfaces/doc.json"),
    (
        "interfaces/foundation.json",
        "sources/interfaces/foundation.json",
    ),
    ("interfaces/markup.json", "sources/interfaces/markup.json"),
    ("interfaces/math.json", "sources/interfaces/math.json"),
    ("interfaces/model.json", "sources/interfaces/model.json"),
    (
        "interfaces/sentence-reader.json",
        "sources/interfaces/sentence-reader.json",
    ),
    (
        "interfaces/sentence.json",
        "sources/interfaces/sentence.json",
    ),
];

#[test]
fn index_passive_registration_requires_exact_pairs_and_retains_profile_boundaries() -> Result<()> {
    for &(source, route) in REFERENCES {
        let mut value = registry();
        value["files"] = json!([{"id":"reference","source":source,"route":route}]);
        parse_registry(&serde_json::to_vec(&value)?)?;
        for (field, invalid) in [
            ("source", source.to_ascii_uppercase()),
            ("source", format!("{source}.bak")),
            ("source", format!("doc/../{source}")),
            ("route", format!("{route}.html")),
            ("route", format!("{route}/nested")),
            ("route", "sources/other.json".into()),
        ] {
            let mut bad = value.clone();
            bad["files"][0][field] = json!(invalid);
            assert!(
                parse_registry(&serde_json::to_vec(&bad)?).is_err(),
                "{source}: {field} {invalid}"
            );
        }
        value["pages"][0]["renderer"] = json!(projection::FOOTNOTES_RENDERER);
        value["pages"][0]["route"] = value["pages"][0]["projection"].clone();
        assert!(parse_registry(&serde_json::to_vec(&value)?).is_err());
    }
    for source in [
        "interfaces/reader.json",
        "interfaces/private.json",
        "conformance/other.json",
        "tasks/T01.md",
        "examples/",
        "tasks/",
        ".env",
        "Cargo.toml",
    ] {
        let mut value = registry();
        value["files"] = json!([{"id":"reference","source":source,"route":"sources/other.json"}]);
        assert!(
            parse_registry(&serde_json::to_vec(&value)?).is_err(),
            "{source}"
        );
    }
    Ok(())
}

fn fixture() -> Result<Fixture> {
    let f = Fixture::new()?;
    let mut value = registry();
    value["pages"][0]["renderer"] = json!(projection::RENDERER);
    value["files"] = json!(REFERENCES.iter().enumerate().map(|(i, (source, route))|
        json!({"id":format!("reference-{i}"),"source":source,"route":route})
    ).collect::<Vec<_>>());
    let mut input = String::from("article en \"Index\" body cons paragraph cons sentence ");
    for (i, &(source, _)) in REFERENCES.iter().enumerate() {
        input.push_str(&format!(
            "cons link relative \"../{source}\" none text \"Reference {i}\" "
        ));
        // Deliberately not valid JSON. Passive bytes are not schema inputs and
        // their apparent instructions/links must never be executed/followed.
        f.write(
            source,
            format!("inert reference {i}: {{ invalid JSON; ../unregistered\n"),
        )?;
    }
    for directory in ["examples", "tasks"] {
        input.push_str(&format!("cons link external \"https://github.com/neknaj/NEPL3/tree/main/{directory}\" text \"{directory}/\" "));
    }
    input.push_str("nil nil nil");
    f.write("doc/sample.nepld", input)?;
    f.write("doc/aliases.json", "[]")?;
    f.json("doc/canonical.json", &value)?;
    Ok(f)
}

#[test]
fn index_passive_exports_preserve_exact_bytes_and_do_not_infer_missing_files() -> Result<()> {
    let f = fixture()?;
    let markdown = projection::generate(
        f.root(),
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
    for &(source, _) in REFERENCES {
        assert!(links.contains(&format!("../{source}")));
    }
    let html = generate_html(f.root(), "doc/canonical.json")?;
    let page = std::str::from_utf8(html.files.get("docs/sample.html").ok_or("HTML")?)?;
    for &(source, route) in REFERENCES {
        assert_eq!(
            html.files.get(route).ok_or("passive payload")?,
            &fs::read(f.root().join(source))?
        );
        assert!(page.contains(&format!("href=\"../{route}\"")));
    }
    for directory in ["examples", "tasks"] {
        let url = format!("https://github.com/neknaj/NEPL3/tree/main/{directory}");
        assert!(links.contains(&url));
        assert!(page.contains(&format!("href=\"{url}\"")));
    }
    // Ambient presence is not registration. This removes the last declaration
    // without touching its actual input file and must still fail resolution.
    let mut raw: serde_json::Value =
        serde_json::from_slice(&fs::read(f.root().join("doc/canonical.json"))?)?;
    let _ = raw["files"].as_array_mut().ok_or("files")?.pop();
    f.json("doc/canonical.json", &raw)?;
    let out = f.root().join("incomplete");
    assert!(super::super::markdown(f.root(), "doc/canonical.json", &out).is_err());
    assert!(!out.exists());
    Ok(())
}

#[test]
fn index_passive_rejects_nonregular_oversized_non_utf8_and_html_substitution() -> Result<()> {
    for &(source, route) in REFERENCES {
        let bytes = b"exact passive bytes".to_vec();
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
        assert_eq!(inputs[0].0.route, route);
        assert_eq!(inputs[0].1, bytes);
    }
    for invalid in [vec![b'x'; 262_145], vec![0xff]] {
        let f = fixture()?;
        f.write("interfaces/doc.json", invalid)?;
        let out = f.root().join("invalid");
        assert!(super::super::markdown(f.root(), "doc/canonical.json", &out).is_err());
        assert!(!out.exists());
    }
    let f = fixture()?;
    fs::remove_file(f.root().join("interfaces/doc.json"))?;
    fs::create_dir(f.root().join("interfaces/doc.json"))?;
    let out = f.root().join("directory");
    assert!(super::super::markdown(f.root(), "doc/canonical.json", &out).is_err());
    assert!(!out.exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn index_passive_symlinks_cannot_publish() -> Result<()> {
    let f = fixture()?;
    fs::remove_file(f.root().join("interfaces/doc.json"))?;
    std::os::unix::fs::symlink("../README.md", f.root().join("interfaces/doc.json"))?;
    let out = f.root().join("linked");
    let error = super::super::markdown(f.root(), "doc/canonical.json", &out)
        .err()
        .ok_or("symlink accepted")?;
    assert!(error.to_string().contains("symlink"));
    assert!(!out.exists());
    Ok(())
}

#[test]
fn index_directory_links_do_not_gain_implicit_file_or_index_resolution() -> Result<()> {
    for directory in ["examples", "tasks"] {
        let f = fixture()?;
        // Even an ambient directory and README cannot justify an unregistered
        // implicit index. The author must select explicit external navigation.
        f.write(&format!("{directory}/README.md"), "ambient index")?;
        let source = f.root().join("doc/sample.nepld");
        let original = fs::read_to_string(&source)?;
        let changed = original.replace(
            &format!("link external \"https://github.com/neknaj/NEPL3/tree/main/{directory}\""),
            &format!("link relative \"../{directory}/\" none"),
        );
        assert_ne!(changed, original);
        fs::write(source, changed)?;
        let out = f.root().join("implicit-directory");
        assert!(super::super::markdown(f.root(), "doc/canonical.json", &out).is_err());
        assert!(!out.exists());
    }
    Ok(())
}
