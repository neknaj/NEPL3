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
fn received_plan_uses_typed_execution_and_admitted_source_mapping() -> Result<(), String> {
    with_program(
        "add framed frame neg 7 2",
        |program, sources, runtime, registry| {
            let schema = &runtime.operations[0].schema;
            let encoded = transfer::encode(program, schema, &mut budget()).map_err(error)?;
            let TypedValue::Record(record) = encoded else {
                return Err("plan record".into());
            };
            let bytes =
                nepl3_wire::encode(&NdfValue::Record(record), &mut budget()).map_err(error)?;
            let NdfValue::Record(ref record) =
                nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?
            else {
                return Err("received plan record".into());
            };
            let received = TypedValue::Record(record.clone());
            let heads = program
                .nodes()
                .iter()
                .map(|node| node.head.cloned())
                .collect::<Vec<_>>();
            let checked =
                || transfer::validate(&received, schema, registry, &mut budget()).map_err(error);
            assert!(matches!(
                checked()?.program(&[], sources, &mut budget()),
                Err(transfer::Error::Reference)
            ));
            assert!(matches!(
                checked()?.program(&heads, &SourceStore::default(), &mut budget()),
                Err(transfer::Error::Source(SourceError::MissingSnapshot))
            ));
            let mut limits = budget().limits();
            limits.allocation_units = 0;
            assert!(matches!(
                checked()?.program(&heads, sources, &mut Budget::new(limits)),
                Err(transfer::Error::Stopped(StopReason::AllocationLimit))
            ));
            let admitted = checked()?
                .program(&heads, sources, &mut budget())
                .map_err(error)?;
            assert_eq!(admitted.nodes().len(), program.nodes().len());
            for (received, original) in admitted.nodes().iter().zip(program.nodes()) {
                assert_eq!(received.language, original.language);
                assert_eq!(received.head, original.head);
            }
            let result = runtime
                .run(
                    &admitted,
                    sources,
                    registry,
                    &mut budget(),
                    &mut budget(),
                    |_, _| {},
                    |_| {},
                )
                .map_err(error)?;
            let OperationResult::Complete {
                value: TypedValue::Record(value),
                ..
            } = result
            else {
                return Err("received plan evaluation".into());
            };
            assert_eq!(value.fields, vec![NdfValue::Integer(Integer::from(-5_i64))]);
            Ok(())
        },
    )
}

#[test]
fn source_admission_rejects_missing_and_changed_snapshots_before_execution() -> Result<(), String> {
    with_program(
        "add framed frame neg 7 2",
        |program, sources, runtime, registry| {
            let original = sources.snapshots().first().ok_or("source snapshot")?;
            // Keep the ID and revision, but change the bytes at the same positions.
            // Span admission must compare the full snapshot identity, including digest.
            let changed = SourceSnapshot::new(
                original.identity().source.clone(),
                original.identity().revision,
                original.uri().into(),
                b"add framed frame neg 8 2".to_vec(),
                &mut budget(),
            )
            .map_err(error)?;
            let mut changed_sources = SourceStore::default();
            changed_sources
                .insert_ref_with_budget(&changed, &mut budget())
                .map_err(error)?;
            for (supplied, expected) in [
                (SourceStore::default(), SourceError::MissingSnapshot),
                (changed_sources, SourceError::SnapshotMismatch),
            ] {
                let mut execution = budget();
                let before = execution.usage();
                let mut reports = Vec::new();
                let mut cancelled = Vec::new();
                let result = runtime.run(
                    program,
                    &supplied,
                    registry,
                    &mut execution,
                    &mut budget(),
                    |id, _| reports.push(id),
                    |id| cancelled.push(id),
                );
                let Err(Error::Source(actual)) = result else {
                    return Err("expected source admission rejection".into());
                };
                assert_eq!(actual, expected);
                assert_eq!(execution.usage(), before);
                assert_eq!(execution.poll(), Ok(()));
                assert!(reports.is_empty());
                assert!(cancelled.is_empty());
            }
            Ok(())
        },
    )
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
                    let failure = match result {
                        Err(Error::Execution(failure)) => failure,
                        Ok(OperationResult::Stopped {
                            reason,
                            partial,
                            report,
                        }) => {
                            assert_eq!(reason, expected);
                            assert_eq!(execution.poll(), Err(expected));
                            assert!(partial.is_none());
                            assert_eq!(report.diagnostics[0].code, "evaluation-stopped");
                            assert_eq!(
                                report.diagnostics[0].primary.as_ref(),
                                program.nodes().last().and_then(|node| node.head)
                            );
                            assert!(cancelled.is_empty());
                            continue;
                        }
                        _ => return Err(format!("expected {expected:?} at {limit}")),
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
fn stopped_guest_arithmetic_report_keeps_its_source_and_request() -> Result<(), String> {
    let large = "1234567890123456789012345678901234567890123456789012345678901234567890";
    with_program(
        &format!("add framed frame mul {large} {large} 2"),
        |program, sources, runtime, registry| {
            let mut limits = budget().limits();
            limits.work = 20_000;
            let mut execution = Budget::new(limits);
            let mut awaits = Vec::new();
            let mut cancelled = Vec::new();
            let result = runtime.run(
                program,
                sources,
                registry,
                &mut execution,
                &mut budget(),
                |id, _| awaits.push(id),
                |id| cancelled.push(id),
            );
            let Err(Error::Execution(failure)) = result else {
                return Err("expected stopped guest to block its parent".into());
            };
            assert_eq!(execution.poll(), Err(StopReason::WorkLimit));
            assert_eq!(awaits, vec![7, 5, 4, 3]);
            let accepted = failure.accepted_results().collect::<Vec<_>>();
            assert_eq!(accepted.len(), 1);
            let (call, result) = accepted[0];
            assert_eq!(call.request_id, 3);
            let OperationResult::Stopped {
                reason,
                partial,
                report,
            } = result
            else {
                return Err("expected accepted Stopped".into());
            };
            assert_eq!(report.usage, execution.usage());
            assert_eq!(*reason, StopReason::WorkLimit);
            assert!(partial.is_none());
            assert_eq!(report.diagnostics.len(), 1);
            let diagnostic = &report.diagnostics[0];
            assert_eq!(diagnostic.code, "evaluation-stopped");
            assert_eq!(diagnostic.schema, call.operation.schema);
            assert_eq!(diagnostic.arguments, call.input);
            let span = diagnostic.primary.as_ref().ok_or("guest span")?;
            assert_eq!((span.start(), span.end()), (17, 20));
            let identity = span.snapshot_ref();
            let source = sources
                .get_revision_with_budget(&identity.source, identity.revision, &mut budget())
                .map_err(error)?
                .ok_or("authorized source")?;
            assert_eq!(source.slice(span).map_err(error)?, "mul");
            report
                .validate_with_sources(sources, registry, &mut budget())
                .map_err(error)?;
            assert!(
                report
                    .validate_with_sources(&SourceStore::default(), registry, &mut budget())
                    .is_err()
            );
            assert!(!cancelled.contains(&3));
            assert_eq!(failure.active_request_id(), 4);
            assert_eq!(
                cancelled
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                cancelled.len()
            );
            Ok(())
        },
    )
}

#[test]
fn diagnostic_preparation_stop_uses_host_context_without_claiming_a_report() -> Result<(), String> {
    with_program("neg 7", |program, sources, runtime, registry| {
        let mut limits = budget().limits();
        limits.diagnostics = 0;
        let mut execution = Budget::new(limits);
        let result = runtime.run(
            program,
            sources,
            registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |_| {},
        );
        let Err(Error::Execution(failure)) = result else {
            return Err("expected preparation stop".into());
        };
        assert_eq!(execution.poll(), Err(StopReason::DiagnosticLimit));
        assert_eq!(failure.active_request_id(), 2);
        let node = program.request_node(2).ok_or("neg node")?;
        let span = node.head.ok_or("neg span")?;
        assert_eq!((span.start(), span.end()), (0, 3));
        assert!(failure.accepted_results().next().is_none());
        Ok(())
    })
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

#[test]
fn every_reply_reports_cumulative_execution_usage() -> Result<(), String> {
    with_program(
        "add framed frame neg 7 2",
        |program, sources, runtime, registry| {
            let mut execution = budget();
            execution.charge(Resource::Work, 123).map_err(error)?;
            let mut observations = Vec::new();
            let result = runtime
                .run(
                    program,
                    sources,
                    registry,
                    &mut execution,
                    &mut budget(),
                    |id, report| observations.push((id, report.usage)),
                    |_| {},
                )
                .map_err(error)?;
            let OperationResult::Complete { report, .. } = result else {
                return Err("expected Complete".into());
            };
            assert_eq!(report.usage, execution.usage());
            assert_eq!(
                observations.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
                vec![6, 4, 3, 2]
            );
            let mut previous = 123;
            for (_, usage) in observations {
                assert!(usage.work > previous);
                previous = usage.work;
            }
            assert!(report.usage.work > previous);

            // A schema-shaped selection with an unknown occurrence is rejected by
            // the language callback; even that Invalid includes prior consumption.
            let mut construction = budget();
            let call = Invoke {
                request_id: u64::MAX,
                operation: copy_operation(&runtime.operations[0], &mut construction)
                    .map_err(error)?,
                input: record(
                    &runtime.operations[0].schema,
                    "Selection",
                    [NdfValue::U64(0)],
                    &mut construction,
                )
                .map_err(error)?,
                environment: record(
                    &runtime.operations[0].schema,
                    "PlanIdentity",
                    [NdfValue::Bytes(vec![0; 32])],
                    &mut construction,
                )
                .map_err(error)?,
                sources: vec![],
                resources: vec![],
                limits: execution.limits(),
            };
            let before = execution.usage().work;
            let OperationReply::Result(OperationResult::Invalid { report, .. }) = invoke(
                program,
                &call,
                Digest::of(b"test context"),
                false,
                &mut execution,
            )
            .map_err(error)?
            else {
                return Err("expected Invalid selection".into());
            };
            assert_eq!(report.usage, execution.usage());
            assert!(report.usage.work > before);
            Ok(())
        },
    )
}
