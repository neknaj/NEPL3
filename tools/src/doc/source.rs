//! Shared development-host standard Doc source pipeline.
//! Packages are compiled from checked bootstrap fixtures; these are not claimed
//! to be a completed generated LanguagePackage distribution or suite runtime.
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
pub fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
pub fn host_identity() -> Digest {
    Digest::of(
        concat!(
            include_str!("source.rs"),
            include_str!("host.rs"),
            include_str!("reader.rs"),
            include_str!("../../../crates/foundation/reader/src/builtin/provider.rs")
        )
        .as_bytes(),
    )
}
pub fn with_input<T>(
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
pub fn with_input_route<T>(
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
    with_named_input(native, compiled, input, "doc-input", category, finish)
}
/// Assign distinct source identities when compiling several pages into one set.
pub fn with_named_input<T>(
    native: bool,
    compiled: &Compiled,
    input: &str,
    source_name: &str,
    category: &str,
    finish: impl FnOnce(
        &ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, String>,
) -> Result<T, String> {
    with_named_input_limits(
        native,
        compiled,
        input,
        source_name,
        category,
        budget().limits(),
        finish,
    )
}
/// Configuration-only entry: select limits before creating this parse operation.
/// The selected ceiling is also bound into its resolved ParseProfile.
#[allow(clippy::too_many_arguments)]
pub fn with_named_input_limits<T>(
    native: bool,
    compiled: &Compiled,
    input: &str,
    source_name: &str,
    category: &str,
    parse_limits: Limits,
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
        limits: parse_limits,
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
    let mut b = Budget::new(parse_limits);
    let mut a = SourceAdmission::default();
    let source = SourceSnapshot::new(
        SourceId(source_name.into()),
        0,
        format!("memory:{source_name}"),
        input.as_bytes().to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let tree = parse_source_route(&source, &resolved, "Doc", category, &mut b, &mut a, native)?;
    let checked = tree
        .validate(&resolved, &mut b, &mut a)
        .map_err(|e| format!("tree validation: {e:?}; usage={:?}", b.usage()))?;
    finish(&checked, &resolved, &mut b, &mut a)
}

pub fn parse_source_as(
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
pub fn parse_source_route(
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
    // One allocator namespace per source identity. A counter alone collides
    // when independently parsed pages enter the same source bundle.
    let identity = source.identity();
    let prefix_bound = identity.source.0.len() as u64 + 43;
    b.charge(Resource::Work, prefix_bound).map_err(err)?;
    b.charge(Resource::AllocationUnits, prefix_bound)
        .map_err(err)?;
    let prefix = format!(
        "{}:{}:{}:",
        identity.source.0.len(),
        identity.source.0,
        identity.revision
    );
    let mut result = if native {
        b.charge(Resource::AllocationUnits, prefix.len() as u64)
            .map_err(err)?;
        let mut host = crate::doc::host::NativeHost::new(r, host_identity(), prefix.clone(), b)
            .map_err(err)?;
        let reply = parser
            .read_with_host(request, &store, b, a, &mut host)
            .map_err(err)?;
        if let Some(error) = reply.host_error {
            let cursor = match &reply.reply.outcome {
                ParseOutcome::Stopped { progress, .. } => progress.as_ref().map(|p| p.cursor),
                _ => None,
            };
            return Err(format!(
                "native host: {error:?}; cursor={cursor:?}; usage={:?}",
                b.usage()
            ));
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
                            crate::doc::reader::read
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
                    source_id: SourceId(format!("{prefix}{reservations}")),
                    revision: 0,
                    uri: format!("memory:{prefix}{reservations}"),
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
                if !tail
                    .bytes()
                    .all(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(format!("unexpected trailing input at byte {cursor}"));
                }
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

/// Desktop development-host allowance. `nodes` includes cumulative typed-value
/// validation visits, not just the number of syntax nodes in the final tree.
/// Every operation still keeps its own fixed, sticky Work/Allocation limits.
pub fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 100_000_000,
        depth: 1000,
        nodes: 10_000_000,
        allocation_units: 500_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
pub struct Compiled {
    pub doc: CompiledLanguage,
    pub others: Vec<nepl3_engine::package::LanguagePackage>,
}
pub fn compiled() -> Result<Compiled, String> {
    let document = crate::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/doc/syntax.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut doc = crate::doc::catalog::compile(
        &document,
        "standard.doc",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let mut others = Vec::new();
    for (name, seed) in [
        (
            "math",
            include_bytes!("../../../conformance/fixtures/doc/math.json").as_slice(),
        ),
        (
            "circuit",
            include_bytes!("../../../conformance/fixtures/doc/circuit.json").as_slice(),
        ),
        (
            "grammar",
            include_bytes!("../../../conformance/fixtures/doc/grammar.json").as_slice(),
        ),
    ] {
        let document = crate::bootstrap::load(seed, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let other = crate::doc::catalog::compile(
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
