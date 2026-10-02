use super::*;
use nepl3_core::diagnostic::{Report, TraceOverflow};
use nepl3_engine::portable::PortableError;
use nepl3_engine::{
    analysis::{
        query::QueryError,
        region::{RegionCapability, RegionError, query::RegionQueryError},
    },
    portable::region::completion::failure::{self as failure, RegionCompletionFailureReply},
};

#[test]
fn region_failure_reply_correlates_request_and_preserves_nested_stop_metadata() -> Result<(), String>
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
        let input =
            wire_region::prepare(&prepared, None, &mut codec, &mut budget()).map_err(err)?;
        let capability = input.capability();
        let request = RegionCompletionRequest {
            region: RegionRequest {
                key: input.key(),
                source: tree.tree().bundle.sources[0].reference(),
                offset: u64::MAX,
            },
            prefix: "あ".into(),
        };
        let native = selected::names(
            &input,
            &bound,
            &request,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let error = match native.outcome {
            RegionCompletionOutcome::Invalid(error) => error,
            _ => return Err("invalid offset accepted".into()),
        };
        let mut reply = RegionCompletionFailureReply {
            request: request.clone(),
            capability,
            error,
            report: native.report,
        };
        let mut causes = vec![
            core::mem::replace(
                &mut reply.error,
                RegionCompletionError::Candidates(CandidateError::NoStage),
            ),
            RegionCompletionError::Selection(RegionQueryError::Region(RegionError::Mapping)),
            RegionCompletionError::Selection(RegionQueryError::Region(RegionError::Access(
                BindingAccessError::Stopped(StopReason::Cancelled),
            ))),
            RegionCompletionError::Selection(RegionQueryError::Region(RegionError::Source(
                nepl3_core::source::SourceError::Stopped(StopReason::SourceLimit),
            ))),
            RegionCompletionError::Selection(RegionQueryError::Query(QueryError::Access(
                BindingAccessError::Stopped(StopReason::WorkLimit),
            ))),
            RegionCompletionError::Selection(RegionQueryError::Query(QueryError::Source(
                nepl3_core::source::SourceError::Stopped(StopReason::SourceLimit),
            ))),
            RegionCompletionError::Selection(RegionQueryError::Query(QueryError::Facts)),
            RegionCompletionError::Candidates(CandidateError::NoOccurrence),
            RegionCompletionError::Candidates(CandidateError::Access(
                BindingAccessError::Incomplete,
            )),
            RegionCompletionError::Candidates(CandidateError::Access(BindingAccessError::Stopped(
                StopReason::WorkLimit,
            ))),
            RegionCompletionError::Candidates(CandidateError::Stopped(StopReason::Cancelled)),
            RegionCompletionError::Candidates(CandidateError::Binding(BindingError::Source(
                nepl3_core::source::SourceError::Stopped(StopReason::SourceLimit),
            ))),
        ];
        for reason in [
            StopReason::Cancelled,
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ] {
            causes.push(RegionCompletionError::Candidates(CandidateError::Stopped(
                reason,
            )));
            causes.push(RegionCompletionError::Candidates(CandidateError::Access(
                BindingAccessError::Stopped(reason),
            )));
            causes.push(RegionCompletionError::Candidates(CandidateError::Binding(
                BindingError::Stopped(reason),
            )));
            causes.push(RegionCompletionError::Candidates(CandidateError::Binding(
                BindingError::Source(nepl3_core::source::SourceError::Stopped(reason)),
            )));
            causes.push(RegionCompletionError::Selection(RegionQueryError::Region(
                RegionError::Access(BindingAccessError::Stopped(reason)),
            )));
            causes.push(RegionCompletionError::Selection(RegionQueryError::Region(
                RegionError::Source(nepl3_core::source::SourceError::Stopped(reason)),
            )));
            causes.push(RegionCompletionError::Selection(RegionQueryError::Query(
                QueryError::Access(BindingAccessError::Stopped(reason)),
            )));
            causes.push(RegionCompletionError::Selection(RegionQueryError::Query(
                QueryError::Source(nepl3_core::source::SourceError::Stopped(reason)),
            )));
        }
        causes.push(RegionCompletionError::Candidates(CandidateError::Binding(
            BindingError::Facts(nepl3_engine::facts::FactsError::Report(
                nepl3_core::diagnostic::validation::ReportValidationError::Source(
                    nepl3_core::source::SourceError::Stopped(StopReason::EventLimit),
                ),
            )),
        )));
        causes.push(RegionCompletionError::Candidates(CandidateError::Binding(
            BindingError::Tree(nepl3_engine::tree::TreeError::Syntax(
                nepl3_core::syntax::SyntaxError::View(nepl3_core::view::ViewError::Origin(
                    nepl3_core::origin::OriginError::Source(
                        nepl3_core::source::SourceError::Stopped(StopReason::DepthLimit),
                    ),
                )),
            )),
        )));
        causes.push(RegionCompletionError::Selection(RegionQueryError::Region(
            RegionError::Source(nepl3_core::source::SourceError::Decode {
                valid_up_to: 17,
                error_len: Some(3),
            }),
        )));
        for cause in causes {
            reply.error = cause;
            let packet = failure::to_value(
                &reply,
                &request,
                capability,
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
                capability,
                profile.registry(),
                &mut codec,
                &mut received_budget,
            )
            .map_err(err)?;
            assert_eq!(decoded.request, reply.request);
            assert_eq!(decoded.error, reply.error);
            assert_eq!(decoded.report, reply.report);
            assert_eq!(decoded.capability, reply.capability);
            assert!(matches!(
                failure::to_value(
                    &reply,
                    &request,
                    RegionCapability::ReaderFacts,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::RequestMismatch)
            ));
            assert!(matches!(
                failure::from_value(
                    &packet,
                    &request,
                    RegionCapability::ReaderFacts,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::RequestMismatch)
            ));

            received_budget.charge(Resource::Work, 1).map_err(err)?;
            assert!(received_budget.usage().work > 1235);
            assert_eq!(received_budget.usage().source_bytes, 0);
            for mode in 0..10 {
                let mut wrong = decoded.request.clone();
                let changed = Digest::of(b"wrong request");
                match mode {
                    0 => wrong.region.key.analysis.tree_digest = changed,
                    1 => wrong.region.key.analysis.profile_digest = changed,
                    2 => wrong.region.key.analysis.execution_digest = changed,
                    3 => wrong.region.key.analysis.request_digest = changed,
                    4 => wrong.region.key.reader_facts_digest = changed,
                    5 => wrong.region.source.source_id.0.push('x'),
                    6 => wrong.region.source.revision += 1,
                    7 => wrong.region.source.digest = changed,
                    8 => wrong.region.offset = 0,
                    _ => wrong.prefix = "い".into(),
                }
                assert!(matches!(
                    failure::from_value(
                        &packet,
                        &wrong,
                        capability,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    ),
                    Err(PortableError::RequestMismatch)
                ));
                assert!(matches!(
                    failure::to_value(
                        &reply,
                        &wrong,
                        capability,
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
                    matches!(failure::to_value(&reply, &request, capability, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(reason)) if reason == expected)
                );
                let mut b = Budget::new(limits);
                if mode == 4 {
                    b.cancel();
                }
                assert!(
                    matches!(failure::from_value(&packet, &request, capability, profile.registry(), &mut codec, &mut b), Err(PortableError::Stopped(reason)) if reason == expected)
                );
            }
            for mode in 0..3 {
                let mut malformed = packet.clone();
                let NdfValue::Record(record) = &mut malformed else {
                    return Err("record".into());
                };
                match mode {
                    0 => record.schema.digest = Digest::of(b"wrong failure schema"),
                    1 => record.kind = "RegionCandidates".into(),
                    _ => {
                        record.fields.pop();
                    }
                }
                assert!(
                    failure::from_value(
                        &malformed,
                        &request,
                        capability,
                        profile.registry(),
                        &mut codec,
                        &mut budget()
                    )
                    .is_err()
                );
            }
            let mut malformed = packet.clone();
            let NdfValue::Record(record) = &mut malformed else {
                return Err("record".into());
            };
            record.fields[2] = NdfValue::U64(0);
            assert!(
                failure::from_value(
                    &malformed,
                    &request,
                    capability,
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
            capability,
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
        record.fields[3] = codec.encode_report(&report, &mut budget()).map_err(err)?;
        assert!(matches!(
            failure::from_value(
                &polluted,
                &request,
                capability,
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
                capability,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let NdfValue::Record(record) = &mut packet else {
                return Err("failure record".into());
            };
            record.fields[3] = codec.encode_report(&report, &mut budget()).map_err(err)?;
            let original = core::mem::replace(&mut reply.report, report);
            assert!(matches!(
                failure::to_value(
                    &reply,
                    &request,
                    capability,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::Shape)
            ));
            reply.report = original;

            assert!(matches!(
                failure::from_value(
                    &packet,
                    &request,
                    capability,
                    profile.registry(),
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::Shape)
            ));
        }
        reply.report.usage = nepl3_core::budget::Usage {
            work: u64::MAX,
            source_bytes: u64::MAX,
            depth: u64::MAX,
            nodes: u64::MAX,
            allocation_units: u64::MAX,
            output_bytes: u64::MAX,
            diagnostics: u64::MAX,
            events: u64::MAX,
        };
        let packet = failure::to_value(
            &reply,
            &request,
            capability,
            profile.registry(),
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let mut b = budget();
        let decoded = failure::from_value(
            &packet,
            &request,
            capability,
            profile.registry(),
            &mut codec,
            &mut b,
        )
        .map_err(err)?;
        assert_eq!(decoded.report.usage, reply.report.usage);
        assert_eq!(decoded.report.usage.source_bytes, u64::MAX);
        assert_eq!(b.usage().source_bytes, 0);
        b.charge(Resource::Work, 1).map_err(err)?;
        reply.report.trace_overflow = Some(TraceOverflow { dropped: 1 });
        assert!(matches!(
            failure::to_value(
                &reply,
                &request,
                capability,
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
        let packet = failure::to_value(
            &reply,
            &request,
            capability,
            profile.registry(),
            &mut local,
            &mut b,
        )
        .map_err(err)?;
        let decoded = failure::from_value(
            &packet,
            &request,
            capability,
            profile.registry(),
            &mut local,
            &mut b,
        )
        .map_err(err)?;
        assert_eq!(decoded.report.usage.work, 0);
        assert!(b.usage().work > 0);
        assert_eq!(b.usage().source_bytes, 0);
        Ok(())
    })
}
