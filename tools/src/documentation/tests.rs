use super::*;
use crate::testing::Fixture;

#[test]
fn historical_review_source_is_not_a_documentation_migration_target() {
    assert!(!scoped_path("conformance/results/review/AGENTS.md"));
    assert!(!scoped_path("conformance/results/review/source/lib.rs"));
    assert!(scoped_path("doc/development.md"));
    assert!(scoped_path("crates/foundation/core/src/lib.rs"));
}

fn fixture() -> Result<(Fixture, String)> {
    let files = Fixture::new()?;
    files.git()?;
    files.write("README.md", "# Root\n\n[Guide](doc/guide.md)\n")?;
    files.write(
        "doc/guide.md",
        "# Guide\n\n| A | B |\n| - | - |\n| 1 | 2 |\n",
    )?;
    files.write(
        "tools/src/lib.rs",
        "//! Rust documentation stays owned by Rust source.\n",
    )?;
    for path in CONTRACTS {
        files.write(path, "{}\n")?;
    }
    command(files.root(), "git", &["add", "--all"])?;
    command(
        files.root(),
        "git",
        &[
            "-c",
            "user.name=Inventory Test",
            "-c",
            "user.email=inventory@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "synthetic inventory baseline",
        ],
    )?;
    let commit = String::from_utf8(command(files.root(), "git", &["rev-parse", "HEAD"])?)?
        .trim()
        .to_owned();
    fs::create_dir_all(files.root().join("doc/migration/generated"))?;
    write(files.root(), &commit)?;
    Ok((files, commit))
}

#[test]
fn real_checker_rejects_omitted_page_digest_and_structure_tampering() -> Result<()> {
    let (files, _) = fixture()?;
    check(files.root(), true)?;
    let original: serde_json::Value = json(files.root(), OUTPUT)?;
    for mutation in 0..3 {
        let mut value = original.clone();
        match mutation {
            0 => {
                value["pages"]
                    .as_array_mut()
                    .ok_or("fixture pages missing")?
                    .remove(0);
            }
            1 => value["pages"][0]["file"]["sha256"] = serde_json::json!("0".repeat(64)),
            _ => value["pages"][0]["structure"]["elements"]["heading"] = serde_json::json!(999),
        }
        files.json(OUTPUT, &value)?;
        assert!(check(files.root(), false).is_err());
    }
    Ok(())
}

#[test]
fn real_checker_reports_added_changed_removed_and_rejects_current_claim() -> Result<()> {
    let (files, _) = fixture()?;
    let saved: Inventory = json(files.root(), OUTPUT)?;
    files.write("doc/new.md", "# New\n")?;
    files.write("README.md", "# Changed\n")?;
    fs::remove_file(files.root().join("doc/guide.md"))?;
    assert_eq!(
        delta(files.root(), &saved)?,
        Delta {
            added: vec!["doc/new.md".into()],
            changed: vec!["README.md".into()],
            removed: vec!["doc/guide.md".into()]
        }
    );
    check(files.root(), false)?;
    assert!(check(files.root(), true).is_err());
    Ok(())
}

#[test]
fn commit_reference_and_missing_history_are_not_silently_accepted() -> Result<()> {
    let files = Fixture::new()?;
    files.git()?;
    for bad in [
        "HEAD",
        "--help",
        "25a096b",
        "0000000000000000000000000000000000000000",
    ] {
        assert!(snapshot_paths(files.root(), bad).is_err());
    }
    Ok(())
}

#[test]
fn git_symlink_mode_is_rejected_without_creating_an_os_symlink() -> Result<()> {
    let (files, _) = fixture()?;
    files.write("link-target", "README.md")?;
    let hash = String::from_utf8(command(
        files.root(),
        "git",
        &["hash-object", "-w", "link-target"],
    )?)?
    .trim()
    .to_owned();
    command(
        files.root(),
        "git",
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "120000",
            &hash,
            "doc/link.md",
        ],
    )?;
    command(
        files.root(),
        "git",
        &[
            "-c",
            "user.name=Inventory Test",
            "-c",
            "user.email=inventory@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "synthetic symlink tree entry",
        ],
    )?;
    let commit = String::from_utf8(command(files.root(), "git", &["rev-parse", "HEAD"])?)?
        .trim()
        .to_owned();
    assert!(generate(files.root(), &commit).is_err());
    Ok(())
}

#[test]
fn nonregular_output_and_oversized_current_input_are_rejected() -> Result<()> {
    let (files, commit) = fixture()?;
    fs::remove_file(files.root().join(OUTPUT))?;
    fs::create_dir(files.root().join(OUTPUT))?;
    assert!(write(files.root(), &commit).is_err());
    fs::remove_dir(files.root().join(OUTPUT))?;
    write(files.root(), &commit)?;
    files.write("doc/huge.md", vec![b'x'; 1024 * 1024 + 1])?;
    assert!(check(files.root(), false).is_err());
    Ok(())
}

fn commit(files: &Fixture) -> Result<String> {
    command(files.root(), "git", &["add", "--all"])?;
    command(
        files.root(),
        "git",
        &[
            "-c",
            "user.name=Inventory Test",
            "-c",
            "user.email=inventory@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "snapshot ownership fixture",
        ],
    )?;
    Ok(
        String::from_utf8(command(files.root(), "git", &["rev-parse", "HEAD"])?)?
            .trim()
            .to_owned(),
    )
}
fn registry(source: &str, projection: &str, aliases: &str, renderer: &str) -> serde_json::Value {
    serde_json::json!({"version":1,"pages":[{"id":"test-page", "source":source,
        "projection":projection,"aliases":aliases,"route":if renderer == "nepl3-tools.markdown-footnotes-pages/1" { projection } else { "test.html" },"renderer":renderer}]})
}

#[test]
fn snapshot_registry_owns_projection_and_all_native_inputs_are_fingerprinted() -> Result<()> {
    let (files, _) = fixture()?;
    files.write("doc/example.nepld", "article en \"Title\" body nil\n")?;
    files.write(
        "doc/example.md",
        "<!-- Generated from doc/example.nepld -->\n# Title\n",
    )?;
    files.write("doc/alias-map.json", "{}\n")?;
    files.json(
        "doc/canonical.json",
        &registry(
            "doc/example.nepld",
            "doc/example.md",
            "doc/alias-map.json",
            "nepl3-tools.markdown-annotated/4",
        ),
    )?;
    files.write("interfaces/sentence.json", "{}\n")?;
    files.write("conformance/results/stages/T07.json", "{}\n")?;
    let baseline = commit(&files)?;
    let saved = generate(files.root(), &baseline)?;
    assert_eq!(saved.schema, "nepl3.doc-inventory/3");
    let page = saved
        .pages
        .iter()
        .find(|p| p.file.path == "doc/example.md")
        .ok_or("projection")?;
    assert_eq!(page.classification, "registered-doc-projection");
    assert_eq!(page.canonical_source.as_deref(), Some("doc/example.nepld"));
    assert_eq!(
        page.canonical_registry.as_deref(),
        Some("doc/canonical.json")
    );
    assert_eq!(page.stable_page_id.as_deref(), Some("test-page"));
    assert_eq!(page.declared_route.as_deref(), Some("test.html"));
    assert_eq!(
        page.declared_renderer.as_deref(),
        Some("nepl3-tools.markdown-annotated/4")
    );
    assert_eq!(
        page.published_url, None,
        "declared routes do not prove deployment"
    );
    assert!(
        saved
            .document_sources
            .iter()
            .any(|f| f.path == "doc/example.nepld")
    );
    for path in [
        "doc/canonical.json",
        "doc/alias-map.json",
        "interfaces/sentence.json",
    ] {
        assert!(saved.capability_contracts.iter().any(|f| f.path == path));
    }
    assert!(
        saved
            .exclusions
            .iter()
            .any(|e| e.path == "conformance/results/stages/T07.json" && !e.reason.is_empty())
    );
    let classified: BTreeSet<_> = saved
        .pages
        .iter()
        .map(|p| p.file.path.clone())
        .chain(saved.rustdoc_sources.iter().map(|f| f.path.clone()))
        .chain(saved.document_sources.iter().map(|f| f.path.clone()))
        .chain(saved.exclusions.iter().map(|e| e.path.clone()))
        .collect();
    assert_eq!(classified, snapshot_paths(files.root(), &baseline)?);
    files.write("doc/example.nepld", "changed source\n")?;
    files.write("doc/alias-map.json", "{\"changed\":true}\n")?;
    files.write("interfaces/sentence.json", "{\"changed\":true}\n")?;
    files.write("doc/canonical.json", "invalid live registry\n")?;
    // Re-read immutable blobs; the working registry cannot change the old owner.
    assert_eq!(generate(files.root(), &baseline)?, saved);
    let changed = delta(files.root(), &saved)?;
    assert_eq!(
        changed.changed,
        vec![
            "doc/alias-map.json",
            "doc/canonical.json",
            "doc/example.nepld",
            "interfaces/sentence.json"
        ]
    );
    std::fs::remove_file(files.root().join("doc/alias-map.json"))?;
    assert_eq!(
        delta(files.root(), &saved)?.removed,
        vec!["doc/alias-map.json"]
    );
    Ok(())
}

#[test]
fn standalone_registry_is_relative_and_unregistered_header_is_not_authority() -> Result<()> {
    let (files, _) = fixture()?;
    files.write("examples/document/sample/example.nepld", "source\n")?;
    files.write(
        "examples/document/sample/example.md",
        "<!-- Generated from example.nepld -->\n# Example\n",
    )?;
    files.write("examples/document/sample/aliases.json", "{}\n")?;
    files.json(
        "examples/document/sample/example.pages.json",
        &registry(
            "example.nepld",
            "example.md",
            "aliases.json",
            "nepl3-tools.markdown-footnotes-pages/1",
        ),
    )?;
    files.write(
        "doc/orphan.md",
        "<!-- Generated from ../../untrusted.nepld -->\n# Orphan\n",
    )?;
    let baseline = commit(&files)?;
    let saved = generate(files.root(), &baseline)?;
    let page = saved
        .pages
        .iter()
        .find(|p| p.file.path == "examples/document/sample/example.md")
        .ok_or("projection")?;
    assert_eq!(
        page.canonical_source.as_deref(),
        Some("examples/document/sample/example.nepld")
    );
    assert_eq!(
        page.canonical_registry.as_deref(),
        Some("examples/document/sample/example.pages.json")
    );
    let orphan = saved
        .pages
        .iter()
        .find(|p| p.file.path == "doc/orphan.md")
        .ok_or("orphan")?;
    assert_eq!(orphan.classification, "unregistered-generated-projection");
    assert_eq!(orphan.canonical_source, None);
    assert_eq!(orphan.stable_page_id, None);
    assert_eq!(orphan.declared_route, None);
    // A second manifest cannot silently take ownership of the same projection.
    files.json(
        "doc/canonical.json",
        &registry(
            "examples/document/sample/example.nepld",
            "examples/document/sample/example.md",
            "examples/document/sample/aliases.json",
            "nepl3-tools.markdown-footnotes-pages/1",
        ),
    )?;
    let duplicate = commit(&files)?;
    assert!(generate(files.root(), &duplicate).is_err());
    Ok(())
}

#[test]
fn missing_or_invalid_registry_input_is_not_treated_as_authored_markdown() -> Result<()> {
    let (files, _) = fixture()?;
    files.write("doc/canonical.json", "{\"version\":99,\"pages\":[]}")?;
    let invalid = commit(&files)?;
    assert!(generate(files.root(), &invalid).is_err());
    files.json(
        "doc/canonical.json",
        &registry(
            "doc/missing.nepld",
            "doc/guide.md",
            "doc/missing.aliases.json",
            "nepl3-tools.markdown-annotated/4",
        ),
    )?;
    let missing = commit(&files)?;
    assert!(generate(files.root(), &missing).is_err());
    Ok(())
}
