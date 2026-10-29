use super::*;

#[test]
fn math_grouping_revision_changes_only_math_page_context() -> Result<()> {
    let input = Input {
        page: Page {
            id: "page".into(),
            source: "page.nepld".into(),
            projection: "page.md".into(),
            aliases: "aliases.json".into(),
            route: "page.md".into(),
            renderer: COMPACT_RENDERER.into(),
        },
        source: "source".into(),
        aliases: b"[]".to_vec(),
    };
    let dependency = annotated::pages::MathDependency {
        embed: 7,
        guest_digest: Digest::of(b"math"),
    };
    // Independent wire framing, not the production field helper or version constant.
    let expected = |version: Option<&[u8]>| {
        let mut bytes = Vec::new();
        let mut field = |value: &[u8]| {
            bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
            bytes.extend_from_slice(value);
        };
        field(b"nepl3.canonical-page-context/1\0");
        for value in [
            COMPACT_RENDERER,
            "page",
            "page.nepld",
            "page.md",
            "aliases.json",
        ] {
            field(value.as_bytes());
        }
        field(&Digest::of(b"source").0);
        field(&Digest::of(b"[]").0);
        field(&0_u64.to_be_bytes());
        if let Some(version) = version {
            field(version);
            field(&1_u64.to_be_bytes());
            field(&7_u64.to_be_bytes());
            field(&Digest::of(b"math").0);
        }
        Digest::of(&bytes)
    };
    let actual = page_context(
        &input,
        &[],
        &[],
        &[dependency],
        &mut crate::doc::source::budget(),
    )?;
    assert_eq!(actual, expected(Some(b"structural-math-tex/2\0")));
    assert_ne!(actual, expected(Some(b"structural-math-tex/1\0")));
    assert_eq!(
        page_context(&input, &[], &[], &[], &mut crate::doc::source::budget())?,
        expected(None)
    );
    Ok(())
}
