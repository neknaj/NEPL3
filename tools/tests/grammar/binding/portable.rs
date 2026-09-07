use super::*;
use nepl3_engine::portable::binding::{self as wire_binding, DecodedBindingOutcome};
#[path = "portable/causes.rs"]
mod causes;

#[test]
fn binding_result_first_receiver_preserves_visibility_reports_and_foreign_closure()
-> Result<(), String> {
    let compiled = execution()?;
    for input in [
        "let x x x",
        "lambda x apply guest x x",
        "recursive cons define a b cons define b a nil a",
        "lettext \"\\u{78}\" x",
    ] {
        with_input(&compiled, input, |tree, profile, b, a| {
            let reply = analyze("portable-analysis", tree, profile, b, a);
            let BindingOutcome::Complete(analysis) = &reply.outcome else {
                return Err(format!("{input}: {:?}", reply.outcome));
            };
            let expected = analysis.facts().clone();
            let report = reply.report.clone();
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let value = wire_binding::reply_to_value(&reply, profile.registry(), &mut codec, b)
                .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, b).map_err(err)?;
            drop(reply);
            let mut receiving = budget();
            let mut fresh = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let received = nepl3_wire::decode(&bytes, &mut receiving).map_err(err)?;
            let decoded = wire_binding::reply_from_value(
                &received,
                profile.registry(),
                &mut receiver,
                &mut receiving,
            )
            .map_err(err)?;
            let DecodedBindingOutcome::Complete(result) = &decoded.outcome else {
                return Err("complete".into());
            };
            assert_eq!(result.facts, expected);
            assert_eq!(decoded.report, report);
            assert_eq!(
                result.occurrence_stages.len(),
                result.facts.occurrences.len()
            );
            assert_eq!(
                wire_binding::decoded_to_value(
                    &decoded,
                    profile.registry(),
                    &mut receiver,
                    &mut receiving
                )
                .map_err(err)?,
                value
            );
            // Sources shared by the FactSet and Report table are admitted once.
            let mut ids = std::collections::BTreeSet::new();
            let expected_bytes = result
                .facts
                .sources
                .iter()
                .chain(&result.sources)
                .filter(|s| ids.insert((s.identity().source.0.clone(), s.identity().revision)))
                .map(|s| s.text().len() as u64)
                .sum::<u64>();
            assert_eq!(receiving.usage().source_bytes, expected_bytes);
            // This raw result is not a constructor for BindingAnalysis's proof.
            for index in 0..3 {
                let mut changed = value.clone();
                let NdfValue::Record(outer) = &mut changed else {
                    return Err("reply".into());
                };
                let NdfValue::Variant(outcome) = &mut outer.fields[0] else {
                    return Err("outcome".into());
                };
                let NdfValue::Record(result) = &mut outcome.fields[0] else {
                    return Err("analysis".into());
                };
                match index {
                    0 => {
                        let NdfValue::List(stages) = &mut result.fields[3] else {
                            return Err("stages".into());
                        };
                        let NdfValue::Record(stage) = &mut stages[0] else {
                            return Err("stage".into());
                        };
                        let mut id = stage.fields[0].clone();
                        let NdfValue::Record(id) = &mut id else {
                            return Err("id".into());
                        };
                        id.schema = profile
                            .registry()
                            .selected("nepl3.engine", 1)
                            .ok_or("engine")?
                            .clone();
                        id.kind = "StageId".into();
                        id.fields[0] = NdfValue::U64(0);
                        stage.fields[1] = NdfValue::Some(Box::new(NdfValue::Record(id.clone())));
                    }
                    1 => result.fields[4] = NdfValue::List(vec![]),
                    _ => {
                        let NdfValue::Record(facts) = &mut result.fields[0] else {
                            return Err("facts".into());
                        };
                        // Remove both independent SourceContent declaration tables.
                        facts.fields[7] = NdfValue::List(vec![]);
                        result.fields[1] = NdfValue::List(vec![]);
                    }
                }
                profile
                    .registry()
                    .validate(
                        &nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
                            package: "nepl3.engine".into(),
                            revision: 1,
                            name: "BindingReply".into(),
                        }),
                        &changed,
                        &mut receiving,
                    )
                    .map_err(err)?;
                assert!(
                    wire_binding::reply_from_value(
                        &changed,
                        profile.registry(),
                        &mut receiver,
                        &mut receiving
                    )
                    .is_err()
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn binding_invalid_and_stopped_envelopes_preserve_typed_causes_and_partial_fact_invariants()
-> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lettext \"\\u{78}\" x", |tree, profile, b, _| {
        for mode in 0..6 {
            let mut limits = b.limits();
            if mode == 1 {
                limits.allocation_units = 0;
            } else if mode == 2 {
                limits.source_bytes = 0;
            } else if mode == 3 {
                limits.work = 51;
            } else if mode == 4 {
                limits.nodes = 11;
            }
            let mut operation = Budget::new(limits);
            if mode == 5 {
                operation.cancel();
            }
            let reply = analyze(
                if mode == 0 { "" } else { "partial" },
                tree,
                profile,
                &mut operation,
                &mut SourceAdmission::default(),
            );
            if mode == 0 {
                assert!(matches!(
                    reply.outcome,
                    BindingOutcome::Invalid {
                        error: BindingError::AnalysisId,
                        ..
                    }
                ));
            } else {
                assert!(matches!(reply.outcome, BindingOutcome::Stopped { .. }));
            }
            let mut transport = budget();
            let mut a = SourceAdmission::default();
            let empty = SourceStore::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let value = wire_binding::reply_to_value(
                &reply,
                profile.registry(),
                &mut codec,
                &mut transport,
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut transport).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut transport).map_err(err)?;
            if mode == 0 {
                let mut invalid = value.clone();
                let NdfValue::Record(outer) = &mut invalid else {
                    return Err("reply".into());
                };
                let NdfValue::Variant(outcome) = &mut outer.fields[0] else {
                    return Err("outcome".into());
                };
                outcome.fields[0] = wire_binding::failure_to_value(
                    &BindingError::Tree(nepl3_engine::tree::TreeError::Syntax(
                        nepl3_core::syntax::SyntaxError::View(nepl3_core::view::ViewError::Schema(
                            nepl3_core::schema::SchemaError::Stopped(StopReason::NodeLimit),
                        )),
                    )),
                    profile.registry(),
                    &mut transport,
                )
                .map_err(err)?;
                // The cause is valid typed data, but an Invalid operation cannot
                // hide its nested stop reason.
                profile
                    .registry()
                    .validate(
                        &nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
                            package: "nepl3.engine".into(),
                            revision: 1,
                            name: "BindingReply".into(),
                        }),
                        &invalid,
                        &mut transport,
                    )
                    .map_err(err)?;
                assert!(
                    wire_binding::reply_from_value(
                        &invalid,
                        profile.registry(),
                        &mut codec,
                        &mut transport
                    )
                    .is_err()
                );
            }
            let decoded = wire_binding::reply_from_value(
                &value,
                profile.registry(),
                &mut codec,
                &mut transport,
            )
            .map_err(err)?;
            assert_eq!(decoded.report, reply.report);
            match (&reply.outcome, &decoded.outcome) {
                (
                    BindingOutcome::Invalid { error, .. },
                    DecodedBindingOutcome::Invalid { failure, .. },
                ) => assert_eq!(error, failure),
                (
                    BindingOutcome::Stopped { reason, .. },
                    DecodedBindingOutcome::Stopped {
                        reason: received, ..
                    },
                ) => assert_eq!(reason, received),
                _ => return Err("outcome changed".into()),
            }
            for failure in 0..5 {
                let mut limits = budget().limits();
                let reason = match failure {
                    0 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.depth = 0;
                        StopReason::DepthLimit
                    }
                    3 => {
                        limits.nodes = 0;
                        StopReason::NodeLimit
                    }
                    _ => StopReason::Cancelled,
                };
                let mut low = Budget::new(limits);
                if failure == 4 {
                    low.cancel();
                }
                assert!(
                    matches!(wire_binding::reply_from_value(&value,profile.registry(),&mut codec,&mut low),Err(nepl3_engine::portable::PortableError::Stopped(r)) if r==reason)
                );
                assert_eq!(low.poll(), Err(reason));
            }
        }
        Ok(())
    })
}
