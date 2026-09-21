//! Native reference execution using the same public admission and resume APIs.
use super::*;

pub fn execute(registry: &SchemaRegistry, request: &Invoke) -> Result<OperationReply, String> {
    let sources = SourceStore::default();
    let authority =
        Grants::new(&request.environment, &sources, &[], &mut budget()).map_err(error)?;
    let approved = authority.admit(request, &mut budget()).map_err(error)?;
    let context = context(request, registry)?;
    let mut execution = budget();
    let mut lifetimes = RequestLifetimes::default();
    lifetimes
        .begin_call(request, context, None, &mut budget())
        .map_err(error)?;
    let registration = suspending::Registration {
        operation: &request.operation,
        implementation: identity(),
        invoke: await_increment,
    };
    let OperationReply::Await {
        continuation,
        calls,
        ..
    } = suspending::invoke(
        &registration,
        identity(),
        approved.request(),
        context,
        registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(error)?
    else {
        return Err("expected native Await".into());
    };
    if calls.len() != 1 {
        return Err("expected one native dependency".into());
    }
    lifetimes
        .suspend(
            request.request_id,
            continuation.clone(),
            calls.len(),
            &mut budget(),
        )
        .map_err(error)?;
    let approved = authority.admit(&calls[0], &mut budget()).map_err(error)?;
    let mut selected = request.operation.clone();
    selected.name = "increment".into();
    let registration = suspending::Registration {
        operation: &selected,
        implementation: identity(),
        invoke: increment,
    };
    let OperationReply::Result(result) = suspending::invoke(
        &registration,
        identity(),
        approved.request(),
        context,
        registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(error)?
    else {
        return Err("expected native terminal dependency".into());
    };
    let mut dependencies =
        PendingDependencies::new(&continuation, &calls, &mut budget()).map_err(error)?;
    dependencies
        .accept(
            calls[0].request_id,
            result,
            registry,
            &sources,
            &mut budget(),
        )
        .map_err(error)?;
    let resume = dependencies.take_resume(&mut budget()).map_err(error)?;
    let source_grants = [&sources];
    let saved = resume::SavedAwait {
        parent: request,
        context,
        continuation: &continuation,
        calls: &calls,
        sources: &source_grants,
    };
    let registration = resume::Registration {
        operation: &request.operation,
        implementation: identity(),
        resume: finish,
    };
    let result = resume::execute(
        &registration,
        identity(),
        &saved,
        &resume,
        &mut lifetimes,
        registry,
        &sources,
        &mut execution,
        &mut budget(),
    )
    .map_err(error)?;
    lifetimes
        .finish(request.request_id, &mut budget())
        .map_err(error)?;
    Ok(result)
}

pub fn compare(native: &OperationReply, portable: &OperationReply) -> Result<(), String> {
    match (native, portable) {
        (
            OperationReply::Result(OperationResult::Complete {
                value: a,
                report: ar,
            }),
            OperationReply::Result(OperationResult::Complete {
                value: b,
                report: br,
            }),
        ) if a == b && ar.diagnostics == br.diagnostics => Ok(()),
        (
            OperationReply::Result(OperationResult::Invalid {
                partial: a,
                report: ar,
            }),
            OperationReply::Result(OperationResult::Invalid {
                partial: b,
                report: br,
            }),
        ) if a == b && ar.diagnostics == br.diagnostics => Ok(()),
        _ => Err("native/process semantic value or diagnostic mismatch".into()),
    }
    // Internal traces and implementation-specific usage are excluded by spec09.
}
