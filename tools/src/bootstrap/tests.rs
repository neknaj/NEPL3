use super::*;
use nepl3_core::budget::Limits;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 1_000_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn seed(path: &str) -> crate::Result<Vec<u8>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("workspace")?;
    crate::command(
        root,
        "python",
        &[
            "tools/bootstrap/grammar.py",
            path,
            "--source-id",
            path,
            "--uri",
            &format!("repository:{path}"),
        ],
    )
}
#[test]
fn supplied_grammar_sources_load_complete_typed_constructor_arenas() -> crate::Result<()> {
    for path in [
        "languages/grammar/syntax.neplg",
        "languages/doc/syntax.neplg",
        "languages/math/syntax.neplg",
        "languages/circuit/syntax.neplg",
        "examples/grammar/angle-tag.neplg",
    ] {
        let bytes = seed(path)?;
        let document = load(&bytes, &mut budget(), &mut SourceAdmission::default())
            .map_err(|e| format!("{path}: {e:?}"))?;
        let root = document.node(document.root).map_err(|e| format!("{e:?}"))?;
        let NodeKind::Language { declarations, .. } = &root.kind else {
            return Err("Language root".into());
        };
        assert!(!declarations.items.is_empty());
        assert!(!document.sources[0].text().is_empty());
        assert!(
            document
                .nodes
                .iter()
                .all(|v| v.span.snapshot_ref() == document.sources[0].identity())
        );
    }
    Ok(())
}
#[test]
fn seed_rejects_forged_source_constructor_literal_and_list_boundaries() -> crate::Result<()> {
    let bytes = seed("examples/grammar/angle-tag.neplg")?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let mut cases = vec![];
    let mut bad = value.clone();
    bad["source"]["digest"] = Value::String("00".repeat(32));
    cases.push(bad);
    let mut bad = value.clone();
    bad["root"]["kind"] = Value::String("Category".into());
    cases.push(bad);
    let mut bad = value.clone();
    bad["root"]["fields"]["name"]["value"] = Value::String("Forged".into());
    cases.push(bad);
    let mut bad = value.clone();
    bad["root"]["fields"]["declarations"]["heads"][0] = serde_json::json!([0, 1]);
    cases.push(bad);
    for bad in cases {
        assert!(
            load(
                &serde_json::to_vec(&bad)?,
                &mut budget(),
                &mut SourceAdmission::default()
            )
            .is_err()
        );
    }
    let mut limits = budget().limits();
    limits.allocation_units = 0;
    assert!(matches!(
        load(
            &bytes,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        ),
        Err(SeedError::Stopped(StopReason::AllocationLimit))
    ));
    assert!(
        load(
            br#"{"schema":"a","schema":"b"}"#,
            &mut budget(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    Ok(())
}
