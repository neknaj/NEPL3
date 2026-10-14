use super::*;
use nepl3_math_core::{
    check, lower,
    model::*,
    portable::{self, evaluation as operation},
};

#[test]
fn evaluation_operation_first_receiver_preserves_results_and_stops() -> Result<(), String> {
    let compiled = compiled()?;
    for source in [
        "add frac 1 2 frac 1 3",
        "let x 10 add sum x 1 2 x x",
        "det matrix cons row cons 1 cons 2 nil cons row cons 3 cons 4 nil nil",
        "add x 1",
        "subscript x 1",
        "frac 1 0",
        "det 1",
        "label 7 Sentence sentence cons anno text \"\" cons text \"note\" nil nil",
    ] {
        with_input(&compiled, source, "Expr", |tree, profile, b, a| {
            let syntax = tree
                .tree()
                .bundle
                .validate_with_sources(profile.registry(), b, a)
                .map_err(err)?;
            let input = lower::expression(
                &syntax,
                &compiled.others[0].schema,
                check::Category::Expr,
                profile.registry(),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let request = MathEvaluationRequest {
                syntax: input,
                environment: BindingEnvironment {
                    assignments: vec![],
                },
            };
            let original = request.clone();
            let r = profile.registry();
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let expected =
                operation::execute(&request, r, &mut codec, &mut budget()).map_err(err)?;
            let encoded =
                operation::request_to_value(&request, r, &mut codec, &mut budget()).map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, &mut budget()).map_err(err)?;
            let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            // Fresh receiver has no original native request, source store or ledger.
            let run = |b: &mut Budget| {
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
                Ok::<_, String>(operation::execute_value(&raw, r, &mut codec, b))
            };
            let mut measured = budget();
            let result = run(&mut measured)?.map_err(err)?;
            assert_eq!(
                operation::verify_result(&result, &request, r, &mut codec, &mut budget())
                    .map_err(err)?,
                expected
            );
            assert_eq!(request, original);
            let scalar = |n: i64, d: i64| -> Result<MathExactValue, String> {
                Ok(MathExactValue::Scalar { value: nepl3_math_core::number::ratio(
                    &nepl3_core::value::Integer::from(n),
                    &nepl3_core::value::Integer::from(d), &mut budget()).map_err(err)? })
            };
            let exact = match source {
                "add frac 1 2 frac 1 3" => Some(scalar(5, 6)?),
                "let x 10 add sum x 1 2 x x" => Some(scalar(13, 1)?),
                "det matrix cons row cons 1 cons 2 nil cons row cons 3 cons 4 nil nil" => Some(scalar(-2, 1)?),
                _ => None,
            };
            if let Some(value) = exact {
                assert_eq!(expected, MathEvaluationResult::Success {
                    outcome: MathEvaluationOutcome::Exact(value)
                });
            }
            let mut shifted = budget();
            assert_eq!(shifted.with_depth_at_least(7, |b| {
                let mut a = SourceAdmission::default();
                let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(portable::PortableError::Foundation)?;
                operation::execute_value(&raw, r, &mut c, b)
            }).map_err(err)?, result);
            assert_eq!(shifted.usage().depth, measured.usage().depth + 7);
            assert_eq!(shifted.current_depth(), 0);
            if source == "add x 1" {
                let mut assigned = request.clone();
                assigned.environment.assignments.push(MathAssignment { name: "x".into(), value: scalar(2, 1)? });
                let assigned_raw = operation::request_to_value(&assigned, r, &mut codec, &mut budget()).map_err(err)?;
                let mut duplicate_raw = assigned_raw.clone();
                let NdfValue::Record(outer) = &mut duplicate_raw else { return Err("request record".into()) };
                let NdfValue::Record(env) = &mut outer.fields[1] else { return Err("environment record".into()) };
                let NdfValue::List(assignments) = &mut env.fields[0] else { return Err("assignment list".into()) };
                assignments.push(assignments[0].clone());
                let mut fresh_admission = SourceAdmission::default();
                let mut fresh_codec = FoundationCodec::new(r, &empty, &mut fresh_admission).map_err(err)?;
                assert!(matches!(operation::execute_value(&duplicate_raw, r, &mut fresh_codec, &mut budget()), Err(portable::PortableError::Environment(_))));
                let assigned_result = operation::execute_value(&assigned_raw, r, &mut codec, &mut budget()).map_err(err)?;
                assert_eq!(operation::execute(&assigned, r, &mut codec, &mut budget()).map_err(err)?, MathEvaluationResult::Success { outcome: MathEvaluationOutcome::Exact(scalar(3, 1)?) });
                assert!(matches!(operation::verify_result(&assigned_result, &request, r, &mut codec, &mut budget()), Err(portable::PortableError::EvaluationMismatch)));
                assigned.environment.assignments.push(assigned.environment.assignments[0].clone());
                assert!(operation::execute(&assigned, r, &mut codec, &mut budget()).is_err());
                assert!(operation::request_to_value(&assigned, r, &mut codec, &mut budget()).is_err());
            }

            match source {
                "add x 1" | "subscript x 1" => assert!(matches!(
                    expected,
                    MathEvaluationResult::Success {
                        outcome: MathEvaluationOutcome::Symbolic(_)
                    }
                )),
                "frac 1 0" | "det 1" => {
                    assert!(matches!(expected, MathEvaluationResult::Failure { .. }))
                }
                _ => assert!(matches!(
                    expected,
                    MathEvaluationResult::Success {
                        outcome: MathEvaluationOutcome::Exact(_)
                    }
                )),
            }
            for reason in [
                StopReason::WorkLimit,
                StopReason::AllocationLimit,
                StopReason::SourceLimit,
                StopReason::NodeLimit,
                StopReason::DepthLimit,
            ] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = measured.usage().work,
                    StopReason::NodeLimit => limits.nodes = measured.usage().nodes,
                    StopReason::DepthLimit => limits.depth = measured.usage().depth,
                    StopReason::AllocationLimit => {
                        limits.allocation_units = measured.usage().allocation_units
                    }
                    _ => limits.source_bytes = measured.usage().source_bytes,
                }
                assert_eq!(run(&mut Budget::new(limits))?.map_err(err)?, result);
                match reason {
                    StopReason::WorkLimit => limits.work -= 1,
                    StopReason::NodeLimit => limits.nodes -= 1,
                    StopReason::DepthLimit => limits.depth -= 1,
                    StopReason::AllocationLimit => limits.allocation_units -= 1,
                    _ => {
                        assert!(limits.source_bytes > 0);
                        limits.source_bytes -= 1;
                    }
                }
                let mut stopped = Budget::new(limits);
                assert!(matches!(run(&mut stopped)?, Err(portable::PortableError::Stopped(actual)) if actual == reason));
                assert_eq!(stopped.poll(), Err(reason));
                assert!(matches!(run(&mut stopped)?, Err(portable::PortableError::Stopped(actual)) if actual == reason));
            }
            let absent = nepl3_core::schema::SchemaRegistry::default();
            let mut pre_stopped = budget();
            pre_stopped.cancel();
            assert!(matches!(operation::execute(&request, &absent, &mut codec, &mut pre_stopped), Err(portable::PortableError::Stopped(StopReason::Cancelled))));
            assert!(matches!(operation::verify_result(&result, &request, &absent, &mut codec, &mut pre_stopped), Err(portable::PortableError::Stopped(StopReason::Cancelled))));
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(run(&mut cancelled)?, Err(portable::PortableError::Stopped(StopReason::Cancelled))));
            assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
            let mut missing = request.clone();
            missing.syntax.sources.clear();
            let mut fresh = SourceAdmission::default();
            let mut fresh_codec = FoundationCodec::new(r, &empty, &mut fresh).map_err(err)?;
            assert!(operation::execute(&missing, r, &mut fresh_codec, &mut budget()).is_err());
            let mut stale = raw.clone();
            if let NdfValue::Record(record) = &mut stale {
                record.schema.digest = Digest::of(b"old schema");
            } else {
                return Err("request must be record".into());
            }
            assert!(operation::request_from_value(&stale, r, &mut codec, &mut budget()).is_err());
            // A malformed remote response is rejected before accepting its payload.
            let mut forged = result.clone();
            let NdfValue::Variant(value) = &mut forged else {
                return Err("result variant".into());
            };
            value.fields.clear();
            assert!(
                operation::verify_result(&forged, &request, r, &mut codec, &mut budget()).is_err()
            );
            Ok(())
        })
        .map_err(|e| format!("{source}: {e}"))?;
    }
    Ok(())
}

#[test]
fn evaluation_operation_rejects_non_expression_roots() -> Result<(), String> {
    let compiled = compiled()?;
    for (source, entry, category) in [
        ("row cons 1 cons x nil", "Row", check::Category::Row),
        (
            "Sentence \"[x/ex]\"",
            "SentenceGuest",
            check::Category::SentenceGuest,
        ),
    ] {
        with_input(&compiled, source, entry, |tree, profile, b, a| {
            let r = profile.registry();
            let checked = tree
                .tree()
                .bundle
                .validate_with_sources(r, b, a)
                .map_err(err)?;
            let syntax = lower::expression(
                &checked,
                &compiled.others[0].schema,
                category,
                r,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let request = MathEvaluationRequest {
                syntax,
                environment: BindingEnvironment {
                    assignments: vec![],
                },
            };
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            assert!(matches!(
                operation::execute(&request, r, &mut codec, &mut budget()),
                Err(portable::PortableError::Expression(_))
            ));
            let raw = NdfValue::Record(nepl3_core::value::Record {
                schema: r.selected("nepl3.math", 1).ok_or("Math schema")?.clone(),
                kind: "MathEvaluationRequest".into(),
                fields: vec![
                    portable::to_value(&request.syntax, r, &mut codec, &mut budget())
                        .map_err(err)?,
                    portable::environment_to_value(
                        &request.environment,
                        r,
                        &mut codec,
                        &mut budget(),
                    )
                    .map_err(err)?,
                ],
            });
            let mut fresh_admission = SourceAdmission::default();
            let mut fresh_codec =
                FoundationCodec::new(r, &empty, &mut fresh_admission).map_err(err)?;
            assert!(matches!(
                operation::execute_value(&raw, r, &mut fresh_codec, &mut budget()),
                Err(portable::PortableError::Expression(_))
            ));
            Ok(())
        })?;
    }
    Ok(())
}
