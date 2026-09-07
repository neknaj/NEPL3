//! Actual standard Doc source compilation and prefix parsing. Facts signatures
//! are compile-time declarations only; this parse-only host serves reader calls.
#[path = "doc/labels.rs"]
mod labels;
#[path = "doc/mixed.rs"]
mod mixed;
#[path = "doc/prepare.rs"]
mod prepare;
#[path = "doc/print.rs"]
mod print;
#[path = "doc/text.rs"]
mod text;
use nepl3_core::{budget::*, source::*};
use nepl3_core::{
    syntax::{Environment, EnvironmentEntry},
    value::NdfValue,
};
use nepl3_engine::{parse::*, profile::*, tree::ValidatedParseTree};
use nepl3_grammar_core::compile::package::CompiledLanguage;
use nepl3_reader::{
    model::{ProviderCall, ReadRequest, ReaderContext},
    runtime::ProviderReply,
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
fn with_input<T>(
    compiled: &Compiled,
    input: &str,
    category: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let r = &compiled.doc.registry;
    let packages: Vec<_> = std::iter::once(&compiled.doc.package)
        .chain(compiled.others.iter())
        .collect();
    // Each runtime registration identifies the actual source serving that operation.
    let implementation_for = |operation: &nepl3_core::value::OperationRef| {
        if operation.schema.package == "nepl3.doc.reader" {
            Digest::of(include_bytes!("../src/doc/reader.rs"))
        } else {
            Digest::of(include_bytes!(
                "../../crates/foundation/reader/src/builtin/provider.rs"
            ))
        }
    };
    let mut operations = Vec::new();
    for operation in packages
        .iter()
        .flat_map(|p| p.reader.providers.iter().map(|v| &v.operation))
    {
        if !operations.contains(operation) {
            operations.push(operation.clone());
        }
    }
    let providers: Vec<_> = operations
        .iter()
        .map(|operation| ProviderImplementation {
            provider: operation.name.clone(),
            revision: 1,
            implementation_digest: implementation_for(operation),
            operations: vec![operation.clone()],
        })
        .collect();
    let requirements = operations
        .iter()
        .map(|operation| ProviderRequirement {
            provider: operation.name.clone(),
            revision: 1,
            implementation_digest: implementation_for(operation),
            operation: operation.clone(),
        })
        .collect();
    let mut schemas = packages
        .iter()
        .map(|p| p.schema.clone())
        .collect::<Vec<_>>();
    for schema in packages
        .iter()
        .flat_map(|p| p.payload_schemas.iter())
        .chain(
            [
                "nepl3.foundation",
                "nepl3.reader",
                "nepl3.engine",
                "nepl3.doc",
                "nepl3.doc.reader",
                "nepl3.grammar",
            ]
            .iter()
            .filter_map(|name| r.selected(name, 1)),
        )
    {
        if !schemas.contains(schema) {
            schemas.push(schema.clone());
        }
    }
    let profile = ParseProfile {
        id: "doc-prefix".into(),
        languages: packages
            .iter()
            .zip([
                ("Doc", "Article"),
                ("Math", "Expr"),
                ("Circuit", "Design"),
                ("Grammar", "Root"),
            ])
            .map(|(p, (alias, category))| {
                Ok(LanguageRegistration {
                    alias: alias.into(),
                    package: p
                        .check(r, &mut budget())
                        .and_then(|c| c.semantic_identity(&mut budget()))
                        .map_err(err)?,
                    default_category: category.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
        schemas,
        head_providers: vec![],
        category_modes: vec![],
        providers: requirements,
        allowlist: operations,
        resources: vec![],
        limits: budget().limits(),
    };
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &[],
            },
            r,
            &mut budget(),
        )
        .map_err(err)?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let source = SourceSnapshot::new(
        SourceId("doc-input".into()),
        0,
        "memory:doc-input".into(),
        input.as_bytes().to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let tree = parse_source(&source, &resolved, category, &mut b, &mut a)?;
    let checked = tree.validate(&resolved, &mut b, &mut a).map_err(err)?;
    finish(&checked, &resolved, &mut b, &mut a)
}

fn parse_source(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    parse_source_as(source, resolved, "Doc", category, b, a)
}
fn parse_source_as(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    let r = resolved.registry();
    let foundation = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(err)?;
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&value, foundation, r, b).map_err(err)?;
    let mut contexts = Vec::new();
    for (alias, category) in [
        ("Doc", "Article"),
        ("Math", "Expr"),
        ("Circuit", "Design"),
        ("Grammar", "Root"),
    ] {
        let owner = resolved.language(alias, b).map_err(err)?;
        contexts.push(ReaderContext {
            schema: owner.schema.clone(),
            category: category.into(),
            mode: "Code".into(),
            origins: vec![],
            environment: EnvironmentEntry {
                id: 0,
                digest,
                value: value.clone(),
            },
        });
    }
    let environments = {
        let mut codec = FoundationCodec::new(r, &store, a).map_err(err)?;
        let checked = contexts
            .iter()
            .map(|raw| raw.check(&mut codec, &store, r, b).map_err(err))
            .collect::<Result<Vec<_>, String>>()?;
        let inputs = checked
            .iter()
            .zip(["Doc", "Math", "Circuit", "Grammar"])
            .map(|(context, alias)| EnvironmentInput { alias, context })
            .collect::<Vec<_>>();
        ParseEnvironmentSet::prepare(resolved, &inputs, &store, &mut codec, b)
            .map_err(|e| format!("environments: {e:?}"))?
    };
    let entry = resolved.entry(alias, Some(category), b).map_err(err)?;
    let states = ["Doc", "Math", "Circuit", "Grammar"].map(|alias| LanguageReaderState {
        alias: alias.into(),
        state: NdfValue::Unit,
    });
    let mut parser =
        ParseSession::new("doc-parse".into(), resolved, &environments, b).map_err(err)?;
    let mut result = parser
        .read(
            ParseRequest {
                snapshot: source,
                start: 0,
                limit: source.text().len() as u64,
                final_input: true,
                entry: &entry,
                states: &states,
            },
            &store,
            b,
            a,
        )
        .map_err(err)?;
    let mut reservations = 0;
    loop {
        match result.outcome {
            ParseOutcome::Await { call, continuation } => {
                let ProviderCall::Read {
                    operation,
                    request,
                    depth_base,
                    ..
                } = call.as_ref()
                else {
                    return Err("unexpected provider".into());
                };
                let mut declared = SourceStore::default();
                for source in &request.sources {
                    declared.insert(source.clone()).map_err(err)?;
                }
                let snapshot = declared
                    .resolve(&request.snapshot)
                    .ok_or("request source")?;
                let terminal = b
                    .with_depth_at_least(*depth_base, |b| {
                        let mut codec = FoundationCodec::new(r, &declared, a)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        let checked = request
                            .context
                            .check(&mut codec, &declared, r, b)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        (if operation.schema.package == "nepl3.doc.reader" {
                            nepl3_tools::doc::reader::read
                        } else {
                            nepl3_reader::builtin::provider::read
                        })(
                            operation,
                            ReadRequest {
                                snapshot,
                                start: request.start,
                                limit: request.limit,
                                final_input: request.final_input,
                                context: &checked,
                                state: &request.state,
                            },
                            r,
                            &declared,
                            b,
                            a,
                        )
                    })
                    .map_err(err)?;
                result = parser
                    .resume(
                        &continuation,
                        ProviderReply::Read(Box::new(terminal)),
                        &store,
                        b,
                        a,
                    )
                    .map_err(err)?;
            }
            ParseOutcome::Reserve { continuation, .. } => {
                reservations += 1;
                let reserved = SourceReservation {
                    source_id: SourceId(format!("decoded-{reservations}")),
                    revision: 0,
                    uri: format!("memory:decoded-{reservations}"),
                };
                result = parser
                    .reserve(&continuation, &reserved, &store, b, a)
                    .map_err(err)?;
            }
            ParseOutcome::Complete { tree, cursor, .. } => {
                assert_eq!(cursor, source.text().len() as u64);
                return Ok(tree);
            }
            other => return Err(format!("candidate: {other:?}")),
        }
    }
}

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 500_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
struct Compiled {
    doc: CompiledLanguage,
    others: Vec<nepl3_engine::package::LanguagePackage>,
}
fn compiled() -> Result<Compiled, String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../conformance/fixtures/doc/syntax.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut doc = nepl3_tools::doc::catalog::compile(
        &document,
        "standard.doc",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut others = Vec::new();
    for (name, seed) in [
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
        let document =
            nepl3_tools::bootstrap::load(seed, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
        let other = nepl3_tools::doc::catalog::compile(
            &document,
            &format!("standard.{name}"),
            &mut budget(),
            &mut SourceAdmission::default(),
        )?;
        let schema = other.package.schema.clone();
        let descriptor = other
            .registry
            .descriptor(&schema)
            .ok_or("surface descriptor")?
            .clone();
        doc.registry
            .register(schema, descriptor, &mut budget())
            .map_err(err)?;
        others.push(other.package);
    }
    doc.registry.finalize(&mut budget()).map_err(err)?;
    Ok(Compiled { doc, others })
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
            let value = nepl3_doc_core::portable::from_value(
                &token.payload,
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
