use super::*;
use crate::{program, syntax::Cursor};
use external_hello_language::{budget, composition, error};
use nepl3_core::source::SourceAdmission;
use nepl3_engine::{parse::ParseOutcome, profile::RuntimeCatalog};

fn with_program(
    text: &str,
    check: impl FnOnce(&Program<'_>, &SourceStore, &Runtime, &SchemaRegistry) -> Result<(), String>,
) -> Result<(), String> {
    let languages = composition::languages("Expr", "Frame")?;
    let profile = languages.profile("composition")?;
    let packages = languages
        .packages
        .iter()
        .map(|(_, p)| p)
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
    let observation = composition::inspect(text, true)?;
    let ParseOutcome::Complete { tree, .. } = observation.parse.outcome else {
        return Err("expected complete parse".into());
    };
    let proof = tree
        .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
        .map_err(error)?;
    let cursor = Cursor::root(
        &proof,
        &packages[0].schema,
        &packages[1].schema,
        &mut budget(),
    )
    .map_err(error)?;
    let program = program::compile(cursor, &mut budget()).map_err(error)?;
    let mut sources = SourceStore::default();
    for source in &tree.bundle.sources {
        sources
            .insert_ref_with_budget(source, &mut budget())
            .map_err(error)?;
    }
    let mut registry = SchemaRegistry::default();
    let runtime = Runtime::register(
        &mut registry,
        [
            Digest::of(b"MiniExpr test evaluator"),
            Digest::of(b"Frame test evaluator"),
        ],
        &mut budget(),
    )
    .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    check(&program, &sources, &runtime, &registry)
}

#[test]
fn nested_operations_and_depth_stop_preserve_request_source() -> Result<(), String> {
    for depth in [1, 8, 24] {
        // Direct source construction exercises the real parser on increasing
        // nesting independently of its printer and the operation adapter.
        let text = format!("{}7", "neg ".repeat(depth));
        with_program(&text, |program, sources, runtime, registry| {
            let mut awaits = Vec::new();
            let mut cancelled = Vec::new();
            let result = runtime
                .run(
                    program,
                    sources,
                    registry,
                    &mut budget(),
                    &mut budget(),
                    |id, _| awaits.push(id),
                    |id| cancelled.push(id),
                )
                .map_err(error)?;
            let OperationResult::Complete {
                value: TypedValue::Record(value),
                ..
            } = result
            else {
                return Err("expected complete evaluation".into());
            };
            assert_eq!(
                value.fields,
                vec![NdfValue::Integer(Integer::from(if depth % 2 == 0 {
                    7_i64
                } else {
                    -7_i64
                }))]
            );
            assert_eq!(awaits.len(), depth);
            assert!(cancelled.is_empty());
            let mut limits = budget().limits();
            limits.depth = 1;
            let mut execution = Budget::new(limits);
            let result = runtime.run(
                program,
                sources,
                registry,
                &mut execution,
                &mut budget(),
                |_, _| {},
                |id| cancelled.push(id),
            );
            let Err(Error::Execution(failure)) = result else {
                return Err("expected depth failure".into());
            };
            assert_eq!(execution.poll(), Err(StopReason::DepthLimit));
            // Child activation failed while the root frame was active.
            assert_eq!(failure.active_request_id(), depth as u64 + 1);
            let node = program
                .request_node(failure.active_request_id())
                .ok_or("failure occurrence")?;
            let span = node.head.ok_or("failure head span")?;
            assert_eq!((span.start(), span.end()), (0, 3));
            assert!(failure.accepted_results().next().is_none());
            assert_eq!(
                cancelled
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                cancelled.len()
            );
            assert!(program.request_node(0).is_none());
            assert!(program.request_node(u64::MAX).is_none());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn work_and_allocation_stops_retain_inspectable_active_occurrence() -> Result<(), String> {
    with_program(
        "add framed frame neg 7 2",
        |program, sources, runtime, registry| {
            let mut measured = budget();
            runtime
                .run(
                    program,
                    sources,
                    registry,
                    &mut measured,
                    &mut budget(),
                    |_, _| {},
                    |_| {},
                )
                .map_err(error)?;
            for allocation in [false, true] {
                let total = if allocation {
                    measured.usage().allocation_units
                } else {
                    measured.usage().work
                };
                for limit in [0, 1, total / 4, total / 2, total - 1] {
                    let mut limits = budget().limits();
                    let expected = if allocation {
                        limits.allocation_units = limit;
                        StopReason::AllocationLimit
                    } else {
                        limits.work = limit;
                        StopReason::WorkLimit
                    };
                    let mut execution = Budget::new(limits);
                    let mut cancelled = Vec::new();
                    let result = runtime.run(
                        program,
                        sources,
                        registry,
                        &mut execution,
                        &mut budget(),
                        |_, _| {},
                        |id| cancelled.push(id),
                    );
                    let Err(Error::Execution(failure)) = result else {
                        return Err(format!("expected {expected:?} at {limit}"));
                    };
                    assert_eq!(execution.poll(), Err(expected));
                    let before = execution.usage();
                    let node = program
                        .request_node(failure.active_request_id())
                        .ok_or("active occurrence")?;
                    assert!(node.head.is_some());
                    assert_eq!(before, execution.usage());
                    assert_eq!(
                        cancelled
                            .iter()
                            .collect::<std::collections::BTreeSet<_>>()
                            .len(),
                        cancelled.len()
                    );
                    for (call, _) in failure.accepted_results() {
                        assert!(!cancelled.contains(&call.request_id));
                        assert!(program.request_node(call.request_id).is_some());
                    }
                }
            }
            Ok(())
        },
    )
}

#[test]
fn execution_allocation_scales_with_occurrences_without_plan_copies() -> Result<(), String> {
    let mut allocations = Vec::new();
    for depth in [8, 16, 32] {
        with_program(
            &format!("{}7", "neg ".repeat(depth)),
            |program, sources, runtime, registry| {
                let mut execution = budget();
                runtime
                    .run(
                        program,
                        sources,
                        registry,
                        &mut execution,
                        &mut budget(),
                        |_, _| {},
                        |_| {},
                    )
                    .map_err(error)?;
                allocations.push(execution.usage().allocation_units);
                Ok(())
            },
        )?;
    }
    // A copied full plan adds N-sized payloads to each of N requests. With a
    // shared plan, each extra occurrence retains only a fixed-size selection,
    // context identity and the small source snapshot used by this fixture.
    eprintln!("execution AllocationUnits for depths 8/16/32: {allocations:?}");
    for pair in allocations.windows(2) {
        assert!(
            pair[1] < pair[0] * 3,
            "full-plan copy regression: {allocations:?}"
        );
    }
    Ok(())
}
