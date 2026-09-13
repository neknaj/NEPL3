use super::*;
use crate::testing::Fixture;
use serde_json::json;

#[test]
fn source_bytes_and_profile_are_explicit_without_executing_guest() -> Result<()> {
    let fixture = Fixture::new()?;
    crate::command(fixture.root(), "git", &["init", "-q"])?;
    let source = "not valid DSL <script>alert(1)</script>\r\n日本語";
    fixture.write("examples/sample.neplx", source)?;
    fixture.json(
        "profile.json",
        &json!({"id":"test/1","aliases":{"External":"External/Root"}}),
    )?;
    let catalog = json!({"version":1,"profile":"profile.json","examples":[{
        "id":"external.sample","source":"examples/sample.neplx","language":"External","category":"Root"}]});
    fixture.json("site/examples.json", &catalog)?;
    crate::command(fixture.root(), "git", &["add", "."])?;
    for base in ["/NEPL3/", "/acceptance/project/"] {
        let result = generate(fixture.root(), base, "fixture-commit")?;
        assert_eq!(
            result["examples/sources/external.sample.txt"],
            source.as_bytes()
        );
        let html = std::str::from_utf8(&result["examples/index.html"])?;
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains(&format!(
            "href=\"{base}examples/sources/external.sample.txt\""
        )));
        let manifest: serde_json::Value =
            serde_json::from_slice(&result["examples/manifest.json"])?;
        assert_eq!(manifest["examples"][0]["sha256"], hash(source.as_bytes()));
        assert_eq!(manifest["examples"][0]["revision"], "fixture-commit");
        assert_eq!(manifest["examples"][0]["execution_available"], false);
        assert_eq!(manifest["source_profile"]["resolved_runtime"], false);
    }
    for (field, value) in [
        ("source", "../outside"),
        ("source", "examples/../profile.json"),
        ("source", "examples/missing.neplx"),
        ("category", "Other"),
        ("language", "Unknown"),
        ("id", "bad/id"),
    ] {
        let mut invalid = catalog.clone();
        invalid["examples"][0][field] = json!(value);
        fixture.json("site/examples.json", &invalid)?;
        assert!(
            generate(fixture.root(), "/NEPL3/", "fixture-commit").is_err(),
            "{field}: {value}"
        );
    }
    let mut duplicate = catalog.clone();
    duplicate["examples"]
        .as_array_mut()
        .ok_or("examples")?
        .push(catalog["examples"][0].clone());
    fixture.json("site/examples.json", &duplicate)?;
    assert!(generate(fixture.root(), "/NEPL3/", "fixture-commit").is_err());
    fixture.json("site/examples.json", &catalog)?;
    fixture.write("examples/sample.neplx", [0xff])?;
    assert!(generate(fixture.root(), "/NEPL3/", "fixture-commit").is_err());
    fixture.write("examples/sample.neplx", vec![b'x'; 1024 * 1024 + 1])?;
    assert!(generate(fixture.root(), "/NEPL3/", "fixture-commit").is_err());
    let mut many = Vec::new();
    for i in 0..5 {
        let path = format!("examples/large{i}.neplx");
        fixture.write(&path, vec![b'x'; 1024 * 1024])?;
        many.push(
            json!({"id":format!("large{i}"),"source":path,"language":"External","category":"Root"}),
        );
    }
    let mut large = catalog.clone();
    large["examples"] = json!(many);
    fixture.json("site/examples.json", &large)?;
    assert!(
        generate(fixture.root(), "/NEPL3/", "fixture-commit")
            .err()
            .ok_or("unbounded examples")?
            .to_string()
            .contains("example source limit")
    );
    Ok(())
}
