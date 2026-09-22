//! Evaluate MiniExpr/Frame using public package APIs and the native scheduler.
use external_composition_runtime::{
    execution::{self, Runtime},
    program,
    syntax::Cursor,
};
use external_hello_language::{budget, composition, error};
use nepl3_core::{
    diagnostic::OperationResult,
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::{NdfValue, TypedValue},
};
use nepl3_engine::{parse::ParseOutcome, profile::RuntimeCatalog};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: evaluate \"add framed frame neg 7 2\"")?;
    if args.next().is_some() {
        return Err("expected one quoted input".into());
    }
    let languages = composition::languages("Expr", "Frame")?;
    let profile = languages.profile("composition")?;
    let packages = languages
        .packages
        .iter()
        .map(|(_, package)| package)
        .collect::<Vec<_>>();
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &languages.registry,
            &mut budget(),
        )
        .map_err(error)?;
    let observation = composition::inspect(&input, true)?;
    let ParseOutcome::Complete { tree, .. } = observation.parse.outcome else {
        return Err("evaluation requires a complete, unrecovered parse".into());
    };
    let proof = tree
        .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
        .map_err(error)?;
    let root = Cursor::root(
        &proof,
        &packages[0].schema,
        &packages[1].schema,
        &mut budget(),
    )
    .map_err(error)?;
    let plan = program::compile(root, &mut budget()).map_err(error)?;
    let mut registry = SchemaRegistry::default();
    // Example executable version identities, not package distribution signatures.
    let runtime = Runtime::register(
        &mut registry,
        [
            Digest::of(b"MiniExpr example evaluator v1"),
            Digest::of(b"Frame example evaluator v1"),
        ],
        &mut budget(),
    )
    .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let mut sources = SourceStore::default();
    for source in &tree.bundle.sources {
        sources
            .insert_ref_with_budget(source, &mut budget())
            .map_err(error)?;
    }
    let result = runtime
        .run(
            &plan,
            &sources,
            &registry,
            &mut budget(),
            &mut budget(),
            |id, report| eprintln!("Await request {id}: {report:?}"),
            |id| eprintln!("Cancelled request {id}"),
        )
        .map_err(|failure| {
            if let execution::Error::Execution(scheduler) = &failure {
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
}
