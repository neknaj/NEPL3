use super::*;

#[test]
fn final_receipt_is_inside_the_output_allowance() {
    assert!(check_size(MAX_SITE_BYTES - 1, 1).is_ok());
    assert!(check_size(MAX_SITE_BYTES, 1).is_err());
    assert!(check_size(usize::MAX, 1).is_err());
}

#[test]
fn running_renderer_has_an_executable_and_build_compiler_identity() -> Result<()> {
    let identity = renderer_identity()?;
    assert_eq!(identity.executable_sha256.len(), 64);
    assert!(
        identity
            .executable_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    );
    assert!(identity.build_rustc.starts_with("rustc "));
    assert_eq!(
        identity.executable_sha256,
        hash(&std::fs::read(std::env::current_exe()?)?)
    );
    Ok(())
}

#[test]
fn base_paths_are_project_paths_not_urls_or_traversals() {
    for base in ["/", "/NEPL3/", "/acceptance/project/"] {
        assert!(
            SiteConfig {
                version: 1,
                base_path: base.into()
            }
            .validate()
            .is_ok()
        );
    }
    for base in [
        "",
        "//",
        "NEPL3/",
        "/NEPL3",
        "/../",
        "/./",
        "/a//b/",
        "/%2e%2e/",
        "/a?b/",
        "/a#b/",
        "https://example.org/",
        "/a\\b/",
    ] {
        assert!(
            SiteConfig {
                version: 1,
                base_path: base.into()
            }
            .validate()
            .is_err(),
            "{base}"
        );
    }
}

#[test]
fn composition_preserves_doc_bytes_and_identifies_every_payload() -> Result<()> {
    let registry: canonical::Registry = serde_json::from_str(
        r#"{"version":1,"pages":[{"id":"intro","source":"intro.nepld","projection":"intro.md","aliases":"intro.json","route":"docs/intro.html","renderer":"test"}]}"#,
    )?;
    let input = || GeneratedPages {
        files: BTreeMap::from([("docs/intro.html".into(), b"<h1>Source output</h1>".to_vec())]),
        manifest: "{\"doc\":true}".into(),
    };
    for base in ["/NEPL3/", "/acceptance/project/"] {
        let config = SiteConfig {
            version: 1,
            base_path: base.into(),
        };
        let first = compose(
            &config,
            &registry,
            input(),
            "commit",
            "design",
            &RendererIdentity {
                executable_sha256: "test-executable".into(),
                build_rustc: "test-compiler",
            },
        )?;
        let second = compose(
            &config,
            &registry,
            input(),
            "commit",
            "design",
            &RendererIdentity {
                executable_sha256: "test-executable".into(),
                build_rustc: "test-compiler",
            },
        )?;
        assert_eq!(first.files, second.files);
        assert_eq!(first.manifest, second.manifest);
        assert_eq!(first.files["docs/intro.html"], b"<h1>Source output</h1>");
        assert_eq!(first.files["doc-manifest.json"], b"{\"doc\":true}");
        let index = std::str::from_utf8(&first.files["index.html"])?;
        assert!(index.contains(&format!("href=\"{base}docs/intro.html\"")));
        assert!(!index.contains("<script"));
        assert!(!index.contains("{{"));
        let manifest: serde_json::Value = serde_json::from_str(&first.manifest)?;
        let files = manifest["files"].as_array().ok_or("files array missing")?;
        assert_eq!(files.len(), first.files.len());
        for entry in files {
            let bytes = &first.files[entry["path"].as_str().ok_or("file path missing")?];
            assert_eq!(entry["sha256"], hash(bytes));
            assert_eq!(entry["bytes"], bytes.len());
        }
        let mut missing = input();
        missing.files.clear();
        assert!(
            compose(
                &config,
                &registry,
                missing,
                "commit",
                "design",
                &RendererIdentity {
                    executable_sha256: "test-executable".into(),
                    build_rustc: "test-compiler"
                }
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn collisions_do_not_overwrite_generated_documents() {
    for existing in [
        "index.html",
        "INDEX.HTML",
        "index.html/child",
        "INDEX.HTML/child",
    ] {
        let mut files = BTreeMap::from([(existing.into(), b"original".to_vec())]);
        assert!(insert(&mut files, "index.html", b"replacement".to_vec()).is_err());
        assert_eq!(files[existing], b"original");
    }
    let mut files = BTreeMap::from([("assets".into(), Vec::new())]);
    assert!(insert(&mut files, "assets/site.css", Vec::new()).is_err());
}
