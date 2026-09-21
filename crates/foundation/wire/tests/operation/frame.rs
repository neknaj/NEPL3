use super::*;
use nepl3_core::diagnostic::{OperationResult, Report};

#[test]
fn decoded_resume_is_checked_against_host_saved_lifetime() -> Result<(), String> {
    use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes, RequestPhase};
    let (registry, call) = setup()?;
    let sources = SourceStore::default();
    let continuation = Continuation {
        provider: call.operation.clone(),
        parent_request: call.request_id,
        snapshot_digest: Digest::of(b"saved request snapshot"),
        state: call.environment.clone(),
    };
    let mut host = RequestLifetimes::default();
    host.begin(
        call.request_id,
        call.operation,
        continuation.snapshot_digest,
        &mut budget(),
    )
    .map_err(error)?;
    host.suspend(call.request_id, continuation.clone(), 0, &mut budget())
        .map_err(error)?;
    for forged in [true, false] {
        let mut received = continuation.clone();
        if forged {
            // Both records satisfy the same wire schema. Host correlation must
            // reject the changed state even after successful schema decoding.
            received.state = call.input.clone();
        }
        let frame = ProviderFrame::Resume(Resume {
            request_id: call.request_id,
            continuation: received,
            dependency_results: vec![],
        });
        let bytes = encode_frame(
            &frame,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        let (frame, remaining) = decode_frame(
            &bytes,
            true,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
        .ok_or("missing resume frame")?;
        assert!(remaining.is_empty());
        let ProviderFrame::Resume(resume) = frame else {
            return Err("expected resume".into());
        };
        if forged {
            assert_eq!(
                host.resume(&resume, &mut budget()),
                Err(LifetimeError::Binding(ContinuationError::State))
            );
            assert_eq!(
                host.phase(call.request_id, &mut budget()),
                Ok(RequestPhase::Awaiting)
            );
        } else {
            host.resume(&resume, &mut budget()).map_err(error)?;
            assert_eq!(
                host.phase(call.request_id, &mut budget()),
                Ok(RequestPhase::Running)
            );
            assert_eq!(
                host.resume(&resume, &mut budget()),
                Err(LifetimeError::Phase)
            );
        }
    }
    Ok(())
}

#[test]
fn provider_frames_preserve_cases_ids_and_stream_boundaries() -> Result<(), String> {
    let (registry, call) = setup()?;
    let sources = SourceStore::default();
    let response = OperationReply::Result(OperationResult::Complete {
        value: call.input.clone(),
        report: Report::default(),
    });
    let resume = Resume {
        request_id: 37,
        continuation: Continuation {
            provider: call.operation.clone(),
            parent_request: 17,
            snapshot_digest: Digest::of(b"saved"),
            state: call.environment.clone(),
        },
        dependency_results: vec![response.clone()],
    };
    let cases = [
        ("Invoke", ProviderFrame::Invoke(call)),
        ("Resume", ProviderFrame::Resume(resume)),
        (
            "Reply",
            ProviderFrame::Reply {
                request_id: 41,
                reply: response,
            },
        ),
        ("Cancel", ProviderFrame::Cancel { request_id: 43 }),
        ("Close", ProviderFrame::Close),
    ];
    for (name, frame) in cases {
        let bytes = encode_frame(
            &frame,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?;
        // Inspect the independently specified envelope layout before roundtrip.
        let raw = nepl3_wire::decode(&bytes[8..], &mut budget()).map_err(error)?;
        let NdfValue::Variant(raw) = &raw else {
            return Err("frame variant".into());
        };
        assert_eq!(raw.type_name, "ProviderFrame");
        assert_eq!(raw.variant, name);
        match name {
            "Reply" => {
                assert_eq!(raw.fields.len(), 2);
                assert_eq!(raw.fields[0], NdfValue::U64(41));
            }
            "Cancel" => assert_eq!(raw.fields, [NdfValue::U64(43)]),
            "Close" => assert!(raw.fields.is_empty()),
            _ => assert_eq!(raw.fields.len(), 1),
        }
        let mut stream = bytes.clone();
        stream.extend_from_slice(&bytes);
        let (actual, rest) = decode_frame(
            &stream,
            true,
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
        .ok_or("missing frame")?;
        assert_eq!(actual, frame);
        assert_eq!(rest, bytes);
        assert!(
            decode_frame(
                &bytes[..bytes.len() - 1],
                false,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(error)?
            .is_none()
        );
        assert!(matches!(
            decode_frame(
                &bytes[..bytes.len() - 1],
                true,
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            ),
            Err(WireError::UnexpectedEnd)
        ));
    }
    Ok(())
}

#[test]
fn provider_frames_reject_wrong_cases_fields_and_stopped_budget() -> Result<(), String> {
    let (registry, call) = setup()?;
    for (case, fields) in [
        ("Unknown", vec![]),
        ("Close", vec![NdfValue::U64(1)]),
        ("Cancel", vec![]),
        ("Cancel", vec![NdfValue::Text("1".into())]),
    ] {
        let raw = NdfValue::Variant(Variant {
            schema: call.operation.schema.clone(),
            type_name: "ProviderFrame".into(),
            variant: case.into(),
            fields,
        });
        let payload = nepl3_wire::encode(&raw, &mut budget()).map_err(error)?;
        let mut bytes = (payload.len() as u64).to_be_bytes().to_vec();
        bytes.extend(payload);
        assert!(
            decode_frame(
                &bytes,
                true,
                &registry,
                &SourceStore::default(),
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .is_err()
        );
    }
    let mut stopped = budget();
    stopped.cancel();
    assert!(
        encode_frame(
            &ProviderFrame::Close,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut stopped
        )
        .is_err()
    );
    assert!(stopped.poll().is_err());
    Ok(())
}
