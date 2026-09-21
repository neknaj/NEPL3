use super::*;
use nepl3_core::syntax::FieldValue;

#[test]
fn recursive_packages_keep_contexts_source_and_print() -> Result<(), String> {
    let input = "add framed frame neg 7 2";
    let result = inspect(input, true)?;
    let ParseOutcome::Complete { tree, cursor, .. } = &result.parse.outcome else {
        return Err(format!("{:?}", result.parse));
    };
    assert_eq!(*cursor, input.len() as u64);
    assert!(result.parse.report.diagnostics.is_empty());
    assert_eq!(tree.bundle.nodes[0].kind, "Add");
    let FieldValue::Foreign(frame) = &tree.bundle.nodes[1].fields[0] else {
        return Err("Frame boundary".into());
    };
    assert_eq!(frame.bundle.nodes[0].kind, "Frame");
    assert_eq!(frame.bundle.nodes[0].schema.package, "org.example.frame");
    let FieldValue::Foreign(expr) = &frame.bundle.nodes[0].fields[0] else {
        return Err("Expr reentry".into());
    };
    assert_eq!(expr.bundle.nodes[0].kind, "Neg");
    assert_eq!(
        expr.bundle.nodes[0].schema.package,
        "org.example.miniexpr.framed"
    );
    assert_eq!(
        (
            expr.bundle.tokens[1].head.start(),
            expr.bundle.tokens[1].head.end()
        ),
        (21, 22)
    );
    assert_eq!(
        result.printed.ok_or("print")?.outcome,
        print::PrintOutcome::Complete(input.into())
    );
    assert_eq!(tree.contexts.len(), 3);
    assert_eq!(tree.contexts[0].nodes[0].entry.alias, "Expr");
    assert!(
        tree.contexts
            .iter()
            .any(|c| c.nodes[0].entry.alias == "Frame" && c.nodes[0].entry.mode == "FrameCode")
    );
    let nested = inspect("framed frame framed frame 7", true)?;
    let ParseOutcome::Complete { tree, .. } = nested.parse.outcome else {
        return Err("recursive reentry".into());
    };
    assert_eq!(tree.contexts.len(), 5);
    Ok(())
}

#[test]
fn nested_failures_and_unfinished_input_remain_observable() -> Result<(), String> {
    for input in [
        "framed frame add 1",
        "framed unknown",
        "framed frame unknown",
        "framed",
        "framed frame",
    ] {
        let result = inspect(input, true).map_err(|e| format!("{input}: {e}"))?;
        assert!(
            matches!(result.parse.outcome, ParseOutcome::Recovered { .. }),
            "{result:?}"
        );
        assert!(!result.parse.report.diagnostics.is_empty());
        assert!(result.printed.is_none());
    }
    let result = inspect("framed frame add 1 ", false)?;
    assert!(matches!(
        result.parse.outcome,
        ParseOutcome::NeedMore { .. }
    ));
    assert!(result.printed.is_none());
    let unicode = inspect("framed frame 未知", true)?;
    let span = unicode
        .parse
        .report
        .diagnostics
        .first()
        .and_then(|d| d.primary.as_ref())
        .ok_or("primary")?;
    assert_eq!((span.start(), span.end()), (13, 19));
    super::super::parse::with_languages(
        "framed frame 7",
        true,
        true,
        languages("Expr", "Frame")?,
        ("Expr", "cancel"),
        |reply, _, _, _| {
            assert!(matches!(reply.outcome, ParseOutcome::Stopped { .. }));
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn recovered_foreign_roots_survive_wire_and_reject_forged_selection() -> Result<(), String> {
    use nepl3_core::source::SourceStore;
    use nepl3_engine::{portable, selection::ShapeSelection};
    use nepl3_wire::foundation::FoundationCodec;
    for input in [
        "framed unknown",
        "framed",
        "framed frame unknown",
        "framed frame",
    ] {
        super::super::parse::with_languages(
            input,
            true,
            false,
            languages("Expr", "Frame")?,
            ("Expr", "recovery"),
            |reply, profile, registry, codec| {
                let ParseOutcome::Recovered { tree, .. } = reply.outcome else {
                    return Err("recovered".into());
                };
                let value = portable::tree::to_value(&tree, profile, codec, &mut budget())
                    .map_err(error)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
                let decoded = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
                let sources = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(registry, &sources, &mut admission).map_err(error)?;
                let restored =
                    portable::tree::from_value(&decoded, profile, &mut receiver, &mut budget())
                        .map_err(error)?;
                assert_eq!(
                    portable::tree::to_value(&restored, profile, &mut receiver, &mut budget())
                        .map_err(error)?,
                    value
                );
                let mut corrupt = restored.clone();
                corrupt.recovery.clear();
                assert!(
                    corrupt
                        .validate(profile, &mut budget(), &mut SourceAdmission::default())
                        .is_err()
                );
                let mut corrupt = restored.clone();
                let selected = corrupt
                    .contexts
                    .iter_mut()
                    .flat_map(|c| &mut c.nodes)
                    .find(|n| matches!(n.shape, ShapeSelection::Recovery))
                    .ok_or("recovery selection")?;
                selected.entry.alias = "unregistered".into();
                assert!(
                    corrupt
                        .validate(profile, &mut budget(), &mut SourceAdmission::default())
                        .is_err()
                );
                let mut corrupt = restored;
                let FieldValue::Foreign(foreign) = &mut corrupt.bundle.nodes[0].fields[0] else {
                    return Err("boundary".into());
                };
                foreign.category = "wrong".into();
                assert!(
                    corrupt
                        .validate(profile, &mut budget(), &mut SourceAdmission::default())
                        .is_err()
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn aliases_are_host_selected_and_missing_package_is_rejected() -> Result<(), String> {
    let packages = languages("Arithmetic", "Container")?;
    super::super::parse::with_languages(
        "framed frame 7",
        true,
        false,
        packages,
        ("Arithmetic", "renamed"),
        |reply, profile, _, _| {
            let ParseOutcome::Complete { tree, .. } = reply.outcome else {
                return Err("complete".into());
            };
            tree.validate(profile, &mut budget(), &mut SourceAdmission::default())
                .map_err(error)?;
            Ok(())
        },
    )?;
    let mut packages = languages("Expr", "Frame")?;
    packages.packages.pop();
    assert!(
        super::super::parse::with_languages(
            "framed frame 7",
            true,
            false,
            packages,
            ("Expr", "missing"),
            |_, _, _, _| Ok(())
        )
        .is_err()
    );
    Ok(())
}
