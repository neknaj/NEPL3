use super::*;
use nepl3_engine::portable::{PortableError, completion as wire};

#[test]
fn scope_candidate_transport_recomputes_success_and_rejects_forged_metadata() -> Result<(), String>
{
    let compiled = execution()?;
    for text in [
        "lambda a lambda b probe",
        "lambda あ probe",
        "recursive cons define a 1 cons define a 2 nil probe",
        "probe",
    ] {
        with_input(&compiled, text, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let prepared = keyed::prepare(
                "portable-candidates",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let sender = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let receiver = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let BindingOutcome::Complete(analysis) = &sender.reply().outcome else {
                return Err("complete sender".into());
            };
            let occurrence = analysis
                .facts()
                .occurrences
                .iter()
                .find(|o| o.role == OccurrenceRole::Reference && o.name == "probe")
                .ok_or("probe")?;
            let request = ScopeCandidateRequest {
                key: prepared.key(),
                occurrence: occurrence.id,
                prefix: "",
            };
            let request_value =
                wire::request_to_value(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&request_value, &mut budget()).map_err(err)?;
            let request_value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let received = wire::request_decode(
                &request_value,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(received.key, request.key);
            assert_eq!(received.occurrence, request.occurrence);
            assert_eq!(received.prefix, "");
            let reply = names(&sender, &request, &mut budget()).map_err(err)?;
            let packet = wire::reply_to_value(
                &reply,
                &request,
                &sender,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(err)?;
            let packet = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let decoded = wire::reply_decode(
                &packet,
                &received.as_request(),
                &receiver,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(decoded.key, reply.key);
            assert_eq!(decoded.occurrence, reply.occurrence);
            assert_eq!(decoded.namespace, reply.namespace);
            assert_eq!(decoded.report, reply.report);
            assert_eq!(
                decoded
                    .candidates
                    .iter()
                    .map(|c| (&c.name, &c.resolution))
                    .collect::<Vec<_>>(),
                reply
                    .candidates
                    .iter()
                    .map(|c| (&c.name, &c.resolution))
                    .collect::<Vec<_>>()
            );
            for mode in 0..9 {
                let mut forged = packet.clone();
                let NdfValue::Record(record) = &mut forged else {
                    return Err("record".into());
                };
                match mode {
                    0 => record.fields[1] = NdfValue::U64(u64::MAX),
                    1 => record.fields[2] = NdfValue::U64(u64::MAX),
                    7 => record.schema.digest = Digest::of(b"wrong schema"),
                    _ => {
                        let NdfValue::List(items) = &mut record.fields[3] else {
                            return Err("list".into());
                        };
                        if items.is_empty() {
                            continue;
                        }
                        match mode {
                            2 => {
                                items.pop();
                            }
                            3 => items.push(items[0].clone()),
                            4 => {
                                let NdfValue::Record(first) = &mut items[0] else {
                                    return Err("candidate".into());
                                };
                                first.fields[0] = NdfValue::Text("forged".into());
                            }
                            6 => {
                                let NdfValue::Record(first) = &mut items[0] else {
                                    return Err("candidate".into());
                                };
                                first.fields[1] = codec
                                    .encode_reference_resolution(
                                        &ReferenceResolution::Resolved(EntityId(u64::MAX)),
                                        &mut budget(),
                                    )
                                    .map_err(err)?;
                            }
                            8 => {
                                let Some(NameCandidate {
                                    resolution: ReferenceResolution::Ambiguous(ids),
                                    ..
                                }) = reply.candidates.first()
                                else {
                                    continue;
                                };
                                let mut ids = ids.clone();
                                ids.reverse();
                                let NdfValue::Record(first) = &mut items[0] else {
                                    return Err("candidate".into());
                                };
                                first.fields[1] = codec
                                    .encode_reference_resolution(
                                        &ReferenceResolution::Ambiguous(ids),
                                        &mut budget(),
                                    )
                                    .map_err(err)?;
                            }
                            _ => {
                                if items.len() < 2 {
                                    continue;
                                }
                                items.reverse();
                            }
                        }
                    }
                }
                assert!(
                    wire::reply_decode(
                        &forged,
                        &request,
                        &receiver,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .is_err(),
                    "{text}/{mode}"
                );
            }
            let mut semantic_forgery = names(&sender, &request, &mut budget()).map_err(err)?;
            if let Some(first) = semantic_forgery.candidates.first_mut() {
                first.name.push_str("forged");
            } else {
                semantic_forgery.namespace = NamespaceRef(u64::MAX);
            }
            assert!(matches!(
                wire::reply_to_value(
                    &semantic_forgery,
                    &request,
                    &sender,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::RequestMismatch)
            ));
            let mut malformed = request_value.clone();
            let NdfValue::Record(record) = &mut malformed else {
                return Err("request record".into());
            };
            record.fields[2] = NdfValue::U64(0);
            assert!(
                wire::request_decode(&malformed, profile.registry(), &mut codec, &mut budget())
                    .is_err()
            );
            for mode in 0..2 {
                let mut claimed = names(&sender, &request, &mut budget()).map_err(err)?;
                let schema = profile
                    .registry()
                    .selected("nepl3.engine", 1)
                    .ok_or("schema")?
                    .clone();
                if mode == 0 {
                    claimed.report.events.push(nepl3_core::diagnostic::Event {
                        schema,
                        kind: "unexpected".into(),
                        operation_path: vec![],
                        span: None,
                        payload: nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                            schema: profile
                                .registry()
                                .selected("nepl3.engine", 1)
                                .ok_or("schema")?
                                .clone(),
                            kind: "Unexpected".into(),
                            fields: vec![],
                        }),
                    });
                } else {
                    claimed
                        .report
                        .diagnostics
                        .push(nepl3_core::diagnostic::Diagnostic {
                            schema,
                            code: "unexpected".into(),
                            severity: nepl3_core::diagnostic::Severity::Error,
                            stage: "completion".into(),
                            arguments: nepl3_core::value::TypedValue::Record(
                                nepl3_core::value::Record {
                                    schema: profile
                                        .registry()
                                        .selected("nepl3.engine", 1)
                                        .ok_or("schema")?
                                        .clone(),
                                    kind: "Unexpected".into(),
                                    fields: vec![],
                                },
                            ),
                            primary: None,
                            related: vec![],
                            fixes: vec![],
                        });
                }
                assert!(matches!(
                    wire::reply_to_value(
                        &claimed,
                        &request,
                        &sender,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(PortableError::Shape)
                ));
            }
            let mut wrong_request = received;
            wrong_request.key.tree_digest = Digest::of(b"stale");
            assert!(
                wire::reply_decode(
                    &packet,
                    &wrong_request.as_request(),
                    &receiver,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                )
                .is_err()
            );
            let mut forged = names(&sender, &request, &mut budget()).map_err(err)?;
            forged.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert!(matches!(
                wire::reply_to_value(
                    &forged,
                    &request,
                    &sender,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::Shape)
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn scope_candidate_transport_rejects_incomplete_prefix_changes_and_budget_exhaustion()
-> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda あ probe", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let prepared = keyed::prepare(
            "codec-gates",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bound = prepared
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
            return Err("complete".into());
        };
        let occurrence = analysis
            .facts()
            .occurrences
            .iter()
            .find(|o| o.role == OccurrenceRole::Reference)
            .ok_or("reference")?;
        let request = ScopeCandidateRequest {
            key: prepared.key(),
            occurrence: occurrence.id,
            prefix: "あ",
        };
        let encoded =
            wire::request_to_value(&request, profile.registry(), &mut codec, &mut budget())
                .map_err(err)?;
        let decoded = wire::request_decode(&encoded, profile.registry(), &mut codec, &mut budget())
            .map_err(err)?;
        assert_eq!(decoded.prefix, "あ");
        let result = names(&bound, &request, &mut budget()).map_err(err)?;
        let packet = wire::reply_to_value(
            &result,
            &request,
            &bound,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let other_prefix = ScopeCandidateRequest {
            key: request.key,
            occurrence: request.occurrence,
            prefix: "い",
        };
        assert!(matches!(
            wire::reply_decode(
                &packet,
                &other_prefix,
                &bound,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::RequestMismatch)
        ));
        let mut cancelled = budget();
        cancelled.cancel();
        let stopped = prepared
            .execute(&mut cancelled, &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            wire::reply_decode(
                &packet,
                &request,
                &stopped,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::Candidate(CandidateError::Access(
                BindingAccessError::Incomplete
            )))
        ));
        let other = keyed::prepare(
            "other-analysis",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let other = other
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            wire::reply_decode(
                &packet,
                &request,
                &other,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::Candidate(CandidateError::Access(
                BindingAccessError::StaleAnalysis
            )))
        ));
        assert!(matches!(
            wire::reply_to_value(
                &result,
                &request,
                &other,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::Candidate(CandidateError::Access(
                BindingAccessError::StaleAnalysis
            )))
        ));
        for mode in 0..5 {
            let mut limits = budget().limits();
            let reason = match mode {
                0 => {
                    limits.work = 0;
                    StopReason::WorkLimit
                }
                1 => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                2 => {
                    limits.nodes = 0;
                    StopReason::NodeLimit
                }
                3 => {
                    limits.depth = 0;
                    StopReason::DepthLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut b = Budget::new(limits);
            if mode == 4 {
                b.cancel();
            }
            assert!(
                matches!(wire::reply_decode(&packet, &request, &bound, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(r)) if r == reason)
            );
        }
        for mode in 0..5 {
            let mut limits = budget().limits();
            let reason = match mode {
                0 => {
                    limits.work = 0;
                    StopReason::WorkLimit
                }
                1 => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                2 => {
                    limits.nodes = 0;
                    StopReason::NodeLimit
                }
                3 => {
                    limits.depth = 0;
                    StopReason::DepthLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut b = Budget::new(limits);
            if mode == 4 {
                b.cancel();
            }
            assert!(
                matches!(wire::reply_to_value(&result, &request, &bound, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(r)) if r == reason)
            );
        }
        let mut ambient_store = SourceStore::default();
        ambient_store
            .insert(
                SourceSnapshot::new(
                    nepl3_core::source::SourceId("unrelated".into()),
                    1,
                    "private:ambient".into(),
                    b"unrelated content".to_vec(),
                    &mut budget(),
                )
                .map_err(err)?,
            )
            .map_err(err)?;
        let mut ambient_admission = SourceAdmission::default();
        let mut ambient_codec =
            FoundationCodec::new(profile.registry(), &ambient_store, &mut ambient_admission)
                .map_err(err)?;
        let mut limits = budget().limits();
        limits.source_bytes = 0;
        let mut ambient_budget = Budget::new(limits);
        let ambient_packet = wire::reply_to_value(
            &result,
            &request,
            &bound,
            profile.registry(),
            &mut ambient_codec,
            &mut ambient_budget,
        )
        .map_err(err)?;
        wire::reply_decode(
            &ambient_packet,
            &request,
            &bound,
            profile.registry(),
            &mut ambient_codec,
            &mut ambient_budget,
        )
        .map_err(err)?;
        assert_eq!(ambient_budget.usage().source_bytes, 0);
        let mut claimed = names(&bound, &request, &mut budget()).map_err(err)?;
        claimed.report.usage = Default::default();
        let claimed_packet = wire::reply_to_value(
            &claimed,
            &request,
            &bound,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut receiver_cost = budget();
        wire::reply_decode(
            &claimed_packet,
            &request,
            &bound,
            profile.registry(),
            &mut codec,
            &mut receiver_cost,
        )
        .map_err(err)?;
        assert!(receiver_cost.usage().work > 0);
        let mut measured = budget();
        wire::reply_decode(
            &packet,
            &request,
            &bound,
            profile.registry(),
            &mut codec,
            &mut measured,
        )
        .map_err(err)?;
        assert!(measured.usage().work > result.report.usage.work);
        let mut limits = budget().limits();
        limits.work = measured.usage().work - 1;
        assert!(matches!(
            wire::reply_decode(
                &packet,
                &request,
                &bound,
                profile.registry(),
                &mut codec,
                &mut Budget::new(limits)
            ),
            Err(PortableError::Stopped(StopReason::WorkLimit))
        ));
        Ok(())
    })
}

#[path = "portable/error.rs"]
mod error;

#[path = "portable/failure.rs"]
mod failure;
