//! Actual standard Doc source compilation and prefix parsing. Facts signatures
//! are compile-time declarations only; this parse-only host serves reader calls.
#[path = "doc/export.rs"]
mod export;
#[path = "doc/external.rs"]
mod external;
#[path = "doc/html.rs"]
mod html;
#[path = "doc/labels.rs"]
mod labels;
#[path = "doc/mixed.rs"]
mod mixed;
#[path = "doc/pages.rs"]
mod pages;
#[path = "doc/prepare.rs"]
mod prepare;
#[path = "doc/print.rs"]
mod print;
#[path = "doc/projection.rs"]
mod projection;
#[path = "doc/text.rs"]
mod text;
use nepl3_core::{budget::*, source::*};
use nepl3_core::{
    syntax::{Environment, EnvironmentEntry},
    value::NdfValue,
};
use nepl3_engine::profile::*;
use nepl3_reader::model::{ReadRequest, ReaderContext};
use nepl3_tools::doc::source::{
    Compiled, budget, compiled, err, parse_source_as, with_input, with_input_route,
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
#[test]
fn native_doc_host_preserves_owned_parse_tree_and_uses_less_allocation() -> Result<(), String> {
    let compiled = compiled()?;
    let source = include_str!("../../examples/document/line-break.nepld");
    let run = |native| {
        with_input_route(native, &compiled, source, "Article", |tree, _, b, _| {
            Ok((tree.tree().clone(), b.usage()))
        })
    };
    let (owned, owned_usage) = run(false)?;
    let (native, native_usage) = run(true)?;
    assert_eq!(native, owned);
    assert!(native_usage.allocation_units < owned_usage.allocation_units);
    println!("Doc identical tree: owned={owned_usage:?}; native={native_usage:?}");
    Ok(())
}
#[test]
fn doc_prefix_original_annotation_examples_use_actual_parser_and_lower() -> Result<(), String> {
    use nepl3_doc_core::{
        check::{Category, ShapeError},
        lower::{self, LowerError},
        model::*,
    };
    let compiled = compiled()?;
    for (source, valid) in [
        (r#"ruby concat cons text "" nil text "r""#, false),
        (r#"anno text "a" nil"#, false),
        (
            r#"sentence cons text "a" cons ruby text "b" text "r" nil"#,
            true,
        ),
    ] {
        with_input(
            &compiled,
            source,
            if valid { "Sentence" } else { "Inline" },
            |tree, profile, b, a| {
                let bundle = tree
                    .tree()
                    .bundle
                    .validate_with_sources(profile.registry(), b, a)
                    .map_err(err)?;
                let result = lower::prefix(
                    &bundle,
                    &compiled.doc.package.schema,
                    if valid {
                        Category::Sentence
                    } else {
                        Category::Inline
                    },
                    profile.registry(),
                    b,
                    a,
                );
                if valid {
                    let value = result.map_err(err)?;
                    assert!(matches!(value.value.root, DocRoot::Sentence(_)));
                    assert!(
                        value
                            .value
                            .nodes
                            .iter()
                            .any(|n| matches!(n.kind, DocKind::Ruby { .. }))
                    );
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(LowerError::Shape(
                                ShapeError::EmptyAnnotationPart(_) | ShapeError::AnnotationNotes(_)
                            ))
                        ),
                        "{result:?}"
                    );
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn doc_auxiliary_fragments_and_parent_operands_have_typed_boundaries() -> Result<(), String> {
    use nepl3_doc_core::{
        check::{Category, ShapeError},
        lower::{self, LowerError},
        model::*,
    };
    let compiled = compiled()?;
    // Expectations are the explicit signatures and U64/Bytes32 operand ranges,
    // not values generated from the lowerer's current output.
    for (category, root, source) in [
        ("Alignment", Category::Alignment, "left"),
        (
            "ListStyle",
            Category::ListStyle,
            "ordered 18446744073709551615",
        ),
        ("Check", Category::Check, "checked"),
        ("OptionalText", Category::OptionalText, r#"some "lang""#),
        ("OptionalRow", Category::OptionalRow, "some row nil"),
        (
            "OptionalSentence",
            Category::OptionalSentence,
            "some sentence nil",
        ),
        (
            "LinkTarget",
            Category::Target,
            r#"page "guide" some "part""#,
        ),
        ("Asset", Category::Asset, r#"asset "image" none"#),
        (
            "Block",
            Category::Block,
            "table cons left nil some row cons sentence nil nil cons row cons sentence nil nil nil",
        ),
        (
            "Block",
            Category::Block,
            "list ordered 3 cons item unchecked body nil nil",
        ),
        (
            "Inline",
            Category::Inline,
            r#"link external "https://example.invalid/" text "label""#,
        ),
        (
            "Block",
            Category::Block,
            r#"image asset "asset-id" none sentence nil some sentence nil"#,
        ),
    ] {
        with_input(&compiled, source, category, |tree, profile, b, a| {
            let bundle = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let doc = lower::prefix(
                &bundle,
                &compiled.doc.package.schema,
                root,
                profile.registry(),
                b,
                a,
            )
            .map_err(err)?;
            if category == "Block" || category == "Inline" {
                assert!(!doc.value.nodes.iter().any(|n| matches!(
                    n.kind,
                    DocKind::Alignment { .. }
                        | DocKind::OptionalRow { .. }
                        | DocKind::ListStyle { .. }
                        | DocKind::Check { .. }
                        | DocKind::Target { .. }
                        | DocKind::Asset { .. }
                        | DocKind::OptionalText { .. }
                        | DocKind::OptionalSentence { .. }
                )));
            }
            assert!(!doc.origins.is_empty());
            assert!(!doc.views.is_empty());
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let wire = nepl3_doc_core::portable::to_value(&doc, profile.registry(), &mut codec, b)
                .map_err(err)?;
            let bytes = nepl3_wire::encode(&wire, b).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, b).map_err(err)?;
            let mut fresh = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let received = nepl3_doc_core::portable::from_value(
                &value,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_doc_retention(&doc, &received);
            assert_eq!(
                nepl3_doc_core::portable::to_value(
                    &received,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                )
                .map_err(err)?,
                value
            );
            Ok(())
        })?;
    }
    for (source, category, root, error) in [
        (
            "ordered 18446744073709551616",
            "ListStyle",
            Category::ListStyle,
            0,
        ),
        (
            "table cons left nil some row nil nil",
            "Block",
            Category::Block,
            1,
        ),
        (r#"asset "a" some "abc""#, "Asset", Category::Asset, 2),
    ] {
        with_input(&compiled, source, category, |tree, profile, b, a| {
            let bundle = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let result = lower::prefix(
                &bundle,
                &compiled.doc.package.schema,
                root,
                profile.registry(),
                b,
                a,
            );
            assert!(
                match error {
                    0 => matches!(result, Err(LowerError::NaturalRange { .. })),
                    1 => matches!(result, Err(LowerError::Shape(ShapeError::TableWidth(_)))),
                    _ => matches!(result, Err(LowerError::AssetDigest { .. })),
                },
                "{result:?}"
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn doc_code_same_alias_keeps_invalid_guest_meaning_and_restores_host() -> Result<(), String> {
    use nepl3_doc_core::{
        check::{Category, ShapeError},
        lower::{self, LowerError},
        model::*,
    };
    let compiled = compiled()?;
    // Guest syntax is valid, but its Ruby's base has no visible content. Code
    // must retain it without requiring a successful semantic Doc lower.
    let source = r#"paragraph cons code Doc article en sentence nil body cons paragraph cons sentence cons ruby text "" text "r" nil nil nil cons sentence cons text "host-tail" nil nil"#;
    with_input(&compiled, source, "Block", |tree, profile, b, a| {
        assert_eq!(tree.tree().contexts.len(), 2);
        let host = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let doc = lower::prefix(
            &host,
            &compiled.doc.package.schema,
            Category::Block,
            profile.registry(),
            b,
            a,
        )
        .map_err(err)?;
        assert!(
            doc.value
                .nodes
                .iter()
                .any(|n| matches!(&n.kind,DocKind::Text{text} if text=="host-tail"))
        );
        assert_eq!(doc.value.embeds.len(), 1);
        let embed = &doc.value.embeds[0];
        assert_eq!(embed.kind, EmbedKind::Code);
        assert_eq!(embed.closure.syntax.schema, compiled.doc.package.schema);
        assert_eq!(embed.closure.syntax.category, "Article");
        let guest = embed
            .closure
            .syntax
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        assert!(matches!(
            lower::prefix(
                &guest,
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                a
            ),
            Err(LowerError::Shape(ShapeError::EmptyAnnotationPart(_)))
        ));
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let value = nepl3_doc_core::portable::to_value(&doc, profile.registry(), &mut codec, b)
            .map_err(err)?;
        let received =
            nepl3_doc_core::portable::from_value(&value, profile.registry(), &mut codec, b)
                .map_err(err)?;
        assert_doc_retention(&doc, &received);
        assert_eq!(
            nepl3_doc_core::portable::to_value(&received, profile.registry(), &mut codec, b)
                .map_err(err)?,
            value
        );
        Ok(())
    })
}

fn assert_doc_retention(
    before: &nepl3_doc_core::model::DocumentSyntax,
    after: &nepl3_doc_core::model::DocumentSyntax,
) {
    // Source tables and guest graph coordinates have the common canonical wire
    // order. Domain nodes and their original provenance are unchanged.
    assert_eq!(before.value.root, after.value.root);
    assert_eq!(before.value.nodes, after.value.nodes);
    assert_eq!(before.origins, after.origins);
    assert_eq!(before.views, after.views);
    assert_eq!(before.source_maps, after.source_maps);
    assert_eq!(before.sources.len(), after.sources.len());
    for source in &before.sources {
        assert!(after.sources.iter().any(|other| other == source));
    }
    assert_eq!(before.value.embeds.len(), after.value.embeds.len());
    for (before, after) in before.value.embeds.iter().zip(&after.value.embeds) {
        assert_eq!(before.kind, after.kind);
        assert_eq!(
            before.closure.owner_environment,
            after.closure.owner_environment
        );
        assert_eq!(before.closure.owner_origins, after.closure.owner_origins);
        assert_eq!(
            before.closure.owner_source_maps,
            after.closure.owner_source_maps
        );
    }
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn doc_suite_seed_artifacts_match_the_original_sources() -> Result<(), String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("root")?;
    for (name, stored) in [
        (
            "doc",
            include_bytes!("../../conformance/fixtures/doc/syntax.json").as_slice(),
        ),
        (
            "math",
            include_bytes!("../../conformance/fixtures/doc/math.json").as_slice(),
        ),
        (
            "circuit",
            include_bytes!("../../conformance/fixtures/doc/circuit.json").as_slice(),
        ),
        (
            "grammar",
            include_bytes!("../../conformance/fixtures/doc/grammar.json").as_slice(),
        ),
    ] {
        let path = format!("languages/{name}/syntax.neplg");
        let output = std::process::Command::new("python")
            .arg(root.join("tools/bootstrap/grammar.py"))
            .arg(root.join(&path))
            .args([
                "--source-id",
                &path,
                "--uri",
                &format!("memory:{name}-grammar"),
            ])
            .env("PYTHONIOENCODING", "utf-8")
            .output()
            .map_err(err)?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        // JSON whitespace is transport framing; the embedded original source
        // bytes/identity and every typed constructor must be exactly equal.
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(err)?;
        let saved: serde_json::Value = serde_json::from_slice(stored).map_err(err)?;
        assert_eq!(actual, saved, "{name}");
    }
    Ok(())
}

#[test]
fn doc_prefix_limits_preserve_typed_stops_and_caller_depth() -> Result<(), String> {
    use nepl3_doc_core::{
        check::Category,
        lower::{self, LowerError},
    };
    let compiled = compiled()?;
    with_input(
        &compiled,
        r#"strong strong ruby text "base" text "reading""#,
        "Inline",
        |tree, profile, b, a| {
            let bundle = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let mut stopped = 0;
            let mut completed = 0;
            for selector in 0..5 {
                for cap in [0, 1, 8, 64, 512, 4096, 32768, 262144] {
                    let mut limits = budget().limits();
                    let expected = match selector {
                        0 => {
                            limits.work = cap;
                            StopReason::WorkLimit
                        }
                        1 => {
                            limits.allocation_units = cap;
                            StopReason::AllocationLimit
                        }
                        2 => {
                            limits.source_bytes = cap;
                            StopReason::SourceLimit
                        }
                        3 => {
                            limits.nodes = cap;
                            StopReason::NodeLimit
                        }
                        _ => {
                            limits.depth = cap;
                            StopReason::DepthLimit
                        }
                    };
                    let mut operation = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let result = operation.with_depth_at_least(7, |b| {
                        lower::prefix(
                            &bundle,
                            &compiled.doc.package.schema,
                            Category::Inline,
                            profile.registry(),
                            b,
                            &mut admission,
                        )
                    });
                    assert_eq!(operation.current_depth(), 0);
                    match result {
                        Ok(doc) => {
                            completed += 1;
                            doc.validate_structure(
                                profile.registry(),
                                &mut budget(),
                                &mut SourceAdmission::default(),
                            )
                            .map_err(err)?;
                        }
                        Err(LowerError::Stopped(reason)) => {
                            stopped += 1;
                            assert_eq!(reason, expected);
                            assert_eq!(operation.poll(), Err(expected));
                        }
                        Err(other) => {
                            return Err(format!("cap {cap}, resource {selector}: {other:?}"));
                        }
                    }
                }
            }
            assert!(stopped > 0 && completed > 0);
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                lower::prefix(
                    &bundle,
                    &compiled.doc.package.schema,
                    Category::Inline,
                    profile.registry(),
                    &mut cancelled,
                    &mut SourceAdmission::default()
                ),
                Err(LowerError::Stopped(StopReason::Cancelled))
            ));
            Ok(())
        },
    )
}

#[test]
fn doc_sentence_provider_payload_and_prefix_share_the_actual_normal_form() -> Result<(), String> {
    use nepl3_doc_core::{check::Category, lower, model::*};
    let compiled = compiled()?;
    let literal = with_input(
        &compiled,
        r#""これは{[文書/ぶんしょ]/document}を記述する。""#,
        "Sentence",
        |tree, profile, b, a| {
            let bundle = &tree.tree().bundle;
            let node = &bundle.nodes[bundle.root.0 as usize];
            assert_eq!(node.kind, "Leaf:SentenceLiteral");
            let token = node
                .token
                .and_then(|t| bundle.tokens.get(t.0 as usize))
                .ok_or("literal token")?;
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let owner = bundle
                .sources
                .iter()
                .find(|s| s.identity() == token.head.snapshot_ref())
                .ok_or("owner source")?;
            let value = nepl3_doc_core::portable::sentence::from_value(
                &token.payload,
                owner,
                profile.registry(),
                &mut codec,
                b,
            )
            .map_err(err)?;
            assert_eq!(value.views[0].head, token.head);
            assert_eq!(value.views[0].view, token.views);
            Ok(value)
        },
    )?;
    let prefix = with_input(
        &compiled,
        r#"sentence cons text "これは" cons anno ruby text "文書" text "ぶんしょ" cons text "document" nil cons text "を記述する。" nil"#,
        "Sentence",
        |tree, profile, b, a| {
            let bundle = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            lower::prefix(
                &bundle,
                &compiled.doc.package.schema,
                Category::Sentence,
                profile.registry(),
                b,
                a,
            )
            .map_err(err)
        },
    )?;
    assert_eq!(literal.value.root, prefix.value.root);
    assert_eq!(
        literal
            .value
            .nodes
            .iter()
            .map(|n| &n.kind)
            .collect::<Vec<_>>(),
        prefix
            .value
            .nodes
            .iter()
            .map(|n| &n.kind)
            .collect::<Vec<_>>()
    );
    assert!(matches!(literal.value.root, DocRoot::Sentence(_)));
    Ok(())
}

#[test]
fn doc_sentence_provider_failures_have_typed_report_positions_and_stops() -> Result<(), String> {
    use nepl3_reader::model::ReadReply;
    let compiled = compiled()?;
    let r = &compiled.doc.registry;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let source = SourceSnapshot::new(
        SourceId("literal-failure".into()),
        0,
        "memory:literal-failure".into(),
        "\"前[文/ぶん".as_bytes().to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(err)?;
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(
        &value,
        r.selected("nepl3.foundation", 1).ok_or("foundation")?,
        r,
        &mut b,
    )
    .map_err(err)?;
    let context = ReaderContext {
        schema: compiled.doc.package.schema.clone(),
        category: "Sentence".into(),
        mode: "Code".into(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value,
        },
    };
    let operation = nepl3_tools::doc::reader::signature(r, &mut b)
        .map_err(err)?
        .operation;
    let mut codec = FoundationCodec::new(r, &store, &mut a).map_err(err)?;
    let checked = context.check(&mut codec, &store, r, &mut b).map_err(err)?;
    let reply = nepl3_tools::doc::reader::read(
        &operation,
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: source.text().len() as u64,
            final_input: true,
            context: &checked,
            state: &NdfValue::Unit,
        },
        r,
        &store,
        &mut b,
        &mut a,
    )
    .map_err(err)?;
    let ReadReply::Failed {
        diagnostic, report, ..
    } = reply
    else {
        return Err(format!("{reply:?}"));
    };
    assert_eq!(diagnostic.code, "UnclosedAnnotation");
    assert_eq!(report.diagnostics, vec![diagnostic.clone()]);
    let primary = diagnostic.primary.as_ref().ok_or("primary")?;
    assert_eq!((primary.start(), primary.end()), (15, 15));
    let opening = diagnostic.related[0].span.as_ref().ok_or("opening")?;
    assert_eq!((opening.start(), opening.end()), (4, 5));
    report.validate(&store, &[], r, &mut b).map_err(err)?;
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        nepl3_tools::doc::reader::read(
            &operation,
            ReadRequest {
                snapshot: &source,
                start: 0,
                limit: source.text().len() as u64,
                final_input: true,
                context: &checked,
                state: &NdfValue::Unit
            },
            r,
            &store,
            &mut stopped,
            &mut a
        )
        .map_err(err)?,
        ReadReply::Stopped {
            reason: StopReason::WorkLimit,
            ..
        }
    ));
    Ok(())
}
