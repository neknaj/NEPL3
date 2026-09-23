//! Evaluate MiniExpr/Frame using public package APIs and the native scheduler.
use external_composition_runtime::{
    execution::{self, Runtime},
    program,
    syntax::Cursor,
};
use external_hello_language::{budget, composition, error};
use nepl3_core::{
    diagnostic::OperationResult,
    source::{Digest, SourceAdmission, SourceStore},
    value::{NdfValue, TypedValue},
};
use nepl3_engine::{
    parse::ParseOutcome,
    profile::{ProviderImplementation, ProviderRequirement},
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: evaluate \"add framed frame neg 7 2\"")?;
    if args.next().is_some() {
        return Err("expected one quoted input".into());
    }
    let mut languages = composition::languages("Expr", "Frame")?;
    let expr_schema = languages.packages[0].1.schema.clone();
    let frame_schema = languages.packages[1].1.schema.clone();
    // Host-selected example executable versions.
    let implementations = [
        Digest::of(b"MiniExpr example evaluator v1"),
        Digest::of(b"Frame example evaluator v1"),
    ];
    let runtime = Runtime::register(&mut languages.registry, implementations, &mut budget())
        .map_err(error)?;
    languages.registry.finalize(&mut budget()).map_err(error)?;
    let mut profile = languages.profile("composition")?;
    profile.schemas.push(runtime.operations()[0].schema.clone());
    let mut providers = Vec::new();
    for (index, operation) in runtime.operations().iter().enumerate() {
        let provider = format!("example.evaluator.{index}");
        profile.providers.push(ProviderRequirement {
            provider: provider.clone(),
            revision: 1,
            implementation_digest: implementations[index],
            operation: operation.clone(),
        });
        profile.allowlist.push(operation.clone());
        providers.push(ProviderImplementation {
            provider,
            revision: 1,
            implementation_digest: implementations[index],
            operations: vec![operation.clone()],
        });
    }
    external_hello_language::with_registered_profile(
        &input,
        true,
        false,
        (languages, profile),
        ("Expr", "composition"),
        &providers,
        |reply, resolved, registry, _| {
            let ParseOutcome::Complete { tree, .. } = reply.outcome else {
                return Err("evaluation requires a complete, unrecovered parse".into());
            };
            let proof = tree
                .validate(resolved, &mut budget(), &mut SourceAdmission::default())
                .map_err(error)?;
            let root =
                Cursor::root(&proof, &expr_schema, &frame_schema, &mut budget()).map_err(error)?;
            let plan = program::compile(root, &mut budget()).map_err(error)?;
            let mut sources = SourceStore::default();
            for source in &tree.bundle.sources {
                sources
                    .insert_ref_with_budget(source, &mut budget())
                    .map_err(error)?;
            }
            let session = runtime
                .prepare(&plan, &sources, registry, budget().limits(), &mut budget())
                .map_err(error)?;
            let result = session
                .run_in_profile(
                    resolved,
                    &mut budget(),
                    &mut budget(),
                    |id, report| eprintln!("Await request {id}: {report:?}"),
                    |id| eprintln!("Cancelled request {id}"),
                )
                .map_err(|failure| {
                    if let execution::Error::Profile(nepl3_suite::profile::Error::Execution(
                        scheduler,
                    )) = &failure
                    {
                        let request = scheduler.active_request_id();
                        if let Some(node) = plan.request_node(request) {
                            eprintln!(
                                "Active request {request}, language {:?}, source {:?}",
                                node.language, node.head
                            );
                        }
                        for (child, outcome) in scheduler.accepted_results() {
                            eprintln!("Accepted request {}: {outcome:?}", child.request_id);
                        }
                        for pending in scheduler.uncommitted_results() {
                            eprintln!(
                                "Uncommitted terminal request {}, operation {:?}, context {:?}: {:?}",
                                pending.request_id, pending.operation, pending.context, pending.result
                            );
                        }
                        for pending in scheduler.uncommitted_await_reports() {
                            eprintln!(
                                "Uncommitted Await request {}, operation {:?}, context {:?}: {:?}",
                                pending.request_id, pending.operation, pending.context, pending.report
                            );
                        }
                    }
                    error(failure)
                })?;
            let OperationResult::Complete {
                value: TypedValue::Record(value),
                ..
            } = result
            else {
                return Err(format!("evaluation did not complete: {result:?}"));
            };
            let [NdfValue::Integer(number)] = value.fields.as_slice() else {
                return Err("expected Integer result".into());
            };
            println!("{}", number.as_bigint());
            Ok(())
        },
    )
}
