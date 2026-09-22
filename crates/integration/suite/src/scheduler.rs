//! Sequential native dependency execution using explicit host frames.
use crate::{
    dispatch::{resume, suspending},
    grants::{GrantError, Grants, dependencies::OperationGrant},
    suspension::{
        execution::ExecutionScope,
        host::{ActivationError, OwnedActiveAwait},
    },
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::{OperationResult, Report},
    operation::{
        Invoke,
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

/// Owns the failure cause and the active Await generations. Their accepted
/// terminal results retain each child's operation, output schema and sources.
/// Generations already transferred to a Resume callback are consumed.
pub struct Failure {
    pub cause: Error,
    root_request: u64,
    frames: Vec<Frame>,
}

impl Failure {
    /// Request whose execution frame was active when scheduling failed. Before
    /// root activation this is the root ID. A dependency admission/depth failure
    /// is attributed to its active parent; accepted child outcomes retain their
    /// own IDs through accepted_results. This accessor performs no allocation.
    pub fn active_request_id(&self) -> u64 {
        self.frames
            .last()
            .map_or(self.root_request, |frame| frame.request_id)
    }
    /// Borrow results already accepted by the host, outer generation first and
    /// in call order within each generation. Rejected replies are excluded.
    /// This inspection neither allocates nor polls the stopped Budget.
    pub fn accepted_results(
        &self,
    ) -> impl Iterator<Item = (&Invoke, &OperationResult<TypedValue>)> {
        self.frames
            .iter()
            .filter_map(|frame| frame.active.as_ref())
            .flat_map(|active| active.pending.accepted_results())
    }
}

impl core::fmt::Debug for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Failure")
            .field("cause", &self.cause)
            .field("active_request_id", &self.active_request_id())
            .field("active_frames", &self.frames.len())
            .finish_non_exhaustive()
    }
}

struct Frame {
    request_id: u64,
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
        request_id: request.request_id,
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
/// Failure retains unconsumed, accepted dependency results without allocating
/// during failure handling. A stopped child never resumes its parent.
pub fn run(
    registrations: &[Registration<'_>],
    root: &Invoke,
    registry: &SchemaRegistry,
    execution: &mut Budget,
    validation: &mut Budget,
    mut report: impl FnMut(u64, Report),
    mut cancel: impl FnMut(u64),
) -> Result<OperationResult<TypedValue>, Failure> {
    let mut lifetimes = RequestLifetimes::default();
    let mut stack = Vec::new();
    let result = run_inner(
        registrations,
        root,
        registry,
        execution,
        validation,
        &mut lifetimes,
        &mut report,
        &mut stack,
    );
    match result {
        Ok(result) => Ok(result),
        Err(cause) => {
            lifetimes.close(&mut cancel);
            Err(Failure {
                cause,
                root_request: root.request_id,
                frames: stack,
            })
        }
    }
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
    stack: &mut Vec<Frame>,
) -> Result<OperationResult<TypedValue>, Error> {
    let selected = select(registrations, root, validation)?;
    let context = (registrations[selected].context)(
        root,
        registrations[selected].invoke.implementation,
        validation,
    )?;
    let scope = ExecutionScope::root(execution, root.limits)?;
    reserve(stack, 1, validation)?;
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
        // Preserve accepted child results before any Resume extraction or
        // further validation once the shared execution has stopped.
        execution.poll()?;
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
                reserve(stack, 1, validation)?;
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
                    resume::execute_with(
                        &registration.resume,
                        registration.resume.implementation,
                        &saved,
                        &resume_request,
                        lifetimes,
                        registry,
                        execution,
                        validation,
                        |reply, validation| {
                            suspending::prepare_reply(
                                reply,
                                call,
                                current.context,
                                registry,
                                &current.sources,
                                validation,
                            )
                            .map_err(Into::into)
                        },
                    )
                })
                .map_err(Error::Resume)?
        } else {
            current
                .scope
                .run(execution, |execution| {
                    suspending::invoke_with(
                        &registration.invoke,
                        registration.invoke.implementation,
                        call,
                        current.context,
                        registry,
                        execution,
                        validation,
                        |reply, validation| {
                            suspending::prepare_reply(
                                reply,
                                call,
                                current.context,
                                registry,
                                &current.sources,
                                validation,
                            )
                        },
                    )
                })
                .map_err(Error::Invoke)?
        };
        match response {
            suspending::PreparedReply::Await(prepared) => {
                let calls = prepared.calls();
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
                let mut active = prepared
                    .activate(&policy, &contexts, lifetimes, validation)
                    .map_err(Error::Activation)?;
                report(call.request_id, core::mem::take(&mut active.report));
                current.active = Some(active);
                current.contexts = contexts;
                current.completed_sources = completed_sources;
                current.next = 0;
            }
            suspending::PreparedReply::Result(result) => {
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
