#![cfg(not(target_family = "wasm"))]
use nepl3_tools::doc::{
    export::body::{self, page},
    source::compiled,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let p = std::env::temp_dir().join(format!("nepl3-body-page-{name}-{}", std::process::id()));
        fs::create_dir(&p)?;
        Ok(Self(p))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(
    input: &Path,
    output: &Path,
    metadata: &Path,
    limits: Option<&Path>,
) -> std::io::Result<Output> {
    let mut c = Command::new(env!("CARGO_BIN_EXE_nepl3-tools"));
    c.args(["doc-html", "body-page", "--metadata"])
        .arg(metadata);
    if let Some(p) = limits {
        c.arg("--limits").arg(p);
    }
    c.arg(input).arg(output).output()
}
#[test]
fn explicit_page_metadata_preserves_body_without_network_or_article()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new("valid")?;
    let input = f.0.join("body.nepld");
    let output = f.0.join("page.html");
    let meta = f.0.join("meta.json");
    let source = r#"body cons paragraph cons parallel cons variant en "English" cons variant ja "日本語" nil nil cons section one "First" body nil cons section two "Second" body nil nil"#;
    let metadata = page::Metadata {
        title: "</title><script>alert(1)</script>&".into(),
        language: "en".into(),
    };
    fs::write(&input, source)?;
    fs::write(&meta, serde_json::to_vec(&metadata)?)?;
    let result = run(&input, &output, &meta, None)?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let actual = fs::read_to_string(&output)?;
    let c = compiled()?;
    assert_eq!(
        actual,
        page::from_source(&c, source, &metadata, Default::default())?.html
    );
    let raw = body::from_source(&c, source, Default::default())?;
    assert!(actual.contains(&format!("<body>{}</body>", raw.html)));
    assert!(
        actual.contains("<title>&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;&amp;</title>")
    );
    assert!(actual.contains("English") && actual.contains("日本語"));
    assert!(
        !actual.contains("<script") && !actual.contains("<article") && !actual.contains("<link")
    );
    assert!(actual.contains("nepl-parallel-rows"));
    assert_eq!(actual.matches("<h1").count(), 2);
    assert!(actual.contains("<style>"));
    assert!(!actual.contains("https://fonts"));
    let limits = f.0.join("limits.json");
    fs::write(&limits, b"{}")?;
    let explicit = f.0.join("explicit.html");
    assert!(
        run(&input, &explicit, &meta, Some(&limits))?
            .status
            .success()
    );
    assert_eq!(fs::read(&explicit)?, fs::read(&output)?);
    assert!(!run(&input, &output, &meta, None)?.status.success());
    assert_eq!(fs::read_to_string(&output)?, actual);
    Ok(())
}
#[test]
fn page_cli_refuses_bad_metadata_limits_and_sources_without_output()
-> Result<(), Box<dyn std::error::Error>> {
    let f = Fixture::new("invalid")?;
    let input = f.0.join("body.nepld");
    let output = f.0.join("page.html");
    let meta = f.0.join("meta.json");
    fs::write(&input, "body nil")?;
    for text in [
        r#"{}"#,
        r#"{"title":"T","language":"en","extra":1}"#,
        r#"{"title":1,"language":"en"}"#,
        r#"{"title":"T","tit\u006ce":"T","language":"en"}"#,
        r#"{"title":"T","language":"en_US"}"#,
        r#"{"title":" ","language":"en"}"#,
        r#"{"title":"\u0000","language":"en"}"#,
    ] {
        fs::write(&meta, text)?;
        let result = run(&input, &output, &meta, None)?;
        assert!(!result.status.success(), "{text}");
        assert!(!output.exists());
        if text.contains("tit\\u006ce") {
            assert!(String::from_utf8_lossy(&result.stderr).contains("duplicate object key"));
        }
    }
    fs::write(&meta, [0xff])?;
    assert!(!run(&input, &output, &meta, None)?.status.success());
    assert!(!output.exists());
    fs::write(&meta, vec![b' '; 65_537])?;
    let result = run(&input, &output, &meta, None)?;
    assert!(String::from_utf8_lossy(&result.stderr).contains("input exceeds byte limit"));
    assert!(!output.exists());
    let mut exact = r#"{"title":"T","language":"en"}"#.to_owned();
    exact.push_str(&" ".repeat(65_536 - exact.len()));
    fs::write(&meta, exact)?;
    assert!(run(&input, &output, &meta, None)?.status.success());
    fs::remove_file(&output)?;
    for source in [
        r#"article en "T" body nil"#,
        r#"body cons paragraph cons sentence cons link external "https://example.org" text "x" nil nil nil"#,
        r#"body cons paragraph cons "unfinished""#,
    ] {
        fs::write(&input, source)?;
        assert!(!run(&input, &output, &meta, None)?.status.success());
        assert!(!output.exists());
    }
    fs::write(&input, "body nil")?;
    let mut limits = nepl3_tools::doc::export::pages::resources::ExecutionLimits::default();
    limits.output_limits.work = 0;
    let config = f.0.join("limits.json");
    fs::write(&config, serde_json::to_vec(&limits)?)?;
    let result = run(&input, &output, &meta, Some(&config))?;
    assert!(String::from_utf8_lossy(&result.stderr).contains("WorkLimit"));
    assert!(!output.exists());
    for args in [
        vec!["doc-html", "body-page"],
        vec!["doc-html", "body-page", "--unknown"],
        vec!["doc-html", "body-page", "--metadata"],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_nepl3-tools"))
                .args(args)
                .output()?
                .status
                .success()
        );
    }
    Ok(())
}
