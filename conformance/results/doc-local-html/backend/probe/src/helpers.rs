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
fn host_identity() -> Digest {
    Digest::of(
        concat!(
            include_str!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/tools/src/doc/host.rs"),
            include_str!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/tools/src/doc/reader.rs"),
            include_str!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/crates/foundation/reader/src/builtin/provider.rs")
        )
        .as_bytes(),
    )
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
    with_input_route(false, compiled, input, category, finish)
}
fn with_input_route<T>(
    native: bool,
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
    // Fixture implementation identity binds all code selected by this host.
    let implementation_for = |_: &nepl3_core::value::OperationRef| host_identity();
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
    let tree = parse_source_route(&source, &resolved, "Doc", category, &mut b, &mut a, native)?;
    let checked = tree.validate(&resolved, &mut b, &mut a).map_err(|e| format!("tree validation: {e:?}; usage={:?}", b.usage()))?;
    finish(&checked, &resolved, &mut b, &mut a)
}

fn parse_source_as(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<nepl3_engine::recovery::ParseTree, String> {
    parse_source_route(source, resolved, alias, category, b, a, false)
}
#[allow(clippy::too_many_arguments)]
fn parse_source_route(
    source: &SourceSnapshot,
    resolved: &ResolvedParseProfile<'_>,
    alias: &str,
    category: &str,
    b: &mut Budget,
    a: &mut SourceAdmission,
    native: bool,
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
    let request = ParseRequest {
        snapshot: source,
        start: 0,
        limit: source.text().len() as u64,
        final_input: true,
        entry: &entry,
        states: &states,
    };
    let mut result = if native {
        let mut host =
            nepl3_tools::doc::host::NativeHost::new(r, host_identity(), "decoded-".into(), b)
                .map_err(err)?;
        let reply = parser
            .read_with_host(request, &store, b, a, &mut host)
            .map_err(err)?;
        if let Some(error) = reply.host_error {
            let cursor = match &reply.reply.outcome { ParseOutcome::Stopped { progress, .. } => progress.as_ref().map(|p| p.cursor), _ => None };
            return Err(format!("native host: {error:?}; cursor={cursor:?}; usage={:?}", b.usage()));
        }
        reply.reply
    } else {
        parser.read(request, &store, b, a).map_err(err)?
    };
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
                // A prefix parse ends after its root, before trailing source
                // whitespace. Keep the original snapshot, including file LF.
                let tail = usize::try_from(cursor)
                    .ok()
                    .and_then(|offset| source.text().get(offset..))
                    .ok_or("invalid completed source cursor")?;
                assert!(
                    tail.bytes()
                        .all(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
                );
                return Ok(tree);
            }
            ParseOutcome::Stopped { reason, progress } => {
                return Err(format!(
                    "candidate stopped: {reason:?}; cursor={:?}; usage={:?}",
                    progress.as_ref().map(|p| p.cursor),
                    b.usage()
                ));
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
        include_bytes!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/conformance/fixtures/doc/syntax.json"),
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
            include_bytes!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/conformance/fixtures/doc/math.json").as_slice(),
        ),
        (
            "circuit",
            include_bytes!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/conformance/fixtures/doc/circuit.json").as_slice(),
        ),
        (
            "grammar",
            include_bytes!("C:/projects/NEPL3-runtime/.tmp/review-doc-html-current/workspace/conformance/fixtures/doc/grammar.json").as_slice(),
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
