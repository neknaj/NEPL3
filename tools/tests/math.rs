//! Production four-language profile for Math parsing, including real Doc reader callbacks.
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
                "nepl3.math",
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
        id: "math-prefix".into(),
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
        SourceId("math-input".into()),
        0,
        "memory:math-input".into(),
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
    parse_source_as(source, resolved, "Math", category, b, a)
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
    let descriptor = nepl3_math_core::schema::descriptor(&mut budget()).map_err(err)?;
    doc.registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    doc.registry.finalize(&mut budget()).map_err(err)?;
    Ok(Compiled { doc, others })
}

#[test]
fn all_math_constructors_lower_from_the_actual_parser_and_first_receiver() -> Result<(), String> {
    use nepl3_math_core::{check::Category, model::MathRoot};
    let compiled = compiled()?;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../conformance/fixtures/math/lower.json"))
            .map_err(err)?;
    let cases = fixture["cases"].as_array().ok_or("Math cases")?;
    let formdata: serde_json::Value =
        serde_json::from_str(include_str!("../../design/forms.json")).map_err(err)?;
    let mut expected = std::collections::BTreeSet::new();
    for (name, category) in formdata["categories"].as_object().ok_or("categories")? {
        if name.starts_with("Math/") {
            for form in category["forms"].as_object().ok_or("forms")?.values() {
                expected.insert(form["kind"].as_str().ok_or("kind")?.to_string());
            }
        }
    }
    expected.extend(["Number".to_string(), "SymbolName".to_string()]);
    let mut seen = std::collections::BTreeSet::new();
    for case in cases {
        let input = case["source"].as_str().ok_or("source")?;
        let entry = case["entry"].as_str().ok_or("entry")?;
        let kind = case["kind"].as_str().ok_or("kind")?;
        seen.insert(kind.to_string());
        let category = match entry {
            "Expr" => Category::Expr,
            "Row" => Category::Row,
            "DocGuest" => Category::DocGuest,
            _ => return Err("entry fixture".into()),
        };
        with_input(&compiled, input, entry, |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let original_tree = tree.tree().clone();
            let mut operation = budget();
            let original = nepl3_math_core::lower::expression(
                &checked,
                &compiled.others[0].schema,
                category,
                profile.registry(),
                &mut operation,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            assert_eq!(tree.tree(), &original_tree);
            let root = match original.value.root {
                MathRoot::Expr(v) => v.0,
                MathRoot::Row(v) => v.0,
                MathRoot::DocGuest(v) => v.0,
            };
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let value = nepl3_math_core::portable::to_value(
                &original,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let NdfValue::Record(syntax) = &value else {
                return Err("MathSyntax record".into());
            };
            let NdfValue::Record(arena) = &syntax.fields[0] else {
                return Err("MathValue record".into());
            };
            let NdfValue::List(nodes) = &arena.fields[1] else {
                return Err("nodes list".into());
            };
            let NdfValue::Record(node) = &nodes[root as usize] else {
                return Err("node record".into());
            };
            let NdfValue::Variant(actual_kind) = &node.fields[0] else {
                return Err("node kind".into());
            };
            assert_eq!(
                actual_kind.variant,
                if kind == "SymbolName" { "Symbol" } else { kind }
            );
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let mut admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let actual = nepl3_math_core::portable::from_value(
                &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(actual.value.root, original.value.root);
            assert_eq!(actual.value.nodes, original.value.nodes);
            assert_eq!(
                nepl3_math_core::portable::to_value(
                    &actual,
                    profile.registry(),
                    &mut receiver,
                    &mut budget()
                )
                .map_err(err)?,
                value
            );
            Ok(())
        })
        .map_err(|e| format!("{kind} / {input}: {e}"))?;
    }
    assert_eq!(seen, expected);
    assert_eq!(cases.len(), 31);
    Ok(())
}

#[test]
fn math_lower_failures_keep_original_constructor_and_sticky_resource_reason() -> Result<(), String>
{
    use nepl3_math_core::{
        check::{Category, ShapeError},
        lower::LowerError,
    };
    let compiled = compiled()?;
    for (input, expected) in [
        ("matrix nil", "EmptyMatrix"),
        ("vector nil", "EmptyVector"),
        ("fence \"ab\" \"\" x", "FenceWidth"),
        ("root 0 x", "InvalidRootDegree"),
        ("matrix cons row nil nil", "MatrixWidth"),
    ] {
        with_input(&compiled, input, "Expr", |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let error = nepl3_math_core::lower::expression(
                &checked,
                &compiled.others[0].schema,
                Category::Expr,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .err()
            .ok_or("invalid Math accepted")?;
            let LowerError::Invalid { node, error } = error else {
                return Err(format!("unlocated lower failure: {error:?}"));
            };
            assert_eq!(node, tree.tree().bundle.root);
            let cover = tree.tree().bundle.nodes[node.0 as usize]
                .cover
                .as_ref()
                .ok_or("source cover")?;
            assert_eq!((cover.start(), cover.end()), (0, input.len() as u64));
            assert!(matches!(
                (expected, error),
                ("EmptyMatrix", ShapeError::EmptyMatrix(_))
                    | ("EmptyVector", ShapeError::EmptyVector(_))
                    | ("FenceWidth", ShapeError::FenceWidth(_))
                    | ("InvalidRootDegree", ShapeError::InvalidRootDegree(_))
                    | ("MatrixWidth", ShapeError::MatrixWidth { .. })
            ));
            Ok(())
        })?;
    }
    for input in [
        "let x frac 1 2 add x y",
        "label x Doc sentence cons anno text \"\" cons text \"note\" nil nil",
    ] {
        with_input(&compiled, input, "Expr", |tree, profile, b, a| {
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let before = tree.tree().clone();
            let mut full = budget();
            let baseline = nepl3_math_core::lower::expression(
                &checked,
                &compiled.others[0].schema,
                Category::Expr,
                profile.registry(),
                &mut full,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            // Doc annotation remains syntax, including its invalid empty Anno.
            assert_eq!(
                baseline.value.embeds.len(),
                usize::from(input.starts_with("label"))
            );
            let usage = full.usage();
            let mut stopped = 0;
            for (reason, used) in [
                (StopReason::SourceLimit, usage.source_bytes),
                (StopReason::WorkLimit, usage.work),
                (StopReason::NodeLimit, usage.nodes),
                (StopReason::AllocationLimit, usage.allocation_units),
                (StopReason::DepthLimit, usage.depth),
            ] {
                for cap in [0, used / 2, used] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::SourceLimit => limits.source_bytes = cap,
                        StopReason::WorkLimit => limits.work = cap,
                        StopReason::NodeLimit => limits.nodes = cap,
                        StopReason::AllocationLimit => limits.allocation_units = cap,
                        StopReason::DepthLimit => limits.depth = cap,
                        _ => {}
                    }
                    let mut budget = Budget::new(limits);
                    match nepl3_math_core::lower::expression(
                        &checked,
                        &compiled.others[0].schema,
                        Category::Expr,
                        profile.registry(),
                        &mut budget,
                        &mut SourceAdmission::default(),
                    ) {
                        Err(LowerError::Stopped(actual)) => {
                            assert_eq!(actual, reason);
                            assert_eq!(budget.poll(), Err(reason));
                            stopped += 1;
                        }
                        Ok(actual) => assert_eq!(actual, baseline),
                        Err(error) => return Err(format!("unexpected lower failure: {error:?}")),
                    }
                    assert_eq!(tree.tree(), &before);
                }
            }
            assert!(stopped >= 10);
            let mut b = budget();
            let actual = b
                .with_depth_at_least(7, |b| {
                    nepl3_math_core::lower::expression(
                        &checked,
                        &compiled.others[0].schema,
                        Category::Expr,
                        profile.registry(),
                        b,
                        &mut SourceAdmission::default(),
                    )
                })
                .map_err(err)?;
            assert_eq!(actual, baseline);
            assert_eq!(b.current_depth(), 0);
            assert_eq!(b.usage().depth, usage.depth + 7);
            Ok(())
        })?;
    }
    Ok(())
}
