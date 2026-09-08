//! Host orchestration of the real reader/engine/lower path. Providers are selected
//! explicitly; this adapter does not emulate tokenization or prefix parsing.
mod host;
pub(super) mod metrics;
use metrics::Metrics;
use nepl3_core::{
    budget::{Budget, Resource, StopReason, Usage},
    schema::{TypeDescriptor, TypeShape},
    source::{Digest, SourceAdmission, SourceId, SourceReservation, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, SchemaRef},
};
use nepl3_engine::{parse::*, profile::*};
use nepl3_grammar_core::{compile::package::CompiledLanguage, model::Document};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::{ProviderCall, ReadRequest, ReaderContext},
    runtime::ProviderReply,
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};
pub enum RuntimeError {
    Boundary(String),
    Host {
        error: ParseError,
        reply: Box<ParseReply>,
    },
    /// Setup stopped before a parser reply existed. No successful tree or fake
    /// parser progress is supplied; usage belongs to the caller's shared budget.
    PreparationStopped {
        reason: StopReason,
        usage: Usage,
    },
    /// The last formally accepted parser reply (including its source closure),
    /// with cumulative usage updated through host provider/trivia/lower work.
    /// Its outcome can be Await or Complete when later host work stopped.
    Stopped {
        reason: StopReason,
        reply: Box<ParseReply>,
    },
    Incomplete(Box<ParseReply>),
    TrailingInput(u64),
}
impl core::fmt::Debug for RuntimeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Boundary(error) => f.debug_tuple("Boundary").field(error).finish(),
            Self::Host { error, reply } => f
                .debug_struct("Host")
                .field("error", error)
                .field("usage", &reply.report.usage)
                .finish(),
            Self::PreparationStopped { reason, usage } => f
                .debug_struct("PreparationStopped")
                .field("reason", reason)
                .field("usage", usage)
                .finish(),
            Self::TrailingInput(offset) => f.debug_tuple("TrailingInput").field(offset).finish(),
            Self::Stopped { reason, reply } => f
                .debug_struct("Stopped")
                .field("reason", reason)
                .field("usage", &reply.report.usage)
                .field(
                    "cursor",
                    &match &reply.outcome {
                        ParseOutcome::Stopped { progress, .. } => {
                            progress.as_ref().map(|v| v.cursor)
                        }
                        _ => None,
                    },
                )
                .finish(),
            Self::Incomplete(reply) => {
                let outcome = match &reply.outcome {
                    ParseOutcome::Complete { .. } => "Complete",
                    ParseOutcome::Recovered { .. } => "Recovered",
                    ParseOutcome::NeedMore { .. } => "NeedMore",
                    ParseOutcome::Stopped { .. } => "Stopped",
                    ParseOutcome::Await { .. } => "Await",
                    ParseOutcome::AwaitHead { .. } => "AwaitHead",
                    ParseOutcome::Reserve { .. } => "Reserve",
                };
                f.debug_struct("Incomplete")
                    .field("outcome", &outcome)
                    .field("usage", &reply.report.usage)
                    .finish()
            }
        }
    }
}
fn boundary(v: impl core::fmt::Debug) -> RuntimeError {
    RuntimeError::Boundary(format!("{v:?}"))
}
// Use only before the first parser invocation: no formal reply exists yet.
// Once a reply exists the driver must retain it in RuntimeError::Stopped.
fn preparation(error: RuntimeError, budget: &Budget) -> RuntimeError {
    match budget.poll() {
        Err(reason) => RuntimeError::PreparationStopped {
            reason,
            usage: budget.usage(),
        },
        Ok(()) => error,
    }
}
/// Exact identity of the host executable implementing this driver and native providers.
pub fn executable_identity() -> Result<Digest, RuntimeError> {
    let path = std::env::current_exe().map_err(boundary)?;
    let bytes = std::fs::read(path).map_err(boundary)?;
    Ok(Digest::of(&bytes))
}
fn schemas(
    compiled: &CompiledLanguage,
    budget: &mut Budget,
) -> Result<Vec<SchemaRef>, RuntimeError> {
    let registry = &compiled.registry;
    let mut output = vec![compiled.package.schema.clone()];
    for name in ["nepl3.foundation", "nepl3.reader", "nepl3.engine"] {
        let reference = registry
            .selected(name, 1)
            .ok_or_else(|| boundary("missing runtime schema"))?;
        if !output.contains(reference) {
            output.push(reference.clone());
        }
    }
    for reference in &compiled.package.payload_schemas {
        if !output.contains(reference) {
            output.push(reference.clone());
        }
    }
    let mut index = 0;
    while index < output.len() {
        let descriptor = registry
            .descriptor(&output[index])
            .ok_or_else(|| boundary("unregistered schema"))?;
        let mut pending = Vec::new();
        for named in &descriptor.types {
            match &named.shape {
                TypeShape::Record { fields } => pending.extend(fields.iter().map(|v| &v.ty)),
                TypeShape::Variant { variants } => {
                    for variant in variants {
                        pending.extend(variant.fields.iter().map(|v| &v.ty));
                    }
                }
            }
        }
        for op in &descriptor.operations {
            pending.push(&op.input);
            pending.push(&op.output);
        }
        while let Some(ty) = pending.pop() {
            budget.charge(Resource::Work, 1).map_err(boundary)?;
            match ty {
                TypeDescriptor::Named(reference) => {
                    let selected = registry
                        .selected(&reference.package, reference.revision)
                        .ok_or_else(|| boundary("missing named schema"))?;
                    if !output.contains(selected) {
                        output.push(selected.clone());
                    }
                }
                TypeDescriptor::List(inner) | TypeDescriptor::Option(inner) => pending.push(inner),
                _ => {}
            }
        }
        index += 1;
    }
    Ok(output)
}
/// Parse one complete Grammar document with the compiled package, explicitly
/// service standard native-reader calls, and lower the validated tree.
/// `implementation` is the real host binary/manifest identity supplied by the caller.
pub fn parse(
    source: &SourceSnapshot,
    compiled: &CompiledLanguage,
    implementation: Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Document, RuntimeError> {
    let result = with_tree(
        source,
        compiled,
        implementation,
        budget,
        admission,
        |tree, profile, budget, admission| {
            nepl3_grammar_core::lower::lower(tree, profile, budget, admission).map_err(boundary)
        },
    );
    match result {
        Err(error @ RuntimeError::Stopped { .. }) => Err(error),
        Err(error) => match budget.poll() {
            Err(reason) => Err(RuntimeError::PreparationStopped {
                reason,
                usage: budget.usage(),
            }),
            Ok(()) => Err(error),
        },
        Ok(document) => Ok(document),
    }
}
pub(super) fn with_tree<T>(
    source: &SourceSnapshot,
    compiled: &CompiledLanguage,
    implementation: Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
    finish: impl FnOnce(
        &nepl3_engine::tree::ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, RuntimeError>,
) -> Result<T, RuntimeError> {
    with_tree_measured(
        source,
        compiled,
        implementation,
        budget,
        admission,
        &mut Metrics {
            inline_host: true,
            ..Metrics::default()
        },
        finish,
    )
}
pub(super) fn with_tree_measured<T>(
    source: &SourceSnapshot,
    compiled: &CompiledLanguage,
    implementation: Digest,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
    metrics: &mut Metrics,
    finish: impl FnOnce(
        &nepl3_engine::tree::ValidatedParseTree<'_>,
        &ResolvedParseProfile<'_>,
        &mut Budget,
        &mut SourceAdmission,
    ) -> Result<T, RuntimeError>,
) -> Result<T, RuntimeError> {
    let registry = &compiled.registry;
    let package = &compiled.package;
    let identity = package
        .check(registry, budget)
        .map_err(|error| preparation(boundary(error), budget))?
        .semantic_identity(budget)
        .map_err(|error| preparation(boundary(error), budget))?;
    let mut providers = Vec::new();
    let mut requirements = Vec::new();
    let mut allowlist = Vec::new();
    for signature in &package.reader.providers {
        let kind = match signature.operation.name.as_str() {
            "builtinName" => BuiltinReader::Name,
            "builtinTrivia" => BuiltinReader::Trivia,
            "builtinNumber" => BuiltinReader::Number,
            _ => return Err(boundary("unsupported native standard provider")),
        };
        let expected = provider::signature(kind, registry, budget)
            .map_err(|error| preparation(boundary(error), budget))?;
        if signature != &expected {
            return Err(boundary("standard signature mismatch"));
        }
        let provider_name = format!("native.{}", signature.operation.name);
        providers.push(ProviderImplementation {
            provider: provider_name.clone(),
            revision: 1,
            implementation_digest: implementation,
            operations: vec![signature.operation.clone()],
        });
        requirements.push(ProviderRequirement {
            provider: provider_name,
            revision: 1,
            implementation_digest: implementation,
            operation: signature.operation.clone(),
        });
        allowlist.push(signature.operation.clone());
    }
    let profile = ParseProfile {
        id: "grammar-bootstrap.native".into(),
        languages: vec![LanguageRegistration {
            alias: "Grammar".into(),
            package: identity,
            default_category: package.root.clone(),
        }],
        schemas: schemas(compiled, budget).map_err(|error| preparation(error, budget))?,
        head_providers: vec![],
        category_modes: vec![],
        providers: requirements,
        allowlist,
        resources: vec![],
        limits: budget.limits(),
    };
    let packages = [package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &[],
            },
            registry,
            budget,
        )
        .map_err(|error| preparation(boundary(error), budget))?;
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(
            source
                .clone_with_budget(budget)
                .map_err(|error| preparation(boundary(error), budget))?,
            budget,
        )
        .map_err(|error| preparation(boundary(error), budget))?;
    let foundation = registry
        .selected("nepl3.foundation", 1)
        .ok_or_else(|| boundary("foundation"))?;
    let value = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = environment_digest(&value, foundation, registry, budget)
        .map_err(|error| preparation(boundary(error), budget))?;
    let entry = resolved
        .entry("Grammar", None, budget)
        .map_err(|error| preparation(boundary(error), budget))?;
    let raw = ReaderContext {
        schema: package.schema.clone(),
        category: entry.category.clone(),
        mode: entry.mode.clone(),
        origins: vec![],
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value,
        },
    };
    let environments = {
        let mut codec = FoundationCodec::new(registry, &sources, admission)
            .map_err(|error| preparation(boundary(error), budget))?;
        let checked = raw
            .check(&mut codec, &sources, registry, budget)
            .map_err(|error| preparation(boundary(error), budget))?;
        ParseEnvironmentSet::prepare(
            &resolved,
            &[EnvironmentInput {
                alias: "Grammar",
                context: &checked,
            }],
            &sources,
            &mut codec,
            budget,
        )
        .map_err(|error| preparation(boundary(error), budget))?
    };
    let mut parser =
        ParseSession::new("grammar-bootstrap".into(), &resolved, &environments, budget)
            .map_err(|error| preparation(boundary(error), budget))?;
    let states = [LanguageReaderState {
        alias: "Grammar".into(),
        state: NdfValue::Unit,
    }];
    let request = ParseRequest {
        snapshot: source,
        start: 0,
        limit: source.text().len() as u64,
        final_input: true,
        entry: &entry,
        states: &states,
    };
    let mut initial = core::mem::take(&mut metrics.initial);
    let mut reservation_id = 0u64;
    let mut reply = if metrics.inline_host {
        let mut host = host::NativeHost {
            registry,
            providers: &providers,
            source,
            reservation_id: 0,
            metrics,
        };
        let result = initial
            .measure(budget, |budget| {
                parser.read_with_host(request, &sources, budget, admission, &mut host)
            })
            .map_err(boundary)?;
        reservation_id = host.reservation_id;
        if let Some(error) = result.host_error {
            return match budget.poll() {
                Err(reason) => Err(RuntimeError::Stopped {
                    reason,
                    reply: Box::new(result.reply),
                }),
                Ok(()) => Err(RuntimeError::Host {
                    error,
                    reply: Box::new(result.reply),
                }),
            };
        }
        result.reply
    } else {
        initial
            .measure(budget, |budget| {
                parser.read(request, &sources, budget, admission)
            })
            .map_err(boundary)?
    };
    metrics.initial = initial;
    let mut finish = Some(finish);
    enum Step<T> {
        Next,
        Done(T),
        Incomplete,
        Stopped(StopReason),
    }
    loop {
        let step = (|| -> Result<Step<T>, RuntimeError> {
            match &reply.outcome {
                ParseOutcome::Await { call, continuation } => {
                    if metrics.probe_continuations {
                        let mut probe = Budget::new(budget.limits());
                        metrics
                            .tokenizer_continuation_copy
                            .measure(&mut probe, |b| continuation.tokenizer.charge_clone(b))
                            .map_err(boundary)?;
                        if let nepl3_reader::tokenizer::TokenizationWait::Provider {
                            continuation: reader,
                        } = &continuation.tokenizer.pending
                        {
                            metrics
                                .reader_continuation_copy
                                .measure(&mut probe, |b| reader.charge_clone(b))
                                .map_err(boundary)?;
                        }
                    }
                    let ProviderCall::Read {
                        operation,
                        request,
                        depth_base,
                        ..
                    } = call.as_ref()
                    else {
                        return Err(boundary("non-read standard operation"));
                    };
                    let declared = metrics.provider_sources.measure(
                        budget,
                        |budget| -> Result<_, RuntimeError> {
                            let mut declared = SourceStore::default();
                            for source in &request.sources {
                                declared
                                    .insert_with_budget(
                                        source.clone_with_budget(budget).map_err(boundary)?,
                                        budget,
                                    )
                                    .map_err(boundary)?;
                            }
                            Ok(declared)
                        },
                    )?;
                    let snapshot = declared
                        .resolve(&request.snapshot)
                        .ok_or_else(|| boundary("provider snapshot missing"))?;
                    let checked = metrics.provider_context.measure(budget, |budget| {
                        let mut codec = FoundationCodec::new(registry, &declared, admission)
                            .map_err(boundary)?;
                        request
                            .context
                            .check(&mut codec, &declared, registry, budget)
                            .map_err(boundary)
                    })?;
                    let terminal = metrics
                        .provider_read
                        .measure(budget, |budget| {
                            budget.with_depth_at_least(*depth_base, |b| {
                                provider::read(
                                    operation,
                                    ReadRequest {
                                        snapshot,
                                        start: request.start,
                                        limit: request.limit,
                                        final_input: request.final_input,
                                        context: &checked,
                                        state: &request.state,
                                    },
                                    registry,
                                    &declared,
                                    b,
                                    admission,
                                )
                            })
                        })
                        .map_err(boundary)?;
                    let next = metrics
                        .resume
                        .measure(budget, |budget| {
                            parser.resume(
                                continuation,
                                ProviderReply::Read(Box::new(terminal)),
                                &sources,
                                budget,
                                admission,
                            )
                        })
                        .map_err(boundary)?;
                    reply = next;
                    Ok(Step::Next)
                }
                ParseOutcome::Reserve { continuation, .. } => {
                    reservation_id = reservation_id
                        .checked_add(1)
                        .ok_or_else(|| boundary("reservation overflow"))?;
                    let reservation = SourceReservation {
                        source_id: SourceId(format!(
                            "{}:decoded:{}",
                            source.identity().source.0,
                            reservation_id
                        )),
                        revision: source.identity().revision,
                        uri: format!("memory:grammar-decoded/{reservation_id}"),
                    };
                    let next = metrics
                        .reservation
                        .measure(budget, |budget| {
                            parser.reserve(continuation, &reservation, &sources, budget, admission)
                        })
                        .map_err(boundary)?;
                    reply = next;
                    Ok(Step::Next)
                }
                ParseOutcome::Complete { tree, cursor, .. } => {
                    // read-one leaves host trailing trivia. The file driver accepts only actual
                    // standard trivia to EOF, never a second expression or arbitrary whitespace.
                    let mut cursor = *cursor;
                    let checked = {
                        let mut codec = FoundationCodec::new(registry, &sources, admission)
                            .map_err(boundary)?;
                        raw.check(&mut codec, &sources, registry, budget)
                            .map_err(boundary)?
                    };
                    while cursor < source.text().len() as u64 {
                        let result = nepl3_reader::builtin::read(
                            BuiltinReader::Trivia,
                            ReadRequest {
                                snapshot: source,
                                start: cursor,
                                limit: source.text().len() as u64,
                                final_input: true,
                                context: &checked,
                                state: &NdfValue::Unit,
                            },
                            None,
                            registry,
                            &sources,
                            budget,
                            admission,
                        )
                        .map_err(boundary)?;
                        match result {
                            nepl3_reader::model::ReadReply::Matched { end, .. } if end > cursor => {
                                cursor = end
                            }
                            nepl3_reader::model::ReadReply::Stopped { reason, .. } => {
                                return Ok(Step::Stopped(reason));
                            }
                            _ => return Err(RuntimeError::TrailingInput(cursor)),
                        }
                    }
                    let checked = tree
                        .validate(&resolved, budget, admission)
                        .map_err(boundary)?;
                    let finish = finish
                        .take()
                        .ok_or_else(|| boundary("completed callback already consumed"))?;
                    Ok(Step::Done(finish(&checked, &resolved, budget, admission)?))
                }
                ParseOutcome::Stopped { reason, .. } => Ok(Step::Stopped(*reason)),
                _ => Ok(Step::Incomplete),
            }
        })();
        match step {
            Ok(Step::Next) => {}
            Ok(Step::Done(value)) => return Ok(value),
            Ok(Step::Incomplete) => return Err(RuntimeError::Incomplete(Box::new(reply))),
            Ok(Step::Stopped(reason)) => {
                reply.report.usage = budget.usage();
                return Err(RuntimeError::Stopped {
                    reason,
                    reply: Box::new(reply),
                });
            }
            Err(error) => match budget.poll() {
                Err(reason) => {
                    reply.report.usage = budget.usage();
                    return Err(RuntimeError::Stopped {
                        reason,
                        reply: Box::new(reply),
                    });
                }
                Ok(()) => return Err(error),
            },
        }
    }
}
