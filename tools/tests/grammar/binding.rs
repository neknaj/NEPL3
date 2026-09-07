//! Expectations follow binding.neplg: Let.init precedes its scope; Apply siblings share their original stage.
use super::*;
use nepl3_core::{
    facts::*,
    syntax::{Environment, EnvironmentEntry},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
use nepl3_engine::{binding::*, parse::*, profile::*, tree::ValidatedParseTree};
use nepl3_reader::{
    model::{ProviderCall, ReadRequest, ReaderContext},
    runtime::ProviderReply,
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
fn with_input<T>(
    compiled: &CompiledLanguage,
    input: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_completed_input(compiled, input, |parsed, profile, b, a| {
        let checked = parsed.tree().validate(profile, b, a).map_err(err)?;
        finish(&checked, profile, b, a)
    })
}
pub(super) fn with_completed_input<T>(
    compiled: &CompiledLanguage,
    input: &str,
    finish: impl FnOnce(
        &CompletedParse,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_completed_input_extra(compiled, None, input, finish)
}
fn with_completed_input_extra<T>(
    compiled: &CompiledLanguage,
    extra: Option<&nepl3_engine::package::LanguagePackage>,
    input: &str,
    finish: impl FnOnce(
        &CompletedParse,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    let p = &compiled.package;
    let r = &compiled.registry;
    let identity = p
        .check(r, &mut budget())
        .and_then(|v| v.semantic_identity(&mut budget()))
        .map_err(err)?;
    // This fixture registers the exact source of its host adapter implementation.
    let implementation = Digest::of(include_bytes!("binding.rs"));
    let implementation_for = |operation: &nepl3_core::value::OperationRef| {
        if operation.name == "bindingFacts" {
            custom::implementation_digest()
        } else {
            implementation
        }
    };
    let mut operations = Vec::new();
    for operation in p
        .reader
        .providers
        .iter()
        .map(|v| &v.operation)
        .chain(p.extensions.iter().map(|v| &v.operation))
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
    let mut schemas = vec![p.schema.clone()];
    for schema in p.payload_schemas.iter().chain(
        ["nepl3.foundation", "nepl3.reader", "nepl3.engine"]
            .iter()
            .filter_map(|name| r.selected(name, 1)),
    ) {
        if !schemas.contains(schema) {
            schemas.push(schema.clone());
        }
    }
    let mut languages = vec![LanguageRegistration {
        alias: "B".into(),
        package: identity,
        default_category: "Expr".into(),
    }];
    let mut packages = vec![p];
    if let Some(extra) = extra {
        schemas.push(extra.schema.clone());
        languages.push(LanguageRegistration {
            alias: "Other".into(),
            package: extra
                .check(r, &mut budget())
                .and_then(|v| v.semantic_identity(&mut budget()))
                .map_err(err)?,
            default_category: "Expr".into(),
        });
        packages.push(extra);
    }
    let profile = ParseProfile {
        id: "binding-review".into(),
        languages,
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
        SourceId("binding-input".into()),
        0,
        "memory:binding-input".into(),
        input.as_bytes().to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let parsed = parse_completed(&source, &resolved, &mut b, &mut a)?;
    finish(&parsed, &resolved, &mut b, &mut a)
}

fn parse_completed(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<CompletedParse, String> {
    parse_completed_with_aux(source, &[], &[], resolved, b, a)
}
pub(super) fn parse_completed_with_aux(
    source: &SourceSnapshot,
    extras: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<CompletedParse, String> {
    match parse_any_with_aux(source, extras, maps, resolved, b, a)? {
        ParseCompletion::Continue(parsed) => Ok(parsed),
        ParseCompletion::Break(reply) => Err(format!("candidate: {:?}", reply.outcome)),
    }
}
pub(super) fn parse_any_with_aux(
    source: &SourceSnapshot,
    extras: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ParseCompletion, String> {
    parse_with_artifacts(source, extras, maps, &[], resolved, b, a)
}
pub(super) fn parse_with_artifacts(
    source: &SourceSnapshot,
    extras: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    facts: &[nepl3_reader::model::ReaderFact],
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ParseCompletion, String> {
    parse_with_artifacts_at(source, extras, maps, (0, facts), resolved, b, a)
}
pub(super) fn parse_completed_with_aux_at(
    source: &SourceSnapshot,
    extras: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    start: u64,
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<CompletedParse, String> {
    match parse_with_artifacts_at(source, extras, maps, (start, &[]), resolved, b, a)? {
        ParseCompletion::Continue(parsed) => Ok(parsed),
        ParseCompletion::Break(reply) => Err(err(reply)),
    }
}
fn parse_with_artifacts_at(
    source: &SourceSnapshot,
    extras: &[SourceSnapshot],
    maps: &[nepl3_core::origin::Mapping],
    injection: (u64, &[nepl3_reader::model::ReaderFact]),
    resolved: &ResolvedParseProfile<'_>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ParseCompletion, String> {
    let (injection_start, facts) = injection;
    let r = resolved.registry();
    let p = resolved.language("B", b).map_err(err)?;
    let foundation = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let mut store = SourceStore::default();
    store.insert(source.clone()).map_err(err)?;
    for source in extras {
        store.insert(source.clone()).map_err(err)?;
    }
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&value, foundation, r, b).map_err(err)?;
    let raw = ReaderContext {
        schema: p.schema.clone(),
        category: "Expr".into(),
        mode: "Code".into(),
        origins: extras
            .iter()
            .map(|v| {
                v.span(0, v.text().len() as u64)
                    .map(nepl3_core::origin::Origin::Direct)
            })
            .collect::<Result<_, _>>()
            .map_err(err)?,
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value,
        },
    };
    let mut raws = vec![raw];
    for language in &resolved.profile().languages {
        if language.alias == "B" {
            continue;
        }
        let p = resolved.language(&language.alias, b).map_err(err)?;
        let mut raw = raws[0].clone();
        raw.schema = p.schema.clone();
        raw.category = language.default_category.clone();
        raws.push(raw);
    }
    let environments = {
        let mut codec = FoundationCodec::new(r, &store, a).map_err(err)?;
        let proofs = raws
            .iter()
            .map(|raw| raw.check(&mut codec, &store, r, b).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let inputs = resolved
            .profile()
            .languages
            .iter()
            .zip(&proofs)
            .map(|(language, proof)| EnvironmentInput {
                alias: &language.alias,
                context: proof,
            })
            .collect::<Vec<_>>();
        ParseEnvironmentSet::prepare(resolved, &inputs, &store, &mut codec, b).map_err(err)?
    };
    let entry = resolved.entry("B", None, b).map_err(err)?;
    let states = resolved
        .profile()
        .languages
        .iter()
        .map(|v| LanguageReaderState {
            alias: v.alias.clone(),
            state: NdfValue::Unit,
        })
        .collect::<Vec<_>>();
    let mut parser =
        ParseSession::new("binding-parse".into(), resolved, &environments, b).map_err(err)?;
    let mut result = parser
        .read_completed(
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
    let mut additional = false;
    loop {
        let progress = match result {
            ParseCompletion::Continue(parsed) => {
                assert_eq!(parsed.cursor(), source.text().len() as u64);
                return Ok(ParseCompletion::Continue(parsed));
            }
            ParseCompletion::Break(reply) => reply,
        };
        match progress.outcome {
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
                let mut terminal = b
                    .with_depth_at_least(*depth_base, |b| {
                        let mut codec = FoundationCodec::new(r, &declared, a)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        let checked = request
                            .context
                            .check(&mut codec, &declared, r, b)
                            .map_err(|_| nepl3_reader::runtime::ReaderError::Context)?;
                        nepl3_reader::builtin::provider::read(
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
                if !additional
                    && request.start >= injection_start
                    && let nepl3_reader::model::ReadReply::Matched {
                        sources,
                        source_maps,
                        facts: returned_facts,
                        ..
                    } = &mut terminal
                {
                    for source in extras {
                        if !sources.iter().any(|v| v.identity() == source.identity()) {
                            sources.push(source.clone());
                        }
                    }
                    source_maps.extend_from_slice(maps);
                    returned_facts.extend_from_slice(facts);
                    additional = true;
                }
                result = parser
                    .resume_completed(
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
                    .reserve_completed(&continuation, &reserved, &store, b, a)
                    .map_err(err)?;
            }
            other => {
                return Ok(ParseCompletion::Break(ParseReply {
                    outcome: other,
                    ..progress
                }));
            }
        }
    }
}

#[test]
fn official_binding_let_lambda_visibility_and_entity_identity() -> Result<(), String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/binding/official.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    for (input, expected) in [
        ("let x x x", vec![None, Some(4)]),
        (
            "lambda x apply let x x x x",
            vec![Some(7), Some(19), Some(7)],
        ),
        ("lambda x lambda x x", vec![Some(16)]),
        (
            "lambda \u{3042} apply let \u{3042} \u{3042} \u{3042} \u{3042}",
            vec![Some(7), Some(21), Some(7)],
        ),
    ] {
        with_input(&compiled, input, |tree, profile, b, a| {
            let reply = analyze("binding-test", tree, profile, b, a);
            let BindingOutcome::Complete(analysis) = reply.outcome else {
                return Err(format!("{input}: {:?}", reply.outcome));
            };
            let facts = analysis.facts();
            let references: Vec<_> = facts
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
                .collect();
            assert_eq!(references.len(), expected.len());
            for (reference, selection_start) in references.iter().zip(expected) {
                match (&reference.resolution, selection_start) {
                    (ReferenceResolution::Unresolved(name), None) => {
                        assert_eq!(name, &reference.name)
                    }
                    (ReferenceResolution::Resolved(id), Some(start)) => {
                        let entity = facts.entities.get(id.0 as usize).ok_or("entity")?;
                        assert_eq!(entity.selection.as_ref().ok_or("selection")?.start(), start);
                    }
                    actual => return Err(format!("{input}: resolution {actual:?}")),
                }
            }
            assert_eq!(
                analysis.result().occurrence_stages.len(),
                facts.occurrences.len()
            );
            facts.validate(&compiled.registry, b, a).map_err(err)?;
            let mut store = SourceStore::default();
            for source in analysis.result().sources.iter() {
                store.insert(source.clone()).map_err(err)?;
            }
            let before = b.usage().source_bytes;
            let mut codec = FoundationCodec::new(&compiled.registry, &store, a).map_err(err)?;
            let value = codec.encode_fact_set(facts, b).map_err(err)?;
            let wire = nepl3_wire::encode(&value, b).map_err(err)?;
            let value = nepl3_wire::decode(&wire, b).map_err(err)?;
            let decoded = codec.decode_fact_set(&value, b).map_err(err)?;
            assert_eq!(&decoded, facts);
            assert_eq!(b.usage().source_bytes, before);
            Ok(())
        })?;
    }
    Ok(())
}

fn execution() -> Result<CompiledLanguage, String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/binding/execution.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding-execution",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
#[test]
fn ordered_exports_recursive_headers_and_foreign_roots() -> Result<(), String> {
    let compiled = execution()?;
    // Expected declaration byte positions were fixed from spec04, before executing
    // analyze. Numeric Entity IDs and implementation traversal order are not goldens.
    type ExpectedReferences = [(u64, &'static [u64])];
    let cases: &[(&str, &ExpectedReferences)] = &[
        ("let x x x", &[(6, &[]), (8, &[4])]),
        (
            "lambda x apply let x x x x",
            &[(21, &[7]), (23, &[19]), (25, &[7])],
        ),
        ("lambda x lambda x x", &[(18, &[16])]),
        (
            "lambda \u{3042} apply let \u{3042} \u{3042} \u{3042} \u{3042}",
            &[(25, &[7]), (29, &[21]), (33, &[7])],
        ),
        (
            "sequence cons define a b cons define b a nil a",
            &[(23, &[]), (39, &[21]), (45, &[21])],
        ),
        (
            "recursive cons define a b cons define b a nil a",
            &[(24, &[38]), (40, &[22]), (46, &[22])],
        ),
        ("sequence cons define a a nil a", &[(23, &[]), (29, &[21])]),
        (
            "recursive cons define a a nil a",
            &[(24, &[22]), (30, &[22])],
        ),
        (
            "sequence cons define a 1 cons define a 2 nil a",
            &[(45, &[37])],
        ),
        (
            "recursive cons define a 1 cons define a 2 nil a",
            &[(46, &[22, 38])],
        ),
        ("sequence cons wrapped define a 1 nil a", &[(37, &[29])]),
        ("apply unimported define a 1 a", &[(28, &[])]),
        ("imported define a a a", &[(18, &[]), (20, &[16])]),
        ("lambda x apply guest x x", &[(21, &[]), (23, &[7])]),
        ("apply guest lambda x x x", &[(21, &[19]), (23, &[])]),
        ("guest guest lambda x x", &[(21, &[19])]),
        ("lambda x guest lambda x x", &[(24, &[22])]),
        ("early x x x", &[(6, &[]), (10, &[8])]),
        ("twice x x x", &[(10, &[6, 8])]),
        ("twice x y x", &[(10, &[6])]),
    ];
    for (input, expected) in cases {
        with_input(&compiled, input, |tree, profile, b, a| {
            let reply = analyze("binding-execution", tree, profile, b, a);
            let BindingOutcome::Complete(analysis) = reply.outcome else {
                return Err(format!("{input}: {:?}", reply.outcome));
            };
            let facts = analysis.facts();
            let references: Vec<_> = facts
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
                .collect();
            assert_eq!(references.len(), expected.len(), "{input}");
            for (start, candidates) in *expected {
                let reference = references
                    .iter()
                    .find(|o| o.span.start() == *start)
                    .ok_or("reference position")?;
                let ids = match &reference.resolution {
                    ReferenceResolution::Resolved(id) => vec![*id],
                    ReferenceResolution::Ambiguous(ids) => ids.clone(),
                    ReferenceResolution::Unresolved(_) => vec![],
                    ReferenceResolution::Deferred(_) => return Err("unexpected deferred".into()),
                };
                let mut actual = Vec::new();
                for id in ids {
                    actual.push(
                        facts
                            .entities
                            .get(id.0 as usize)
                            .ok_or("entity")?
                            .selection
                            .as_ref()
                            .ok_or("selection")?
                            .start(),
                    );
                }
                actual.sort_unstable();
                assert_eq!(actual, *candidates, "{input} at {start}");
            }
            // Header and body phases must retain the same Entity, not duplicate exports.
            for (i, entity) in facts.entities.iter().enumerate() {
                assert!(
                    !facts.entities[..i]
                        .iter()
                        .any(|old| old.selection == entity.selection),
                    "{input}: duplicate declaration"
                );
            }
            facts.validate(&compiled.registry, b, a).map_err(err)?;
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn binding_seed_artifacts_match_original_source_and_host_adapter() -> Result<(), String> {
    for (path, bytes) in [
        (
            "conformance/fixtures/grammar/binding/recursive.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/recursive.json")
                .as_slice(),
        ),
        (
            "conformance/fixtures/grammar/binding/custom.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/custom.json").as_slice(),
        ),
        (
            "conformance/fixtures/grammar/binding/global.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/global.json").as_slice(),
        ),
        (
            "conformance/fixtures/grammar/binding/foreign-body.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/foreign-body.json")
                .as_slice(),
        ),
        (
            "examples/grammar/binding.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/official.json")
                .as_slice(),
        ),
        (
            "conformance/fixtures/grammar/binding/execution.neplg",
            include_bytes!("../../../conformance/fixtures/grammar/binding/execution.json")
                .as_slice(),
        ),
    ] {
        let imported =
            nepl3_tools::bootstrap::load(bytes, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
        assert_eq!(imported, load(path)?);
    }
    Ok(())
}
#[test]
fn binding_stops_preserve_reason_partial_closure_and_caller_depth() -> Result<(), String> {
    let compiled = execution()?;
    for input in [
        r#"lettext "\u{78}" x"#,
        "lambda x apply guest x x",
        "let x absent missing",
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut baseline = budget();
            let complete = analyze(
                "binding-stops",
                tree,
                profile,
                &mut baseline,
                &mut SourceAdmission::default(),
            );
            assert!(matches!(complete.outcome, BindingOutcome::Complete(_)));
            let usage = baseline.usage();
            for resource in 0..6 {
                let total = match resource {
                    0 => usage.work,
                    1 => usage.allocation_units,
                    2 => usage.nodes,
                    3 => usage.depth,
                    4 => usage.source_bytes,
                    _ => usage.diagnostics,
                };
                let step = (total / 64).max(1);
                for cap in (0..=total + step).step_by(step as usize) {
                    let mut limits = budget().limits();
                    match resource {
                        0 => limits.work = cap,
                        1 => limits.allocation_units = cap,
                        2 => limits.nodes = cap,
                        3 => limits.depth = cap,
                        4 => limits.source_bytes = cap,
                        _ => limits.diagnostics = cap,
                    };
                    let mut b = Budget::new(limits);
                    let reply = analyze(
                        "binding-stops",
                        tree,
                        profile,
                        &mut b,
                        &mut SourceAdmission::default(),
                    );
                    match &reply.outcome {
                        BindingOutcome::Complete(_) => assert!(b.poll().is_ok()),
                        BindingOutcome::Stopped { reason, progress } => {
                            assert_eq!(b.poll(), Err(*reason));
                            if let Some(facts) = &progress.facts {
                                assert_eq!(
                                    facts.occurrences.len(),
                                    progress.occurrence_stages.len()
                                );
                            }
                        }
                        other => {
                            return Err(format!(
                                "{input} resource {resource} cap {cap}: {other:?}"
                            ));
                        }
                    }
                    assert_eq!(b.current_depth(), 0);
                    assert_eq!(reply.report.usage, b.usage());
                    if let Some(facts) = reply.facts() {
                        facts
                            .validate(
                                &compiled.registry,
                                &mut budget(),
                                &mut SourceAdmission::default(),
                            )
                            .map_err(err)?;
                    }
                    reply
                        .report
                        .validate(
                            &SourceStore::default(),
                            reply.sources(),
                            &compiled.registry,
                            &mut budget(),
                        )
                        .map_err(err)?;
                }
            }
            let mut depth_budget = budget();
            depth_budget
                .with_depth_at_least(7, |b| -> Result<(), BindingError> {
                    let reply = analyze(
                        "binding-stops",
                        tree,
                        profile,
                        b,
                        &mut SourceAdmission::default(),
                    );
                    assert!(matches!(reply.outcome, BindingOutcome::Complete(_)));
                    assert_eq!(b.current_depth(), 7);
                    Ok(())
                })
                .map_err(err)?;
            assert_eq!(depth_budget.current_depth(), 0);
            assert_eq!(depth_budget.usage().depth, usage.depth + 7);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn declaration_body_foreign_uses_independent_root_for_both_orders() -> Result<(), String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/binding/foreign-body.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding-foreign-body",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    for input in [
        "sequence cons define x 1 nil x",
        "recursive cons define x 1 nil x",
        "lambda x recursive cons define y x nil guest x",
    ] {
        with_input(&compiled, input, |tree, profile, b, a| {
            let reply = analyze("foreign-body", tree, profile, b, a);
            let BindingOutcome::Complete(analysis) = reply.outcome else {
                return Err(format!("{reply:?}"));
            };
            let facts = analysis.facts();
            let last = facts
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
                .max_by_key(|o| o.span.start())
                .ok_or("guest ref")?;
            assert!(matches!(
                last.resolution,
                ReferenceResolution::Unresolved(_)
            ));
            let namespace = &facts.namespaces[last.namespace.0 as usize];
            assert_ne!(
                namespace.root,
                facts.namespaces[facts.entities[0].namespace.0 as usize].root
            );
            assert!(facts.scopes.iter().filter(|s| s.parent.is_none()).count() >= 2);
            facts.validate(&compiled.registry, b, a).map_err(err)?;
            Ok(())
        })?;
    }
    Ok(())
}

#[path = "binding/portable.rs"]
mod portable;

#[test]
fn repeated_recursive_plan_reuses_only_its_own_header_entities() -> Result<(), String> {
    let compiled = execution()?;
    with_input(
        &compiled,
        "repeat cons define x x nil x",
        |tree, profile, b, a| {
            let reply = analyze("repeated-recursive", tree, profile, b, a);
            let BindingOutcome::Complete(analysis) = reply.outcome else {
                return Err(format!("{reply:?}"));
            };
            let facts = analysis.facts();
            assert_eq!(facts.entities.len(), 2);
            assert_eq!(facts.entities[0].selection, facts.entities[1].selection);
            assert_ne!(facts.entities[0].id, facts.entities[1].id);
            let refs: Vec<_> = facts
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
                .collect();
            assert_eq!(refs.len(), 4);
            assert_eq!(refs[0].resolution, refs[1].resolution);
            assert_eq!(refs[2].resolution, refs[3].resolution);
            assert_ne!(refs[0].resolution, refs[2].resolution);
            facts.validate(&compiled.registry, b, a).map_err(err)?;
            Ok(())
        },
    )
}

#[test]
fn analysis_revalidates_concrete_package_and_keeps_initial_failures_typed() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "let x x x", |tree, profile, b, a| {
        let mut other = compiled.package.clone();
        other.forms.swap(0, 1);
        let before = compiled
            .package
            .check(&compiled.registry, b)
            .map_err(err)?
            .semantic_identity(b)
            .map_err(err)?;
        let after = other
            .check(&compiled.registry, b)
            .map_err(err)?
            .semantic_identity(b)
            .map_err(err)?;
        assert_eq!(before, after);
        let providers: Vec<_> = profile
            .profile()
            .providers
            .iter()
            .map(|r| ProviderImplementation {
                provider: r.provider.clone(),
                revision: r.revision,
                implementation_digest: r.implementation_digest,
                operations: vec![r.operation.clone()],
            })
            .collect();
        let packages = [&other];
        let different = profile
            .profile()
            .resolve(
                &RuntimeCatalog {
                    packages: &packages,
                    providers: &providers,
                    resources: &[],
                },
                &compiled.registry,
                b,
            )
            .map_err(err)?;
        assert_eq!(profile.digest(), different.digest());
        let rejected = analyze("wrong-execution", tree, &different, b, a);
        assert!(matches!(
            rejected.outcome,
            BindingOutcome::Invalid {
                error: BindingError::Tree(nepl3_engine::tree::TreeError::ExecutionIdentity),
                ..
            }
        ));
        assert!(rejected.facts().is_none());
        let empty = analyze("", tree, profile, b, a);
        assert!(matches!(
            empty.outcome,
            BindingOutcome::Invalid {
                error: BindingError::AnalysisId,
                ..
            }
        ));
        let mut cancelled = budget();
        cancelled.cancel();
        let reply = analyze(
            "cancelled",
            tree,
            profile,
            &mut cancelled,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            reply.outcome,
            BindingOutcome::Stopped {
                reason: StopReason::Cancelled,
                ..
            }
        ));
        assert_eq!(reply.report.usage, cancelled.usage());
        assert!(reply.facts().is_none());
        Ok(())
    })
}

#[test]
fn open_inputs_and_global_occurrences_keep_their_distinct_scopes() -> Result<(), String> {
    let mut compiled = execution()?;
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Open;
    with_input(&compiled, "free", |tree, profile, b, a| {
        let reply = analyze("open", tree, profile, b, a);
        assert!(reply.report.diagnostics.is_empty());
        let BindingOutcome::Complete(analysis) = reply.outcome else {
            return Err(format!("{reply:?}"));
        };
        assert!(analysis.facts().entities.is_empty());
        let [input] = analysis.result().open_inputs.as_slice() else {
            return Err("expected one open input".into());
        };
        let occurrence = &analysis.facts().occurrences[input.0 as usize];
        assert_eq!(occurrence.name, "free");
        assert_eq!(occurrence.span.start(), 0);
        assert_eq!(occurrence.span.end(), 4);
        assert!(matches!(
            occurrence.resolution,
            ReferenceResolution::Unresolved(_)
        ));
        Ok(())
    })?;
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Global;
    with_input(&compiled, "lambda x x", |tree, profile, b, a| {
        let reply = analyze("global-pending", tree, profile, b, a);
        let BindingOutcome::Complete(analysis) = &reply.outcome else {
            return Err(format!("{reply:?}"));
        };
        let facts = analysis.facts();
        let definition = facts
            .occurrences
            .iter()
            .find(|o| o.role == OccurrenceRole::Definition)
            .ok_or("definition")?;
        let root = facts.namespaces[definition.namespace.0 as usize].root;
        assert_ne!(definition.scope, root);
        assert_eq!(facts.entities[0].scope, root);
        let location = analysis
            .result()
            .occurrence_stages
            .iter()
            .find(|v| v.occurrence == definition.id)
            .ok_or("stage")?;
        assert_eq!(
            analysis.result().stages[location.stage.0 as usize].scope,
            definition.scope
        );
        assert_eq!(
            analysis.result().stages[location.namespace_stage.0 as usize].scope,
            root
        );
        Ok(())
    })
}

#[test]
fn stopped_origin_preparation_never_publishes_a_forward_reference_prefix() -> Result<(), String> {
    use nepl3_core::origin::{Origin, OriginId};
    let compiled = execution()?;
    with_input(&compiled, "let x x x", |tree, profile, b, a| {
        let mut forward = tree.tree().clone();
        let next = forward.bundle.origins.len() as u64;
        forward
            .bundle
            .origins
            .push(Origin::Composite(vec![OriginId(next + 1)]));
        forward
            .bundle
            .origins
            .push(Origin::Direct(forward.bundle.tokens[0].head.clone()));
        let checked = forward.validate(profile, b, a).map_err(err)?;
        let mut baseline = budget();
        let reply = analyze(
            "forward",
            &checked,
            profile,
            &mut baseline,
            &mut SourceAdmission::default(),
        );
        assert!(matches!(reply.outcome, BindingOutcome::Complete(_)));
        let mut empty_graph_stop = false;
        let mut complete_graph_stop = false;
        for resource in 0..2 {
            let total = if resource == 0 {
                baseline.usage().work
            } else {
                baseline.usage().allocation_units
            };
            for cap in (0..=total).step_by((total / 256).max(1) as usize) {
                let mut limits = budget().limits();
                if resource == 0 {
                    limits.work = cap
                } else {
                    limits.allocation_units = cap
                };
                let mut trial = Budget::new(limits);
                let reply = analyze(
                    "forward",
                    &checked,
                    profile,
                    &mut trial,
                    &mut SourceAdmission::default(),
                );
                if let Some(facts) = reply.facts() {
                    assert!(
                        facts.origins.is_empty()
                            || facts.origins.len() == forward.bundle.origins.len()
                    );
                    facts
                        .validate(
                            &compiled.registry,
                            &mut budget(),
                            &mut SourceAdmission::default(),
                        )
                        .map_err(err)?;
                    if matches!(reply.outcome, BindingOutcome::Stopped { .. }) {
                        if facts.origins.is_empty() {
                            empty_graph_stop = true
                        } else {
                            complete_graph_stop = true
                        }
                    }
                }
            }
        }
        assert!(empty_graph_stop && complete_graph_stop);
        Ok(())
    })
}

#[path = "binding/global.rs"]
mod global;

#[path = "binding/custom.rs"]
mod custom;
