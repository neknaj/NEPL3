use super::*;
use nepl3_core::operation::lifetime::LifetimeError;
use nepl3_suite::suspension::host::ActivationError;

fn cycle(
    call: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let mut child = call.clone();
    child.request_id += 1;
    Ok(OperationReply::Await {
        continuation: Continuation {
            provider: call.operation.clone(),
            parent_request: call.request_id,
            snapshot_digest: context,
            state: call.input.clone_with_budget(b)?,
        },
        calls: vec![child],
        report: Report::default(),
    })
}

#[test]
fn scheduler_rejects_missing_ambiguous_registration_and_ancestor_cycle() -> Result<(), String> {
    let (registry, root) = fixture()?;
    let sources = SourceStore::default();
    let grants = Grants::new(&root.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let identity = Digest::of(b"cycle fixture");
    let registration = || scheduler::Registration {
        invoke: suspending::Registration {
            operation: &root.operation,
            implementation: identity,
            invoke: cycle,
        },
        resume: resume::Registration {
            operation: &root.operation,
            implementation: identity,
            resume,
        },
        grants: &grants,
        context,
    };
    let mut cancelled = Vec::new();
    let mut execution = budget();
    assert!(matches!(
        scheduler::run(
            &[],
            &root,
            &registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |id| cancelled.push(id)
        ),
        Err(scheduler::Failure {
            cause: scheduler::Error::Missing,
            ..
        })
    ));
    assert!(matches!(
        scheduler::run(
            &[registration(), registration()],
            &root,
            &registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |id| cancelled.push(id)
        ),
        Err(scheduler::Failure {
            cause: scheduler::Error::Ambiguous,
            ..
        })
    ));
    assert_eq!(execution.usage(), Usage::default());
    assert!(cancelled.is_empty());
    let mut mismatched = registration();
    mismatched.resume.implementation = Digest::of(b"different executable");
    assert!(matches!(
        scheduler::run(
            &[mismatched],
            &root,
            &registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |id| cancelled.push(id)
        ),
        Err(scheduler::Failure {
            cause: scheduler::Error::Registration,
            ..
        })
    ));
    assert_eq!(execution.usage(), Usage::default());
    assert!(cancelled.is_empty());
    assert!(matches!(
        scheduler::run(
            &[registration()],
            &root,
            &registry,
            &mut execution,
            &mut budget(),
            |_, _| {},
            |id| cancelled.push(id)
        ),
        Err(scheduler::Failure {
            cause: scheduler::Error::Activation(ActivationError::Lifetime(
                LifetimeError::CyclicOperation
            )),
            ..
        })
    ));
    assert_eq!(cancelled, vec![root.request_id]);
    Ok(())
}
