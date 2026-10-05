use super::*;
use nepl3_core::diagnostic::*;

fn fixture(output: &str) -> Result<(SchemaRegistry, Invoke, SourceStore, ProviderFrame), String> {
    let (mut registry, mut request) = setup()?;
    let descriptor = SchemaDescriptor {
        package: "example.pending".into(),
        revision: 1,
        types: vec![],
        operations: vec![OperationDescriptor {
            name: "produce".into(),
            input: TypeDescriptor::TypedValue,
            output: TypeDescriptor::Named(TypeRef {
                package: "nepl3.foundation".into(),
                revision: 1,
                name: output.into(),
            }),
            pure: true,
        }],
    };
    let identity = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(identity.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let diagnostic_schema = request.operation.schema.clone();
    request.operation = OperationRef {
        schema: identity,
        name: "produce".into(),
    };
    let source = SourceSnapshot::new(
        SourceId("generated".into()),
        1,
        "memory:generated".into(),
        "生成".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let span = source.span(0, 6).map_err(error)?;
    let mut sources = SourceStore::default();
    sources.insert(source).map_err(error)?;
    let frame = ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Result(OperationResult::Complete {
            value: request.input.clone(),
            report: Report {
                diagnostics: vec![Diagnostic {
                    schema: diagnostic_schema,
                    code: "generated".into(),
                    severity: Severity::Warning,
                    stage: "test".into(),
                    arguments: request.input.clone(),
                    primary: Some(span),
                    related: vec![],
                    fixes: vec![],
                }],
                events: vec![],
                usage: Usage {
                    diagnostics: 1,
                    ..Usage::default()
                },
                trace_overflow: None,
            },
        }),
    };
    Ok((registry, request, sources, frame))
}

fn bytes(
    registry: &SchemaRegistry,
    sources: &SourceStore,
    frame: &ProviderFrame,
) -> Result<Vec<u8>, String> {
    encode_frame(
        frame,
        registry,
        sources,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)
}

#[test]
fn staged_reply_defers_report_sources_and_preserves_frame_suffix() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let mut encoded = bytes(&registry, &sources, &frame)?;
    encoded.extend_from_slice(b"next frame");
    let original = SourceStore::default();
    assert!(
        decode_frame(
            &encoded,
            true,
            &registry,
            &original,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let (pending, rest) =
        decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("missing receipt")?;
    assert_eq!(rest, b"next frame");
    assert_eq!(pending.request_id(), request.request_id);
    let mut calls = 0;
    let decoded = pending
        .finish_with(&request, &original, |payload, _, _, _, _| {
            calls += 1;
            assert_eq!(payload.kind, ReplyPayloadKind::Complete);
            Ok::<_, ()>(sources)
        })
        .map_err(error)?;
    assert_eq!(calls, 1);
    assert_eq!(decoded, frame);
    Ok(())
}

#[test]
fn staged_reply_checks_id_and_selected_output_before_policy() -> Result<(), String> {
    for wrong_id in [true, false] {
        let (registry, mut request, sources, frame) =
            fixture(if wrong_id { "TraceOverflow" } else { "Usage" })?;
        let encoded = bytes(&registry, &sources, &frame)?;
        if wrong_id {
            request.request_id += 1;
        }
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("missing receipt")?;
        let mut calls = 0;
        let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| {
            calls += 1;
            Ok::<_, ()>(sources)
        });
        assert_eq!(calls, 0);
        if wrong_id {
            assert!(matches!(result, Err(ReplyAdmissionError::RequestId { .. })));
        } else {
            assert!(matches!(
                result,
                Err(ReplyAdmissionError::Wire(WireError::Schema(
                    SchemaError::WrongType
                )))
            ));
        }
    }
    Ok(())
}

#[test]
fn staged_reply_preserves_policy_errors_and_sticky_stops() -> Result<(), String> {
    for stop in [false, true] {
        let (registry, request, sources, frame) = fixture("TraceOverflow")?;
        let encoded = bytes(&registry, &sources, &frame)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("missing receipt")?;
        let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, b| {
            if stop {
                b.stop(StopReason::Cancelled);
                Ok(sources)
            } else {
                Err(37u32)
            }
        });
        if stop {
            assert_eq!(
                result,
                Err(ReplyAdmissionError::Wire(WireError::Stopped(
                    StopReason::Cancelled
                )))
            );
        } else {
            assert_eq!(result, Err(ReplyAdmissionError::Policy(37)));
        }
    }
    Ok(())
}

#[test]
fn staged_reply_report_only_results_never_call_payload_policy() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let ProviderFrame::Reply {
        reply: OperationReply::Result(OperationResult::Complete { report, .. }),
        ..
    } = frame
    else {
        return Err("fixture".into());
    };
    for stopped in [false, true] {
        let reply = if stopped {
            OperationResult::Stopped {
                reason: StopReason::Cancelled,
                partial: None,
                report: report.clone(),
            }
        } else {
            OperationResult::Invalid {
                partial: None,
                report: report.clone(),
            }
        };
        let frame = ProviderFrame::Reply {
            request_id: request.request_id,
            reply: OperationReply::Result(reply),
        };
        let encoded = bytes(&registry, &sources, &frame)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
        let mut calls = 0;
        let decoded = pending
            .finish_with(&request, &sources, |_, _, _, _, _| {
                calls += 1;
                Err::<SourceStore, _>(())
            })
            .map_err(error)?;
        assert_eq!(calls, 0);
        assert_eq!(decoded, frame);
    }
    Ok(())
}

#[test]
fn staged_reply_does_not_automatically_resolve_missing_sources() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let encoded = bytes(&registry, &sources, &frame)?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let (pending, _) =
        decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
    assert!(pending.finish(&request, &SourceStore::default()).is_err());
    for end in [0, 4, encoded.len() - 1] {
        assert!(
            decode_pending_reply_frame(
                &encoded[..end],
                false,
                &registry,
                &mut admission,
                &mut budget()
            )
            .map_err(error)?
            .is_none()
        );
        assert!(
            decode_pending_reply_frame(
                &encoded[..end],
                true,
                &registry,
                &mut admission,
                &mut budget()
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn staged_partial_payloads_obey_selected_output_before_policy() -> Result<(), String> {
    for wrong in [false, true] {
        for stopped in [false, true] {
            let (registry, request, sources, frame) =
                fixture(if wrong { "Usage" } else { "TraceOverflow" })?;
            let ProviderFrame::Reply {
                reply: OperationReply::Result(OperationResult::Complete { value, report }),
                ..
            } = frame
            else {
                return Err("fixture".into());
            };
            let result = if stopped {
                OperationResult::Stopped {
                    reason: StopReason::Cancelled,
                    partial: Some(value),
                    report,
                }
            } else {
                OperationResult::Invalid {
                    partial: Some(value),
                    report,
                }
            };
            let frame = ProviderFrame::Reply {
                request_id: request.request_id,
                reply: OperationReply::Result(result),
            };
            let encoded = bytes(&registry, &sources, &frame)?;
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let (pending, _) =
                decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                    .map_err(error)?
                    .ok_or("receipt")?;
            let mut count = 0;
            let result = pending.finish_with(&request, &SourceStore::default(), |v, _, _, _, _| {
                count += 1;
                assert_eq!(
                    v.kind,
                    if stopped {
                        ReplyPayloadKind::StoppedPartial
                    } else {
                        ReplyPayloadKind::InvalidPartial
                    }
                );
                Ok::<_, ()>(sources)
            });
            assert_eq!(count, if wrong { 0 } else { 1 });
            if wrong {
                assert!(result.is_err());
            } else {
                assert_eq!(result.map_err(error)?, frame);
            }
        }
    }
    Ok(())
}

#[test]
fn staged_await_child_sources_never_authorize_outer_report() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let ProviderFrame::Reply {
        reply: OperationReply::Result(OperationResult::Complete { report, .. }),
        ..
    } = frame
    else {
        return Err("fixture".into());
    };
    let generated = sources.snapshots().first().ok_or("source")?.clone();
    let mut child = request.clone();
    child.request_id = 18;
    child.sources = vec![generated.clone()];
    let frame = ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Await {
            continuation: Continuation {
                provider: request.operation.clone(),
                parent_request: request.request_id,
                snapshot_digest: Digest::of(b"snapshot"),
                state: request.input.clone(),
            },
            calls: vec![child],
            report,
        },
    };
    let encoded = bytes(&registry, &sources, &frame)?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let (pending, _) =
        decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
    let mut calls = 0;
    let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| {
        calls += 1;
        Ok::<_, ()>(sources)
    });
    assert!(result.is_err());
    assert_eq!(calls, 0);
    // Failed outer Report decoding retains the child's consumed source bytes and
    // admission identity; retrying that identity cannot charge SourceBytes twice.
    let charged = b.usage().source_bytes;
    assert!(charged >= encoded.len() as u64 + generated.text().len() as u64);
    admission
        .admit_existing(&generated, &mut b)
        .map_err(error)?;
    assert_eq!(b.usage().source_bytes, charged);
    Ok(())
}

#[test]
fn staged_unknown_operation_and_digest_fail_before_policy() -> Result<(), String> {
    for wrong_digest in [false, true] {
        let (registry, mut request, sources, frame) = fixture("TraceOverflow")?;
        let encoded = bytes(&registry, &sources, &frame)?;
        if wrong_digest {
            request.operation.schema.digest = Digest::of(b"wrong");
        } else {
            request.operation.name = "unregistered".into();
        }
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
        let mut calls = 0;
        assert_eq!(
            pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| {
                calls += 1;
                Ok::<_, ()>(sources)
            }),
            Err(ReplyAdmissionError::UnknownOperation)
        );
        assert_eq!(calls, 0);
    }
    Ok(())
}

#[test]
fn staged_source_limit_covers_source_admission_after_framing() -> Result<(), String> {
    for enough in [false, true] {
        let (registry, request, sources, frame) = fixture("TraceOverflow")?;
        let encoded = bytes(&registry, &sources, &frame)?;
        let mut limits = budget().limits();
        limits.source_bytes = encoded.len() as u64 + if enough { 6 } else { 5 };
        let mut b = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
        let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, _| {
            Ok::<_, ()>(sources)
        });
        if enough {
            assert_eq!(result.map_err(error)?, frame);
        } else {
            assert_eq!(
                result,
                Err(ReplyAdmissionError::Wire(WireError::Stopped(
                    StopReason::SourceLimit
                )))
            );
        }
    }
    Ok(())
}

#[test]
fn staged_nested_source_contents_require_selected_policy_validation() -> Result<(), String> {
    for forged in [false, true] {
        let (registry, request, sources, mut frame) = fixture("SourceBundle")?;
        let schema = registry.selected("nepl3.foundation", 1).ok_or("schema")?;
        let source = sources.snapshots().first().ok_or("source")?;
        let bundle = nepl3_wire::source::encode_sources(
            core::slice::from_ref(source),
            schema,
            &registry,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let NdfValue::Record(ref mut bundle) =
            nepl3_wire::decode(&bundle, &mut budget()).map_err(error)?
        else {
            return Err("bundle".into());
        };
        if forged {
            let NdfValue::List(entries) = &mut bundle.fields[0] else {
                return Err("entries".into());
            };
            let NdfValue::Record(content) = &mut entries[0] else {
                return Err("content".into());
            };
            let NdfValue::Record(reference) = &mut content.fields[0] else {
                return Err("reference".into());
            };
            reference.fields[2] = NdfValue::Bytes(vec![0; 32]);
        }
        let ProviderFrame::Reply {
            reply: OperationReply::Result(OperationResult::Complete { value, .. }),
            ..
        } = &mut frame
        else {
            return Err("frame".into());
        };
        *value = TypedValue::Record(bundle.clone());
        let encoded = bytes(&registry, &sources, &frame)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
        if forged {
            let result =
                pending.finish_with(&request, &SourceStore::default(), |payload, _, _, _, b| {
                    let NdfValue::Record(bundle) = payload.value else {
                        return Err("bundle shape");
                    };
                    let NdfValue::List(entries) = &bundle.fields[0] else {
                        return Err("entries");
                    };
                    let NdfValue::Record(content) = &entries[0] else {
                        return Err("content");
                    };
                    let NdfValue::Record(reference) = &content.fields[0] else {
                        return Err("reference");
                    };
                    let NdfValue::Bytes(expected) = &reference.fields[2] else {
                        return Err("digest");
                    };
                    let NdfValue::Text(text) = &content.fields[2] else {
                        return Err("text");
                    };
                    b.charge(Resource::Work, text.len() as u64)
                        .map_err(|_| "budget")?;
                    if expected.as_slice() != Digest::of(text.as_bytes()).0 {
                        return Err("forged source digest");
                    }
                    Err::<SourceStore, _>("unexpected valid digest")
                });
            assert_eq!(
                result,
                Err(ReplyAdmissionError::Policy("forged source digest"))
            );
        } else {
            // Even a valid nested bundle is not discovered as ambient Report context.
            assert!(pending.finish(&request, &SourceStore::default()).is_err());
        }
    }
    Ok(())
}

#[test]
fn staged_finish_preserves_work_and_allocation_boundaries() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let encoded = bytes(&registry, &sources, &frame)?;
    let mut measured = budget();
    let mut admission = SourceAdmission::default();
    let (pending, _) =
        decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut measured)
            .map_err(error)?
            .ok_or("receipt")?;
    assert_eq!(pending.finish(&request, &sources).map_err(error)?, frame);
    let used = measured.usage();
    for work in [false, true] {
        for exact in [false, true] {
            let mut limits = budget().limits();
            let missing = u64::from(!exact);
            if work {
                limits.work = used.work - missing;
            } else {
                limits.allocation_units = used.allocation_units - missing;
            }
            let mut b = Budget::new(limits);
            let mut admission = SourceAdmission::default();
            let (pending, _) =
                decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                    .map_err(error)?
                    .ok_or("receipt")?;
            let result = pending.finish(&request, &sources);
            if exact {
                assert_eq!(result.map_err(error)?, frame);
            } else {
                let reason = if work {
                    StopReason::WorkLimit
                } else {
                    StopReason::AllocationLimit
                };
                assert_eq!(
                    result,
                    Err(ReplyAdmissionError::Wire(WireError::Stopped(reason)))
                );
                assert_eq!(b.poll(), Err(reason));
            }
        }
    }
    Ok(())
}

#[test]
fn staged_decode_needs_no_output_but_policy_cannot_hide_output_stop() -> Result<(), String> {
    for charge in [false, true] {
        let (registry, request, sources, frame) = fixture("TraceOverflow")?;
        let encoded = bytes(&registry, &sources, &frame)?;
        let mut limits = budget().limits();
        limits.output_bytes = 0;
        let mut b = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let (pending, _) =
            decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
                .map_err(error)?
                .ok_or("receipt")?;
        let result = pending.finish_with(&request, &SourceStore::default(), |_, _, _, _, b| {
            if charge {
                assert_eq!(
                    b.charge(Resource::OutputBytes, 1),
                    Err(StopReason::OutputLimit)
                );
            }
            Ok::<_, ()>(sources)
        });
        if charge {
            assert_eq!(
                result,
                Err(ReplyAdmissionError::Wire(WireError::Stopped(
                    StopReason::OutputLimit
                )))
            );
        } else {
            assert_eq!(result.map_err(error)?, frame);
            assert_eq!(b.usage().output_bytes, 0);
        }
    }
    Ok(())
}

#[test]
fn staged_await_uses_original_closure_without_terminal_policy() -> Result<(), String> {
    let (registry, request, sources, frame) = fixture("TraceOverflow")?;
    let ProviderFrame::Reply {
        reply: OperationReply::Result(OperationResult::Complete { report, .. }),
        ..
    } = frame
    else {
        return Err("fixture".into());
    };
    let frame = ProviderFrame::Reply {
        request_id: request.request_id,
        reply: OperationReply::Await {
            continuation: Continuation {
                provider: request.operation.clone(),
                parent_request: request.request_id,
                snapshot_digest: Digest::of(b"snapshot"),
                state: request.input.clone(),
            },
            calls: vec![],
            report,
        },
    };
    let encoded = bytes(&registry, &sources, &frame)?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let (pending, _) =
        decode_pending_reply_frame(&encoded, true, &registry, &mut admission, &mut b)
            .map_err(error)?
            .ok_or("receipt")?;
    let mut calls = 0;
    let decoded = pending
        .finish_with(&request, &sources, |_, _, _, _, _| {
            calls += 1;
            Err::<SourceStore, _>(())
        })
        .map_err(error)?;
    assert_eq!(calls, 0);
    assert_eq!(decoded, frame);
    Ok(())
}

#[test]
fn staged_boundary_rejects_malformed_schema_and_noncanonical_frames() -> Result<(), String> {
    let (registry, _, sources, frame) = fixture("TraceOverflow")?;
    let encoded = bytes(&registry, &sources, &frame)?;
    for defect in 0..4 {
        let mut value = nepl3_wire::decode(&encoded[8..], &mut budget()).map_err(error)?;
        let NdfValue::Variant(root) = &mut value else {
            return Err("variant".into());
        };
        match defect {
            0 => {
                root.fields.pop();
            }
            1 => root.fields[0] = NdfValue::Text("17".into()),
            2 => root.schema.digest = Digest::of(b"forged foundation"),
            _ => {}
        }
        let mut payload = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
        if defect == 3 {
            // NDF Variant is a CBOR array of five values. A longer encoding of
            // that same length is legal CBOR but forbidden by canonical NDF/1.
            assert_eq!(payload[0], 0x85);
            payload[0] = 0x98;
            payload.insert(1, 5);
        }
        let mut bytes = (payload.len() as u64).to_be_bytes().to_vec();
        bytes.extend(payload);
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let failure = decode_pending_reply_frame(&bytes, true, &registry, &mut admission, &mut b)
            .err()
            .ok_or("accepted malformed frame")?;
        if defect == 3 {
            assert_eq!(failure, ReplyFrameError::Wire(WireError::NonCanonical));
        } else {
            assert!(matches!(
                failure,
                ReplyFrameError::Wire(WireError::Schema(_))
            ));
        }
    }
    Ok(())
}
