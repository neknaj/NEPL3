use super::*;
use nepl3_core::diagnostic::{Report, TraceOverflow};
use nepl3_engine::portable::completion::{
    DecodedScopeCandidateRequest,
    failure::{self as failure, ScopeCandidateFailureReply},
};

#[test]
fn candidate_failure_reply_binds_request_and_preserves_only_sender_metadata() -> Result<(), String>
{
    let compiled = execution()?;
    with_input(&compiled, "lambda あ probe", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "failure-reply",
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
        let request = ScopeCandidateRequest {
            key: prepared.key(),
            occurrence: OccurrenceId(u64::MAX),
            prefix: "あ",
        };
        let mut sender_budget = budget();
        let error = match names(&bound, &request, &mut sender_budget) {
            Ok(_) => return Err("missing occurrence accepted".into()),
            Err(error) => error,
        };
        let mut reply = ScopeCandidateFailureReply {
            request: DecodedScopeCandidateRequest {
                key: request.key,
                occurrence: request.occurrence,
                prefix: request.prefix.into(),
            },
            error,
            report: Report {
                usage: sender_budget.usage(),
                ..Report::default()
            },
        };
        for cause in [
            CandidateError::NoOccurrence,
            CandidateError::Access(BindingAccessError::Incomplete),
            CandidateError::Stopped(StopReason::Cancelled),
            CandidateError::Binding(BindingError::Source(
                nepl3_core::source::SourceError::Stopped(StopReason::SourceLimit),
            )),
        ] {
            reply.error = cause;
            let packet = failure::to_value(
                &reply,
                &request,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(err)?;
            let packet = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut received_budget = budget();
            received_budget.charge(Resource::Work, 1234).map_err(err)?;
            let decoded = failure::from_value(
                &packet,
                &request,
                profile.registry(),
                &mut codec,
                &mut received_budget,
            )
            .map_err(err)?;
            assert_eq!(decoded.request, reply.request);
            assert_eq!(decoded.error, reply.error);
            assert_eq!(decoded.report, reply.report);
            received_budget.charge(Resource::Work, 1).map_err(err)?;
            assert!(received_budget.usage().work > 1235);
            assert_eq!(received_budget.usage().source_bytes, 0);
            for mode in 0..6 {
                let mut wrong = decoded.request.clone();
                let changed = Digest::of(b"wrong request");
                match mode {
                    0 => wrong.key.tree_digest = changed,
                    1 => wrong.key.profile_digest = changed,
                    2 => wrong.key.execution_digest = changed,
                    3 => wrong.key.request_digest = changed,
                    4 => wrong.occurrence = OccurrenceId(0),
                    _ => wrong.prefix = "い".into(),
                }
                assert!(matches!(
                    failure::from_value(
                        &packet,
                        &wrong.as_request(),
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(PortableError::RequestMismatch)
                ));
                assert!(matches!(
                    failure::to_value(
                        &reply,
                        &wrong.as_request(),
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(PortableError::RequestMismatch)
                ));
            }
            for mode in 0..5 {
                let mut limits = budget().limits();
                match mode {
                    0 => limits.work = 0,
                    1 => limits.nodes = 0,
                    2 => limits.allocation_units = 0,
                    3 => limits.depth = 0,
                    _ => {}
                }
                let expected = [
                    StopReason::WorkLimit,
                    StopReason::NodeLimit,
                    StopReason::AllocationLimit,
                    StopReason::DepthLimit,
                    StopReason::Cancelled,
                ][mode];
                let mut b = Budget::new(limits);
                if mode == 4 {
                    b.cancel();
                }
                assert!(
                    matches!(failure::to_value(&reply, &request, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(reason)) if reason == expected)
                );
                let mut b = Budget::new(limits);
                if mode == 4 {
                    b.cancel();
                }
                assert!(
                    matches!(failure::from_value(&packet, &request, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(reason)) if reason == expected)
                );
            }
            let mut malformed = packet.clone();
            let NdfValue::Record(record) = &mut malformed else {
                return Err("record".into());
            };
            record.fields[1] = NdfValue::U64(0);
            assert!(
                failure::from_value(
                    &malformed,
                    &request,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                )
                .is_err()
            );
        }
        let mut polluted = failure::to_value(
            &reply,
            &request,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let NdfValue::Record(record) = &mut polluted else {
            return Err("reply record".into());
        };
        let report = Report {
            trace_overflow: Some(TraceOverflow { dropped: 1 }),
            ..Report::default()
        };
        record.fields[2] = codec.encode_report(&report, &mut budget()).map_err(err)?;
        assert!(matches!(
            failure::from_value(
                &polluted,
                &request,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::Shape)
        ));
        for kind in 0..2 {
            let mut report = Report::default();
            let schema = profile
                .registry()
                .selected("nepl3.engine", 1)
                .ok_or("engine schema")?
                .clone();
            let payload = nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                schema: schema.clone(),
                kind: "BindingOptions".into(),
                fields: vec![],
            });
            if kind == 0 {
                report.events.push(nepl3_core::diagnostic::Event {
                    schema,
                    kind: "unexpected".into(),
                    operation_path: vec![],
                    span: None,
                    payload,
                });
                report.usage.events = 1;
            } else {
                report.diagnostics.push(nepl3_core::diagnostic::Diagnostic {
                    schema,
                    code: "unexpected".into(),
                    severity: nepl3_core::diagnostic::Severity::Error,
                    stage: "completion".into(),
                    arguments: payload,
                    primary: None,
                    related: vec![],
                    fixes: vec![],
                });
                report.usage.diagnostics = 1;
            }
            report
                .validate(&empty, &[], profile.registry(), &mut budget())
                .map_err(err)?;
            let mut packet = failure::to_value(
                &reply,
                &request,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let NdfValue::Record(record) = &mut packet else {
                return Err("failure record".into());
            };
            record.fields[2] = codec.encode_report(&report, &mut budget()).map_err(err)?;
            assert!(matches!(
                failure::from_value(
                    &packet,
                    &request,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::Shape)
            ));
        }
        reply.report.usage.work = u64::MAX;
        reply.report.usage.source_bytes = u64::MAX;
        let packet = failure::to_value(
            &reply,
            &request,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut b = budget();
        let decoded =
            failure::from_value(&packet, &request, profile.registry(), &mut codec, &mut b)
                .map_err(err)?;
        assert_eq!(decoded.report.usage.work, u64::MAX);
        assert_eq!(decoded.report.usage.source_bytes, u64::MAX);
        assert_eq!(b.usage().source_bytes, 0);
        b.charge(Resource::Work, 1).map_err(err)?;
        reply.report.trace_overflow = Some(TraceOverflow { dropped: 1 });
        assert!(matches!(
            failure::to_value(
                &reply,
                &request,
                profile.registry(),
                &mut codec,
                &mut budget()
            ),
            Err(PortableError::Shape)
        ));
        reply.report = Report::default();
        let mut ambient = SourceStore::default();
        ambient
            .insert(
                SourceSnapshot::new(
                    nepl3_core::source::SourceId("unrelated-failure-source".into()),
                    1,
                    "private:ambient".into(),
                    b"unrelated".to_vec(),
                    &mut budget(),
                )
                .map_err(err)?,
            )
            .map_err(err)?;
        let mut admission = SourceAdmission::default();
        let mut local =
            FoundationCodec::new(profile.registry(), &ambient, &mut admission).map_err(err)?;
        let mut limits = budget().limits();
        limits.source_bytes = 0;
        let mut b = Budget::new(limits);
        let packet = failure::to_value(&reply, &request, profile.registry(), &mut local, &mut b)
            .map_err(err)?;
        let decoded =
            failure::from_value(&packet, &request, profile.registry(), &mut local, &mut b)
                .map_err(err)?;
        assert_eq!(decoded.report.usage.work, 0);
        assert!(b.usage().work > 0);
        assert_eq!(b.usage().source_bytes, 0);
        Ok(())
    })
}
