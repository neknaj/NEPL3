//! Sequential native dependency execution using explicit host frames.
use crate::{
    dispatch::{resume, suspending},
    grants::{GrantError, Grants, dependencies::OperationGrant},
    suspension::{
        execution::ExecutionScope,
        host::{ActivationError, OwnedActiveAwait, activate_owned},
    },
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    operation::{
        Invoke, OperationReply,
        lifetime::{LifetimeError, RequestLifetimes},
    },
    schema::SchemaRegistry,
    source::{Digest, SourceError, SourceStore},
    value::TypedValue,
};

/// The host supplies executable identity, exact grants and a context digest
/// function covering provider configuration plus the admitted invocation.
pub struct Registration<'a> {
    pub invoke: suspending::Registration<'a>,
    pub resume: resume::Registration<'a>,
    pub grants: &'a Grants<'a>,
    pub context: fn(&Invoke, Digest, &mut Budget) -> Result<Digest, StopReason>,
}

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Missing,
    Ambiguous,
    Registration,
    Grants(GrantError),
    Source(SourceError),
    Lifetime(LifetimeError),
    Invoke(suspending::Error),
    Resume(resume::ResumeError),
    Activation(ActivationError),
    Dependency(nepl3_core::operation::dependencies::DependencyError),
    State,
}
impl From<StopReason> for Error {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}

struct Frame {
    registration: usize,
    context: Digest,
    scope: ExecutionScope,
    sources: SourceStore,
    active: Option<OwnedActiveAwait>,
    contexts: Vec<Digest>,
    completed_sources: Vec<SourceStore>,
    next: usize,
}

fn reserve<T>(values: &mut Vec<T>, count: usize, b: &mut Budget) -> Result<(), Error> {
    b.charge(Resource::Work, count as u64)?;
    let required = values
        .len()
        .checked_add(count)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    if required <= values.capacity() {
        return Ok(());
    }
    let capacity = values
        .capacity()
        .checked_mul(2)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?
        .max(required);
    let bytes = (capacity - values.capacity())
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    b.charge(Resource::Work, values.len() as u64)?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(|_| Error::Stopped(b.stop(StopReason::AllocationLimit)))
}

fn select(
    registrations: &[Registration<'_>],
    request: &Invoke,
    b: &mut Budget,
) -> Result<usize, Error> {
    let mut selected = None;
    for (index, registration) in registrations.iter().enumerate() {
        b.charge(
            Resource::Work,
            (request.operation.name.len()
                + registration.invoke.operation.name.len()
                + request.operation.schema.package.len()
                + registration.invoke.operation.schema.package.len()) as u64
                + 40,
        )?;
        if *registration.invoke.operation == request.operation {
            if selected.is_some() {
                return Err(Error::Ambiguous);
            }
            if registration.resume.operation != registration.invoke.operation
                || registration.resume.implementation != registration.invoke.implementation
            {
                return Err(Error::Registration);
            }
            selected = Some(index);
        }
    }
    let index = selected.ok_or(Error::Missing)?;
    registrations[index]
        .grants
        .admit(request, b)
        .map_err(Error::Grants)?;
    Ok(index)
}

fn frame(
    registrations: &[Registration<'_>],
    request: &Invoke,
    context: Digest,
    scope: ExecutionScope,
    b: &mut Budget,
) -> Result<Frame, Error> {
    let registration = select(registrations, request, b)?;
    let mut sources = SourceStore::default();
    for source in &request.sources {
        sources
            .insert_ref_with_budget(source, b)
            .map_err(Error::Source)?;
    }
    Ok(Frame {
        registration,
        context,
        scope,
        sources,
        active: None,
        contexts: Vec::new(),
        completed_sources: Vec::new(),
        next: 0,
    })
}

fn request<'a>(root: &'a Invoke, ancestors: &'a [Frame]) -> Result<&'a Invoke, Error> {
    match ancestors.last() {
        None => Ok(root),
        Some(parent) => parent
            .active
            .as_ref()
            .ok_or(Error::State)?
            .pending
            .calls()
            .get(parent.next)
            .ok_or(Error::State),
    }
}

/// Run one root and all recursive Await generations using a shared execution
/// Budget and a separate validation Budget. Reports emitted at Await are passed
/// to `report`; terminal reports remain in results. These callbacks must not panic.
/// On error, all unfinished requests in this invocation's private lifetime table
/// are cancelled exactly once. Execution is synchronous; no OS I/O is performed.
/// Diagnostic permissions are the admitted `request.sources` of each call.
/// Additional generated sources require a separate host admission path.
pub fn run(
    registrations: &[Registration<'_>],
    root: &Invoke,
    registry: &SchemaRegistry,
    execution: &mut Budget,
    validation: &mut Budget,
    mut report: impl FnMut(u64, Report),
    mut cancel: impl FnMut(u64),
) -> Result<OperationResult<TypedValue>, Error> {
    let mut lifetimes = RequestLifetimes::default();
    let result = run_inner(
        registrations,
        root,
        registry,
        execution,
        validation,
        &mut lifetimes,
        &mut report,
    );
    if result.is_err() {
        lifetimes.close(&mut cancel);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    registrations: &[Registration<'_>],
    root: &Invoke,
    registry: &SchemaRegistry,
    execution: &mut Budget,
    validation: &mut Budget,
    lifetimes: &mut RequestLifetimes,
    report: &mut impl FnMut(u64, Report),
) -> Result<OperationResult<TypedValue>, Error> {
    let selected = select(registrations, root, validation)?;
    let context = (registrations[selected].context)(
        root,
        registrations[selected].invoke.implementation,
        validation,
    )?;
    let scope = ExecutionScope::root(execution, root.limits)?;
    let mut stack = Vec::new();
    reserve(&mut stack, 1, validation)?;
    stack.push(frame(registrations, root, context, scope, validation)?);
    lifetimes
        .begin_call(root, context, None, validation)
        .map_err(Error::Lifetime)?;
    let mut policy = Vec::new();
    reserve(&mut policy, registrations.len(), validation)?;
    for r in registrations {
        policy.push(OperationGrant {
            operation: r.invoke.operation,
            grants: r.grants,
        });
    }
    loop {
        validation.charge(Resource::Work, 1)?;
        let (current, ancestors) = stack.split_last_mut().ok_or(Error::State)?;
        let call = request(root, ancestors)?;
        let registration = &registrations[current.registration];
        let response = if let Some(active) = &mut current.active {
            if current.next < active.pending.calls().len() {
                let child = &active.pending.calls()[current.next];
                let scope = current.scope.child(child.limits, execution)?;
                let child_frame = frame(
                    registrations,
                    child,
                    current.contexts[current.next],
                    scope,
                    validation,
                )?;
                reserve(&mut stack, 1, validation)?;
                stack.push(child_frame);
                continue;
            }
            let resume_request = active
                .pending
                .take_resume(validation)
                .map_err(Error::Dependency)?;
            let mut sources = Vec::new();
            reserve(&mut sources, current.completed_sources.len(), validation)?;
            sources.extend(current.completed_sources.iter());
            let saved = resume::SavedAwait {
                parent: call,
                context: current.context,
                continuation: active.pending.continuation(),
                calls: active.pending.calls(),
                sources: &sources,
            };
            current
                .scope
                .run(execution, |execution| {
                    resume::execute(
                        &registration.resume,
                        registration.resume.implementation,
                        &saved,
                        &resume_request,
                        lifetimes,
                        registry,
                        &current.sources,
                        execution,
                        validation,
                    )
                })
                .map_err(Error::Resume)?
        } else {
            current
                .scope
                .run(execution, |execution| {
                    suspending::invoke(
                        &registration.invoke,
                        registration.invoke.implementation,
                        call,
                        current.context,
                        registry,
                        &current.sources,
                        execution,
                        validation,
                    )
                })
                .map_err(Error::Invoke)?
        };
        match response {
            OperationReply::Await { ref calls, .. } => {
                let mut contexts = Vec::new();
                reserve(&mut contexts, calls.len(), validation)?;
                let mut completed_sources = Vec::new();
                reserve(&mut completed_sources, calls.len(), validation)?;
                for dependency in calls {
                    let r = &registrations[select(registrations, dependency, validation)?];
                    contexts.push((r.context)(
                        dependency,
                        r.invoke.implementation,
                        validation,
                    )?);
                }
                let mut active = activate_owned(
                    call,
                    current.context,
                    response,
                    &policy,
                    &contexts,
                    registry,
                    &current.sources,
                    lifetimes,
                    validation,
                )
                .map_err(Error::Activation)?;
                report(call.request_id, core::mem::take(&mut active.report));
                current.active = Some(active);
                current.contexts = contexts;
                current.completed_sources = completed_sources;
                current.next = 0;
            }
            OperationReply::Result(result) => {
                let id = call.request_id;
                let context = current.context;
                let finished = stack.pop().ok_or(Error::State)?;
                if let Some(parent) = stack.last_mut() {
                    parent
                        .active
                        .as_mut()
                        .ok_or(Error::State)?
                        .pending
                        .accept_active(
                            id,
                            context,
                            result,
                            registry,
                            &finished.sources,
                            lifetimes,
                            validation,
                        )
                        .map_err(Error::Dependency)?;
                    parent.completed_sources.push(finished.sources);
                    parent.next += 1;
                } else {
                    lifetimes.finish(id, validation).map_err(Error::Lifetime)?;
                    return Ok(result);
                }
            }
        }
    }
}
