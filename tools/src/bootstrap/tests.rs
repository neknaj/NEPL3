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

#[test]
fn production_static_parse_lowers_actual_token_and_list_positions() -> crate::Result<()> {
    let mut b = budget();
    let compiled = lower_fixture(&mut b)?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        0,
        "memory:input".into(),
        b"language Demo 1 Root nil".to_vec(),
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    let lowered = runtime::parse(
        &source,
        &compiled,
        runtime::executable_identity().map_err(|e| format!("{e:?}"))?,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("runtime: {e:?}"))?;
    let root = lowered.node(lowered.root).map_err(|e| format!("{e:?}"))?;
    let NodeKind::Language {
        name,
        revision,
        root: category,
        declarations,
    } = &root.kind
    else {
        return Err("Language".into());
    };
    assert_eq!(name.value, "Demo");
    assert_eq!((name.span.start(), name.span.end()), (9, 13));
    assert_eq!(revision.value, Integer::from(1u64));
    assert_eq!((revision.span.start(), revision.span.end()), (14, 15));
    assert_eq!(category.value, "Root");
    assert_eq!(
        (declarations.span.start(), declarations.span.end()),
        (21, 24)
    );
    assert!(declarations.items.is_empty());
    Ok(())
}

fn lower_fixture(
    b: &mut Budget,
) -> crate::Result<nepl3_grammar_core::compile::package::CompiledLanguage> {
    let bytes = seed("tools/tests/fixtures/grammar/lower-language.neplg")?;
    let document =
        load(&bytes, b, &mut SourceAdmission::default()).map_err(|e| format!("{e:?}"))?;
    let checked = document
        .validate(b, &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = nepl3_core::schema::SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(b),
        nepl3_reader::schema::descriptor(b),
        nepl3_engine::schema::descriptor(b),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor.reference(b).map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, b)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(b).map_err(|e| format!("{e:?}"))?;
    let compiled = nepl3_grammar_core::compile::package::compile(
        &checked,
        &nepl3_grammar_core::compile::package::PackageContext {
            package: "test.lower-grammar",
            state_type: &nepl3_core::schema::TypeDescriptor::Unit,
            reader_imports: &[],
            extensions: &[],
            classes: &[],
            views: &[],
        },
        registry,
        b,
    )
    .map_err(|e| format!("compile: {e:?}"))?;
    Ok(compiled)
}

#[test]
fn complete_grammar_bootstrap_uses_production_parser_lower_and_semantic_identity()
-> crate::Result<()> {
    complete_bootstrap(true, 10_000_000_000).map(|_| ())
}
/// Explicit comparison benchmark, excluded from routine conformance execution.
#[test]
#[ignore = "owned continuation performance baseline; run explicitly with --ignored --nocapture"]
fn complete_grammar_bootstrap_host_comparison() -> crate::Result<()> {
    let owned = complete_bootstrap(false, 20_000_000_000)?;
    let native = complete_bootstrap(true, 20_000_000_000)?;
    assert_eq!(owned, native);
    Ok(())
}
fn complete_bootstrap(
    inline_host: bool,
    work_cap: u64,
) -> crate::Result<nepl3_engine::package::PackageIdentity> {
    let bytes = seed("languages/grammar/syntax.neplg")?;
    let mut limits = budget().limits();
    // The comparison gives both routes the same explicit measurement headroom.
    // Normal native conformance retains its earlier cap; this is not a performance fix.
    limits.work = work_cap;
    limits.allocation_units = 100_000_000_000;
    limits.depth = 4096;
    let mut b = Budget::new(limits);
    let mut admission = SourceAdmission::default();
    let seed = load(&bytes, &mut b, &mut admission).map_err(|e| format!("load: {e:?}"))?;
    let source = seed.sources[0]
        .clone_with_budget(&mut b)
        .map_err(|e| format!("source: {e:?}"))?;
    let first = measure("compile P0", &mut b, &mut admission, |b, a| {
        catalog::compile(&seed, "nepl3.syntax.grammar", b, a).map_err(Into::into)
    })?;
    let implementation = runtime::executable_identity().map_err(|e| format!("identity: {e:?}"))?;
    let p1 = measured_parse(
        "P1",
        &source,
        &first,
        implementation,
        &mut b,
        &mut admission,
        inline_host,
    )?;
    let second = measure("compile P1", &mut b, &mut admission, |b, a| {
        catalog::compile(&p1, "nepl3.syntax.grammar", b, a).map_err(Into::into)
    })?;
    let p2 = measured_parse(
        "P2",
        &source,
        &second,
        implementation,
        &mut b,
        &mut admission,
        inline_host,
    )?;
    let third = measure("compile P2", &mut b, &mut admission, |b, a| {
        catalog::compile(&p2, "nepl3.syntax.grammar", b, a).map_err(Into::into)
    })?;
    let mut identities = Vec::new();
    for compiled in [&first, &second, &third] {
        identities.push(
            compiled
                .package
                .check(&compiled.registry, &mut b)
                .map_err(|e| format!("check: {e:?}"))?
                .semantic_identity(&mut b)
                .map_err(|e| format!("semantic: {e:?}"))?,
        );
    }
    assert_eq!(identities[0], identities[1]);
    assert_eq!(identities[1], identities[2]);
    assert_eq!(p1.nodes.len(), seed.nodes.len());
    assert_eq!(p2.nodes.len(), seed.nodes.len());
    eprintln!(
        "bootstrap actual usage: {:?}; nodes {}",
        b.usage(),
        p2.nodes.len()
    );
    Ok(identities.remove(0))
}

#[test]
fn lower_rechecks_execution_owner_even_when_semantic_profile_is_equal() -> crate::Result<()> {
    let mut b = budget();
    let compiled = lower_fixture(&mut b)?;
    let mut reordered = compiled.package.clone();
    assert_eq!(reordered.forms.len(), 2);
    reordered.forms.reverse();
    let a = compiled
        .package
        .check(&compiled.registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let r = reordered
        .check(&compiled.registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        a.semantic_identity(&mut b).map_err(|e| format!("{e:?}"))?,
        r.semantic_identity(&mut b).map_err(|e| format!("{e:?}"))?
    );
    assert_ne!(
        a.execution_digest(&mut b).map_err(|e| format!("{e:?}"))?,
        r.execution_digest(&mut b).map_err(|e| format!("{e:?}"))?
    );
    let source = SourceSnapshot::new(
        SourceId("lower-execution".into()),
        0,
        "memory:lower-execution".into(),
        b"language Demo 1 Root nil".to_vec(),
        &mut b,
    )
    .map_err(|e| format!("{e:?}"))?;
    runtime::with_tree(
        &source,
        &compiled,
        runtime::executable_identity().map_err(|e| format!("{e:?}"))?,
        &mut b,
        &mut SourceAdmission::default(),
        |tree, profile, budget, admission| {
            let packages = [&reordered];
            let alternate = profile
                .profile()
                .resolve(
                    &nepl3_engine::profile::RuntimeCatalog {
                        packages: &packages,
                        providers: &[],
                        resources: &[],
                    },
                    &compiled.registry,
                    budget,
                )
                .map_err(|e| runtime::RuntimeError::Boundary(format!("{e:?}")))?;
            assert_eq!(profile.digest(), alternate.digest());
            assert!(matches!(
                nepl3_grammar_core::lower::lower(tree, &alternate, budget, admission),
                Err(nepl3_grammar_core::lower::LowerError::Tree(
                    nepl3_engine::tree::TreeError::ExecutionIdentity
                ))
            ));
            // The same proof remains usable with its actual owner; rejecting the
            // alternate cannot mutate the tree or reinterpret old arena indices.
            nepl3_grammar_core::lower::lower(tree, profile, budget, admission)
                .map_err(|e| runtime::RuntimeError::Boundary(format!("{e:?}")))?;
            Ok(())
        },
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

#[test]
fn compiled_angle_reader_keeps_quoted_delimiters_and_streaming_commit() -> crate::Result<()> {
    use nepl3_core::{
        source::SourceStore,
        syntax::{Environment, EnvironmentEntry},
        value::NdfValue,
    };
    use nepl3_reader::{
        model::{ReadReply, ReadRequest, ReaderContext},
        runtime::ReaderSession,
    };
    use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let document = load(
        &seed("examples/grammar/angle-tag.neplg")?,
        &mut b,
        &mut admission,
    )
    .map_err(|e| format!("{e:?}"))?;
    let compiled = catalog::compile(&document, "test.angle", &mut b, &mut admission)?;
    let registry = &compiled.registry;
    let plan = compiled
        .package
        .reader
        .check(registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let raw = ReaderContext {
        schema: compiled.package.schema.clone(),
        category: "Tag".into(),
        mode: "Tags".into(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest: environment_digest(&environment, foundation, registry, &mut b)
                .map_err(|e| format!("{e:?}"))?,
            value: environment,
        },
    };
    // The expected end is the unquoted closing delimiter, even though an earlier
    // '>' occurs inside the string. Trailing host input must remain unread.
    for (index, text) in [r#"<a title="x>y"> tail"#, "<a title='x>y'> tail"]
        .into_iter()
        .enumerate()
    {
        let source = SourceSnapshot::new(
            SourceId(format!("angle-{index}")),
            0,
            "memory:angle".into(),
            text.as_bytes().to_vec(),
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut sources = SourceStore::default();
        sources
            .insert(
                source
                    .clone_with_budget(&mut b)
                    .map_err(|e| format!("{e:?}"))?,
            )
            .map_err(|e| format!("{e:?}"))?;
        let context = {
            let mut codec = FoundationCodec::new(registry, &sources, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            raw.check(&mut codec, &sources, registry, &mut b)
                .map_err(|e| format!("{e:?}"))?
        };
        let mut session =
            ReaderSession::new(format!("angle-reader-{index}"), &plan, registry, &mut b)
                .map_err(|e| format!("{e:?}"))?;
        for final_input in [false, true] {
            let reply = session
                .read(
                    "tag",
                    ReadRequest {
                        snapshot: &source,
                        start: 0,
                        limit: text.len() as u64,
                        final_input,
                        context: &context,
                        state: &NdfValue::Unit,
                    },
                    &sources,
                    &mut b,
                    &mut admission,
                )
                .map_err(|e| format!("{e:?}"))?;
            assert!(matches!(reply, ReadReply::Matched { end: 15, .. }));
        }
        for limit in 0..15 {
            let reply = session
                .read(
                    "tag",
                    ReadRequest {
                        snapshot: &source,
                        start: 0,
                        limit,
                        final_input: false,
                        context: &context,
                        state: &NdfValue::Unit,
                    },
                    &sources,
                    &mut b,
                    &mut admission,
                )
                .map_err(|e| format!("{e:?}"))?;
            assert!(
                matches!(reply, ReadReply::NeedMore { .. }),
                "cut {limit}: {reply:?}"
            );
        }
        let reply = session
            .read(
                "tag",
                ReadRequest {
                    snapshot: &source,
                    start: 0,
                    limit: 12,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
                &sources,
                &mut b,
                &mut admission,
            )
            .map_err(|e| format!("{e:?}"))?;
        assert!(matches!(reply, ReadReply::Failed { .. }));
    }
    Ok(())
}

#[test]
fn original_grammar_parser_compiles_independent_reader_binding_style_mutations() -> crate::Result<()>
{
    let mut limits = budget().limits();
    limits.work = 10_000_000_000;
    limits.allocation_units = 100_000_000_000;
    limits.depth = 4096;
    let mut b = Budget::new(limits);
    let mut admission = SourceAdmission::default();
    let seed = load(
        &seed("languages/grammar/syntax.neplg")?,
        &mut b,
        &mut admission,
    )
    .map_err(|e| format!("{e:?}"))?;
    let parser = catalog::compile(&seed, "nepl3.syntax.grammar", &mut b, &mut admission)?;
    let source = r#"language Mutation 1 Expr
cons reader word some scalar asciiLetter
cons reader trivia some scalar whitespace
cons mode Code cons skip trivia cons take Word word nil
cons category Expr Code
cons namespace Names lexical
cons form Atom Expr "atom"
cons field name builtin Name nil
bind Names name
cons style head "content" nil
nil"#;
    let implementation = runtime::executable_identity().map_err(|e| format!("{e:?}"))?;
    let variants = [
        source.to_string(),
        source.replace("some scalar asciiLetter", "some scalar identifierStart"),
        source.replace("bind Names name", "reference Names name"),
        source.replace("style head \"content\"", "style head \"marker\""),
    ];
    let mut identities = Vec::new();
    for (index, text) in variants.iter().enumerate() {
        if index != 0 {
            assert_ne!(text, source);
        }
        let snapshot = SourceSnapshot::new(
            SourceId(format!("mutation-{index}")),
            0,
            format!("memory:mutation/{index}"),
            text.as_bytes().to_vec(),
            &mut b,
        )
        .map_err(|e| format!("{e:?}"))?;
        // Every variant is read by the unchanged original Grammar parser; no
        // re-seeding or Rust AST mutation can bypass source parsing/lowering.
        let document = runtime::parse(&snapshot, &parser, implementation, &mut b, &mut admission)
            .map_err(|e| format!("variant {index}: {e:?}"))?;
        let compiled = catalog::compile(&document, "test.mutation", &mut b, &mut admission)?;
        identities.push(
            compiled
                .package
                .check(&compiled.registry, &mut b)
                .map_err(|e| format!("{e:?}"))?
                .semantic_identity(&mut b)
                .map_err(|e| format!("{e:?}"))?,
        );
    }
    // Each one-item source change affects a distinct normative part of meaning.
    for (index, identity) in identities.iter().enumerate().skip(1) {
        assert_ne!(&identities[0], identity, "mutation {index}");
    }
    Ok(())
}

#[test]
fn file_driver_distinguishes_trailing_trivia_stop_from_extra_input() -> crate::Result<()> {
    let mut setup = budget();
    let compiled = lower_fixture(&mut setup)?;
    let text = format!("language Demo 1 Root nil #{}", "x".repeat(8192));
    let source = SourceSnapshot::new(
        SourceId("trailing-budget".into()),
        0,
        "memory:trailing-budget".into(),
        text.as_bytes().to_vec(),
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    let implementation = runtime::executable_identity().map_err(|e| format!("{e:?}"))?;
    let mut b = budget();
    runtime::with_tree(
        &source,
        &compiled,
        implementation,
        &mut b,
        &mut SourceAdmission::default(),
        |_, _, _, _| Ok(()),
    )
    .map_err(|e| format!("{e:?}"))?;
    let total = b.usage().work;
    let mut stopped_after_parse = 0;
    for cap in (total.saturating_sub(32768)..total).step_by(512) {
        let mut limits = budget().limits();
        limits.work = cap;
        let mut b = Budget::new(limits);
        match runtime::with_tree(
            &source,
            &compiled,
            implementation,
            &mut b,
            &mut SourceAdmission::default(),
            |_, _, _, _| Ok(()),
        ) {
            Err(runtime::RuntimeError::Stopped { reason, reply }) => {
                assert_eq!(reason, StopReason::WorkLimit);
                assert_eq!(reply.report.usage, b.usage());
                if matches!(
                    reply.outcome,
                    nepl3_engine::parse::ParseOutcome::Complete { .. }
                ) {
                    stopped_after_parse += 1;
                }
            }
            Err(runtime::RuntimeError::TrailingInput(_)) => {
                return Err("valid trailing comment misreported as extra input".into());
            }
            Ok(())
            | Err(runtime::RuntimeError::PreparationStopped {
                reason: StopReason::WorkLimit,
                ..
            }) => {}
            Err(error) => return Err(format!("unexpected sweep result: {error:?}").into()),
        }
    }
    assert!(
        stopped_after_parse > 0,
        "exercise stops after a formal complete parse, before file completion"
    );
    let mut limits = budget().limits();
    limits.work = 0;
    assert!(matches!(
        runtime::parse(
            &source,
            &compiled,
            implementation,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        ),
        Err(runtime::RuntimeError::PreparationStopped {
            reason: StopReason::WorkLimit,
            ..
        })
    ));
    let bad = SourceSnapshot::new(
        SourceId("extra".into()),
        0,
        "memory:extra".into(),
        b"language Demo 1 Root nil extra".to_vec(),
        &mut setup,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        runtime::parse(
            &bad,
            &compiled,
            implementation,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(runtime::RuntimeError::TrailingInput(25))
    ));
    Ok(())
}

fn print_usage(
    stage: &str,
    before: nepl3_core::budget::Usage,
    after: nepl3_core::budget::Usage,
    elapsed: std::time::Duration,
) {
    eprintln!(
        "{stage}: {:?}; work {}; allocation {}; source {}; nodes {}; output {}; depth peak {}",
        elapsed,
        after.work - before.work,
        after.allocation_units - before.allocation_units,
        after.source_bytes - before.source_bytes,
        after.nodes - before.nodes,
        after.output_bytes - before.output_bytes,
        after.depth
    );
}
fn measure<T>(
    stage: &str,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
    run: impl FnOnce(&mut Budget, &mut SourceAdmission) -> crate::Result<T>,
) -> crate::Result<T> {
    let before = budget.usage();
    let started = std::time::Instant::now();
    let result = run(budget, admission)?;
    print_usage(stage, before, budget.usage(), started.elapsed());
    Ok(result)
}
fn measured_parse(
    stage: &str,
    source: &SourceSnapshot,
    compiled: &nepl3_grammar_core::compile::package::CompiledLanguage,
    implementation: nepl3_core::source::Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
    inline_host: bool,
) -> crate::Result<Document> {
    let before = budget.usage();
    let started = std::time::Instant::now();
    let mut metrics = runtime::metrics::Metrics {
        inline_host,
        ..runtime::metrics::Metrics::default()
    };
    let result = runtime::with_tree_measured(
        source,
        compiled,
        implementation,
        budget,
        admission,
        &mut metrics,
        |tree, profile, budget, admission| {
            print_usage(
                &format!("parse {stage} (includes host boundary/validation)"),
                before,
                budget.usage(),
                started.elapsed(),
            );
            measure(&format!("lower {stage}"), budget, admission, |b, a| {
                nepl3_grammar_core::lower::lower(tree, profile, b, a)
                    .map_err(|e| format!("{e:?}").into())
            })
            .map_err(|e| runtime::RuntimeError::Boundary(e.to_string()))
        },
    )
    .map_err(|e| format!("{stage}: {e:?}").into());
    eprintln!("{stage} driver metrics: {metrics:#?}");
    result
}
