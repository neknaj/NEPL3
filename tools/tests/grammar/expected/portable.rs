use super::*;
use nepl3_core::value::NdfValue;
use nepl3_engine::portable::expected;

#[test]
fn first_receiver_rebuilds_tree_and_recomputes_expected_read() -> Result<(), String> {
    for text in ["let", "let x", "guest guest lambda", "x", ""] {
        with_input(
            text,
            |_| Ok(()),
            |input, source, profile| {
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: text.len() as u64,
                };
                let reply = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                assert!(matches!(reply.outcome, ExpectedReadOutcome::Complete(_)));
                let tree = analysis::request_to_value(input, &mut c, &mut budget()).map_err(err)?;
                let query =
                    expected::request_to_value(&request, profile.registry(), &mut c, &mut budget())
                        .map_err(err)?;
                let result =
                    expected::reply_to_value(&reply, &request, input, &mut c, &mut budget())
                        .map_err(err)?;
                let bytes =
                    nepl3_wire::encode(&NdfValue::List(vec![tree, query, result]), &mut budget())
                        .map_err(err)?;
                let NdfValue::List(ref packet) =
                    nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?
                else {
                    return Err("packet".into());
                };
                let receiver_empty = SourceStore::default();
                let mut received_admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(
                    profile.registry(),
                    &receiver_empty,
                    &mut received_admission,
                )
                .map_err(err)?;
                let tree = analysis::request_decode(&packet[0], profile, &mut c, &mut budget())
                    .map_err(err)?;
                let received = analysis::prepare_received(&tree, profile, &mut c, &mut budget())
                    .map_err(err)?;
                let query =
                    expected::request_decode(&packet[1], profile.registry(), &mut c, &mut budget())
                        .map_err(err)?;
                assert_eq!(
                    expected::reply_decode(&packet[2], &query, &received, &mut c, &mut budget())
                        .map_err(err)?,
                    reply
                );

                if let ExpectedReadOutcome::Complete(Some(_)) = &reply.outcome {
                    for which in 0..11 {
                        let mut forged = packet[2].clone();
                        corrupt_complete(&mut forged, which)?;
                        // Exercise an actual receiver packet, bypassing sender validation.
                        assert!(
                            expected::reply_decode(
                                &forged,
                                &query,
                                &received,
                                &mut c,
                                &mut budget()
                            )
                            .is_err(),
                            "accepted forgery {which} for {text}"
                        );
                    }
                    let mut forged = reply.clone();
                    forged.outcome = ExpectedReadOutcome::Complete(None);
                    assert!(
                        expected::reply_to_value(&forged, &query, &received, &mut c, &mut budget())
                            .is_err()
                    );
                    for which in 0..5 {
                        let mut forged = reply.clone();
                        let ExpectedReadOutcome::Complete(Some(value)) = &mut forged.outcome else {
                            return Err("some".into());
                        };
                        match which {
                            0 => value.node += 1,
                            1 => value.bundle += 1,
                            2 => value.path.push(ExpectedReadStep::Child { field: 99 }),
                            3 => value.expected.mode.push('x'),
                            _ => {
                                if let ExpectedReadOrigin::Field { field, .. } = &mut value.origin {
                                    *field += 1;
                                } else {
                                    value.path.push(ExpectedReadStep::Foreign { field: 0 });
                                }
                            }
                        }
                        assert!(
                            expected::reply_to_value(
                                &forged,
                                &query,
                                &received,
                                &mut c,
                                &mut budget()
                            )
                            .is_err()
                        );
                    }
                }
                let mut missing_sources = reply.clone();
                missing_sources.sources.clear();
                assert!(
                    expected::reply_to_value(
                        &missing_sources,
                        &query,
                        &received,
                        &mut c,
                        &mut budget()
                    )
                    .is_err()
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn record_fields(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match value {
        NdfValue::Record(value) => Ok(&mut value.fields),
        _ => Err("record".into()),
    }
}

#[test]
fn failure_transport_preserves_typed_reports_and_rejects_partial_results() -> Result<(), String> {
    with_input(
        "あ",
        |_| Ok(()),
        |input, source, profile| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let request = ExpectedReadRequest {
                key: input.key(),
                source: source.reference(),
                offset: 1,
            };
            let invalid = expected_read(
                input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert_eq!(
                invalid.outcome,
                ExpectedReadOutcome::Invalid(ExpectedReadError::Source(
                    SourceError::ScalarBoundary
                ))
            );
            let mut cancelled = budget();
            cancelled.cancel();
            let stopped = expected_read(
                input,
                &request,
                &mut cancelled,
                &mut SourceAdmission::default(),
            );
            for reply in [invalid, stopped] {
                let value =
                    expected::reply_to_value(&reply, &request, input, &mut c, &mut budget())
                        .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
                let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                assert_eq!(
                    expected::reply_decode(&value, &request, input, &mut c, &mut budget())
                        .map_err(err)?,
                    reply
                );
                let mut leaked = reply.clone();
                leaked.sources.push(source.clone());
                assert!(
                    expected::reply_to_value(&leaked, &request, input, &mut c, &mut budget())
                        .is_err()
                );
                let mut hidden_stop = reply.clone();
                hidden_stop.outcome = ExpectedReadOutcome::Invalid(ExpectedReadError::Access(
                    nepl3_engine::analysis::BindingAccessError::Stopped(StopReason::Cancelled),
                ));
                assert!(
                    expected::reply_to_value(&hidden_stop, &request, input, &mut c, &mut budget())
                        .is_err()
                );
                let mut zero = Budget::new(Limits {
                    work: 0,
                    ..budget().limits()
                });
                assert!(matches!(
                    expected::reply_decode(&value, &request, input, &mut c, &mut zero),
                    Err(nepl3_engine::portable::PortableError::Stopped(
                        StopReason::WorkLimit
                    ))
                ));
            }
            Ok(())
        },
    )
}
fn corrupt_complete(value: &mut NdfValue, which: usize) -> Result<(), String> {
    let reply = record_fields(value)?;
    if which == 0 {
        reply.pop();
        return Ok(());
    }
    if which == 1 {
        reply[3] = NdfValue::List(Vec::new());
        return Ok(());
    }
    if which == 2 {
        reply[0] = NdfValue::Unit;
        return Ok(());
    }
    let NdfValue::Variant(outcome) = &mut reply[1] else {
        return Err("outcome".into());
    };
    if which == 3 {
        outcome.fields[0] = NdfValue::None;
        return Ok(());
    }
    let NdfValue::Some(selected) = &mut outcome.fields[0] else {
        return Err("selected".into());
    };
    let read = record_fields(selected)?;
    match which {
        4 => read[0] = NdfValue::U64(999),
        5 => read[1] = NdfValue::U64(999),
        6 => read[2] = NdfValue::List(vec![NdfValue::Unit]),
        7 => record_fields(&mut read[3])?[1] = NdfValue::Text("other".into()),
        _ => {
            let NdfValue::Variant(origin) = &mut read[4] else {
                return Err("origin".into());
            };
            if origin.variant == "Root" {
                origin.fields.push(NdfValue::Unit);
                return Ok(());
            }
            match which {
                8 => record_fields(&mut origin.fields[4])?[0] = NdfValue::U64(999),
                9 => origin.fields[5] = NdfValue::Some(Box::new(NdfValue::Unit)),
                _ => {
                    let NdfValue::Bool(value) = &mut origin.fields[6] else {
                        return Err("foreign".into());
                    };
                    *value = !*value;
                }
            }
        }
    }
    Ok(())
}
