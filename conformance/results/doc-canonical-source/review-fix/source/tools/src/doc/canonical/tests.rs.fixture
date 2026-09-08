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
