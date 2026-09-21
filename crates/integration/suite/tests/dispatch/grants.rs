use super::*;
use nepl3_core::{
    source::{SourceId, SourceSnapshot},
    syntax::ResourceContent,
};
use nepl3_suite::grants::{GrantError, Grants};

#[test]
fn dependency_batch_requires_exact_operation_and_context_before_dispatch() -> Result<(), String> {
    use nepl3_suite::grants::dependencies::{DependencyGrantError, OperationGrant, authorize};
    let (_, request) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let policy = [OperationGrant {
        operation: &request.operation,
        grants: &grants,
    }];
    let mut next = request.clone();
    next.request_id += 1;
    let mut calls = vec![request.clone(), next];
    let approved = authorize(&calls, &policy, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(approved.len(), 2);
    for (index, proof) in approved.iter().enumerate() {
        assert_eq!(proof.operation(), &request.operation);
        assert!(core::ptr::eq(proof.invocation().request(), &calls[index]));
    }
    drop(approved);
    calls[1].operation.schema.digest = Digest::of(b"unapproved version");
    assert!(matches!(
        authorize(&calls, &policy, &mut budget()),
        Err(DependencyGrantError::NotAllowed)
    ));
    calls[1].operation = request.operation.clone();
    if let TypedValue::Record(record) = &mut calls[1].environment {
        record.fields[0] = NdfValue::U64(99);
    }
    assert!(matches!(
        authorize(&calls, &policy, &mut budget()),
        Err(DependencyGrantError::Context(GrantError::Environment))
    ));
    calls[1].environment = request.environment.clone();
    let duplicate = [
        OperationGrant {
            operation: &request.operation,
            grants: &grants,
        },
        OperationGrant {
            operation: &request.operation,
            grants: &grants,
        },
    ];
    assert!(matches!(
        authorize(&calls, &duplicate, &mut budget()),
        Err(DependencyGrantError::Ambiguous)
    ));
    for allocation in [false, true] {
        let mut limits = budget().limits();
        if allocation {
            limits.allocation_units = 0;
        } else {
            limits.work = 0;
        }
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            authorize(&calls, &policy, &mut stopped),
            Err(DependencyGrantError::Stopped(_))
        ));
        assert!(stopped.poll().is_err());
    }
    Ok(())
}

fn snapshot(uri: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId("input".into()),
        1,
        uri.into(),
        b"hello".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}

#[test]
fn approved_context_is_borrowed_and_can_be_dispatched() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    let environment = request.environment.clone();
    let source = snapshot("memory:input")?;
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(source.clone(), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let resources = [ResourceContent {
        id: "asset".into(),
        digest: Digest::of(b"bytes"),
        bytes: b"bytes".to_vec(),
    }];
    let grants = Grants::new(&environment, &sources, &resources, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    request.sources.push(source);
    request.resources = resources.to_vec();
    let approved = grants
        .admit(&request, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(core::ptr::eq(approved.request(), &request));
    let identity = Digest::of(b"implementation");
    let registrations = [Registration {
        operation: &request.operation,
        implementation: identity,
        invoke: increment,
    }];
    let result = invoke_terminal(
        &registrations,
        &request.operation,
        identity,
        approved.request(),
        &registry,
        &sources,
        &mut budget(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(result, OperationResult::Complete { .. }));
    request.sources.clear();
    request.resources.clear();
    assert!(grants.admit(&request, &mut budget()).is_ok());
    Ok(())
}

#[test]
fn caller_cannot_expand_environment_sources_or_resources() -> Result<(), String> {
    let (_, request) = fixture()?;
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(snapshot("memory:input")?, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let resources = [ResourceContent {
        id: "asset".into(),
        digest: Digest::of(b"bytes"),
        bytes: b"bytes".to_vec(),
    }];
    let grants = Grants::new(&request.environment, &sources, &resources, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut changed = request.clone();
    if let TypedValue::Record(record) = &mut changed.environment {
        record.fields[0] = NdfValue::U64(99);
    }
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::Environment)
    ));
    let mut changed = request.clone();
    changed.sources.push(snapshot("memory:other")?);
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::Source)
    ));
    let mut changed = request.clone();
    changed.resources = resources.to_vec();
    changed.resources[0].id = "ungranted".into();
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::Resource)
    ));
    changed.resources[0].id = "asset".into();
    changed.resources[0].bytes = b"new".to_vec();
    changed.resources[0].digest = Digest::of(b"new");
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::Resource)
    ));
    changed.resources[0].digest = Digest::of(b"bytes");
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::InvalidResources(_))
    ));
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        grants.admit(&request, &mut stopped),
        Err(GrantError::Stopped(StopReason::WorkLimit))
    ));
    assert!(
        Grants::new(
            &request.environment,
            &sources,
            &changed.resources,
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
