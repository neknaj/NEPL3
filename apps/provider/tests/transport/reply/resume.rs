use super::*;
use nepl3_core::operation::lifetime::{RequestLifetimes, RequestPhase};
use nepl3_suite::dispatch::resume::{Registration, SavedAwait};

fn resume_result(
    _: &Invoke,
    request: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 4)?;
    if let [OperationReply::Result(OperationResult::Complete { value, .. })] =
        request.dependency_results.as_slice()
    {
        return Ok(OperationReply::Result(OperationResult::Complete {
            value: value.clone_with_budget(b)?,
            report: Report::default(),
        }));
    }
    Ok(OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    }))
}

#[test]
fn decoded_resume_runs_saved_callback_once_and_returns_dependency_value() -> Result<(), String> {
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"context");
    let identity = Digest::of(b"provider");
    let sources = SourceStore::default();
    let mut child = parent.clone();
    child.request_id = 18;
    let continuation = Continuation {
        provider: parent.operation.clone(),
        parent_request: 17,
        snapshot_digest: context,
        state: parent.input.clone(),
    };
    let result = increment(&child, context, &registry, &mut budget()).map_err(error)?;
    let request = Resume {
        request_id: 17,
        continuation: continuation.clone(),
        dependency_results: vec![result],
    };
    let mut server = connection(&ProviderFrame::Resume(request), &registry)?;
    let Some(ProviderFrame::Resume(received)) = server
        .receive(
            &registry,
            &sources,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(error)?
    else {
        return Err("expected Resume".into());
    };
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&parent, context, None, &mut budget())
        .map_err(error)?;
    lifetimes
        .suspend(17, continuation.clone(), 1, &mut budget())
        .map_err(error)?;
    let registration = Registration {
        operation: &parent.operation,
        implementation: identity,
        resume: resume_result,
    };
    let calls = [child];
    let grants = [&sources];
    let saved = SavedAwait {
        parent: &parent,
        context,
        continuation: &continuation,
        calls: &calls,
        sources: &grants,
    };
    let mut execution = budget();
    execution.charge(Resource::Work, 13).map_err(error)?;
    let reply = server
        .dispatch_resume(
            &registration,
            identity,
            &saved,
            &received,
            &mut lifetimes,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget(),
        )
        .map_err(error)?;
    let work = execution.usage().work;
    assert!(work >= 17);
    assert_eq!(
        lifetimes.phase(17, &mut budget()),
        Ok(RequestPhase::Running)
    );
    assert!(matches!(
        server.dispatch_resume(
            &registration,
            identity,
            &saved,
            &received,
            &mut lifetimes,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget()
        ),
        Err(nepl3_provider::dispatch::DispatchError::Resume(_))
    ));
    assert!(server.is_closed());
    assert_eq!(execution.usage().work, work);
    let (_, bytes) = server.into_parts();
    let mut client = Connection::new(Cursor::new(bytes), Vec::new());
    let portable =
        receive(&mut client, &parent, context, &registry, &mut budget()).map_err(error)?;
    assert_eq!(portable, reply);
    let OperationReply::Result(OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    }) = portable
    else {
        return Err("expected complete dependency value".into());
    };
    assert_eq!(record.fields, vec![NdfValue::U64(42)]);
    // Rejected replay emitted no second frame.
    assert!(
        client
            .receive(
                &registry,
                &sources,
                &mut SourceAdmission::default(),
                &mut budget()
            )
            .map_err(error)?
            .is_none()
    );
    Ok(())
}

#[test]
fn resume_write_failure_consumes_lifetime_and_closes_transport() -> Result<(), String> {
    let (registry, parent) = fixture()?;
    let context = Digest::of(b"context");
    let identity = Digest::of(b"provider");
    let sources = SourceStore::default();
    let continuation = Continuation {
        provider: parent.operation.clone(),
        parent_request: 17,
        snapshot_digest: context,
        state: parent.input.clone(),
    };
    let request = Resume {
        request_id: 17,
        continuation: continuation.clone(),
        dependency_results: vec![],
    };
    let registration = Registration {
        operation: &parent.operation,
        implementation: identity,
        resume: resume_result,
    };
    let saved = SavedAwait {
        parent: &parent,
        context,
        continuation: &continuation,
        calls: &[],
        sources: &[] as &[&SourceStore],
    };
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(&parent, context, None, &mut budget())
        .map_err(error)?;
    lifetimes
        .suspend(17, continuation.clone(), 0, &mut budget())
        .map_err(error)?;
    let mut server = Connection::new(io::empty(), Broken);
    let mut execution = budget();
    assert!(matches!(
        server.dispatch_resume(
            &registration,
            identity,
            &saved,
            &request,
            &mut lifetimes,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget()
        ),
        Err(nepl3_provider::dispatch::DispatchError::Transport(
            TransportError::Io(_)
        ))
    ));
    assert_eq!(execution.usage().work, 4);
    assert_eq!(
        lifetimes.phase(17, &mut budget()),
        Ok(RequestPhase::Running)
    );
    assert!(server.is_closed());
    assert!(matches!(
        server.dispatch_resume(
            &registration,
            identity,
            &saved,
            &request,
            &mut lifetimes,
            &registry,
            &sources,
            &sources,
            &mut SourceAdmission::default(),
            &mut execution,
            &mut budget(),
            &mut budget()
        ),
        Err(nepl3_provider::dispatch::DispatchError::Transport(
            TransportError::Closed
        ))
    ));
    assert_eq!(execution.usage().work, 4);
    Ok(())
}
