use super::*;

#[test]
fn math_batch_exports_fragments_and_scopes_dependencies() -> Result<()> {
    let f = Fixture::new()?;
    f.write("main.nepld", r#"article en "Math" body cons paragraph cons sentence cons math Math frac 1 0 nil nil cons display Math frac 2 3 nil"#)?;
    f.write("other.nepld", r#"article en "Other" body nil"#)?;
    f.write("aliases.json", "[]")?;
    f.json("pages.json", &json!({"version":1,"files":[],"pages":[
        {"id":"main","source":"main.nepld","projection":"main.md","route":"main.md","aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"},
        {"id":"other","source":"other.nepld","projection":"other.md","route":"other.md","aliases":"aliases.json","renderer":"nepl3-tools.markdown-footnotes-pages/1"}
    ]}))?;
    let first = f.root().join("first");
    footnotes_manifest(&f.root().join("pages.json"), &first)?;
    let md = fs::read_to_string(first.join("main.md"))?;
    assert!(md.contains("$`\\frac{1}{0}`$"));
    assert!(md.contains("```math\n\\frac{2}{3}\n```"));
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(first.join("manifest.json"))?)?;
    assert_eq!(
        receipt["page_dependencies"][0]["math"]
            .as_array()
            .ok_or("math dependencies")?
            .len(),
        2
    );
    assert!(receipt["page_dependencies"][1].get("math").is_none());
    f.write(
        "main.nepld",
        r#"article en "Math" body cons display Math frac 4 5 nil"#,
    )?;
    let second = f.root().join("second");
    footnotes_manifest(&f.root().join("pages.json"), &second)?;
    assert_ne!(md, fs::read_to_string(second.join("main.md"))?);
    assert_eq!(
        fs::read(first.join("other.md"))?,
        fs::read(second.join("other.md"))?
    );
    // A later invalid page cannot publish the already-generated earlier page.
    f.write(
        "other.nepld",
        r#"article en "Bad" body cons display Math label x Sentence "[字/じ]" nil"#,
    )?;
    let rejected = f.root().join("rejected");
    assert!(footnotes_manifest(&f.root().join("pages.json"), &rejected).is_err());
    assert!(!rejected.exists());
    Ok(())
}

#[test]
fn checked_in_math_example_is_reproducible() -> Result<()> {
    let f = Fixture::new()?;
    f.write(
        "markdown.nepld",
        include_str!("../../../../../examples/document/math/markdown.nepld"),
    )?;
    f.write(
        "markdown.aliases.json",
        include_str!("../../../../../examples/document/math/markdown.aliases.json"),
    )?;
    f.write(
        "markdown.pages.json",
        include_str!("../../../../../examples/document/math/markdown.pages.json"),
    )?;
    let output = f.root().join("generated");
    footnotes_manifest(&f.root().join("markdown.pages.json"), &output)?;
    let md = fs::read_to_string(output.join("markdown.md"))?;
    assert_eq!(
        md,
        include_str!("../../../../../examples/document/math/markdown.md")
    );
    // Golden equality guards reproduction only. Structural payload preservation
    // and independent Markdown parsing are tested separately on actual inputs.
    Ok(())
}
