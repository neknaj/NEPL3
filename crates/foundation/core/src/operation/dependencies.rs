//! One Await generation: correlate terminal replies and preserve call order.
use super::lifetime::{LifetimeError, RequestLifetimes};
use super::validation::ResultValidationError;
use super::*;
use crate::{diagnostic::validation::DiagnosticSourceResolver, schema::SchemaRegistry};
use alloc::borrow::Cow;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyError {
    Stopped(StopReason),
    Output(ResultValidationError),
    Lifetime(LifetimeError),
    DuplicateId,
    ParentId,
    UnknownId,
    DuplicateReply,
    Incomplete,
    Consumed,
}
impl From<StopReason> for DependencyError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<ResultValidationError> for DependencyError {
    fn from(error: ResultValidationError) -> Self {
        match error {
            ResultValidationError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Output(error),
        }
    }
}

/// Retains the immutable Await accepted by the host, borrowed or owned.
/// The host authorizes calls,
/// registers their connection-wide IDs, detects call graph cycles, and executes
/// them within the parent's budget. Nested Await replies are resolved by the
/// host before submitting their terminal results here. Each batch is one Await
/// generation; transport routing must retain that association.
pub struct PendingDependencies<'a> {
    continuation: Cow<'a, Continuation>,
    calls: Cow<'a, [Invoke]>,
    index: Vec<(u64, usize)>,
    results: Vec<Option<OperationResult<TypedValue>>>,
    remaining: usize,
    consumed: bool,
}

fn reserve<T>(values: &mut Vec<T>, count: usize, b: &mut Budget) -> Result<(), StopReason> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))
}
// Meter comparisons and swaps individually; constructor failure publishes no batch.
fn sift(values: &mut [(u64, usize)], mut root: usize, b: &mut Budget) -> Result<(), StopReason> {
    while root < values.len() / 2 {
        b.charge(Resource::Work, 1)?;
        let mut child = root * 2 + 1;
        if child + 1 < values.len() {
            b.charge(Resource::Work, 1)?;
            if values[child].0 < values[child + 1].0 {
                child += 1;
            }
        }
        b.charge(Resource::Work, 1)?;
        if values[root].0 >= values[child].0 {
            break;
        }
        b.charge(Resource::Work, 1)?;
        values.swap(root, child);
        root = child;
    }
    Ok(())
}
impl<'a> PendingDependencies<'a> {
    pub fn new(
        continuation: &'a Continuation,
        calls: &'a [Invoke],
        b: &mut Budget,
    ) -> Result<Self, DependencyError> {
        Self::from_inputs(Cow::Borrowed(continuation), Cow::Borrowed(calls), b)
    }
    fn from_inputs(
        continuation: Cow<'a, Continuation>,
        calls: Cow<'a, [Invoke]>,
        b: &mut Budget,
    ) -> Result<Self, DependencyError> {
        b.poll()?;
        let mut index = Vec::new();
        reserve(&mut index, calls.len(), b)?;
        for (position, call) in calls.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if call.request_id == continuation.parent_request {
                return Err(DependencyError::ParentId);
            }
            index.push((call.request_id, position));
        }
        for root in (0..index.len() / 2).rev() {
            sift(&mut index, root, b)?;
        }
        for end in (1..index.len()).rev() {
            b.charge(Resource::Work, 1)?;
            index.swap(0, end);
            sift(&mut index[..end], 0, b)?;
        }
        for pair in index.windows(2) {
            b.charge(Resource::Work, 1)?;
            if pair[0].0 == pair[1].0 {
                return Err(DependencyError::DuplicateId);
            }
        }
        let mut results = Vec::new();
        reserve(&mut results, calls.len(), b)?;
        b.charge(Resource::Work, calls.len() as u64)?;
        results.resize_with(calls.len(), || None);
        let remaining = calls.len();
        Ok(Self {
            continuation,
            calls,
            index,
            results,
            remaining,
            consumed: false,
        })
    }
    /// Immutable input retained for host scheduling and Resume validation.
    pub fn calls(&self) -> &[Invoke] {
        &self.calls
    }
    pub fn continuation(&self) -> &Continuation {
        &self.continuation
    }
    /// Borrow accepted terminal results together with their immutable requests,
    /// in call order. Consumed Resume generations contain no remaining results.
    /// Iteration requires no allocation and remains available after Budget stop.
    pub fn accepted_results(
        &self,
    ) -> impl Iterator<Item = (&Invoke, &OperationResult<TypedValue>)> {
        self.calls
            .iter()
            .zip(&self.results)
            .filter_map(|(call, result)| result.as_ref().map(|result| (call, result)))
    }
    /// Validate and store one terminal result. Rejection leaves every slot intact.
    /// The supplied resolver must carry the permissions of this specific call.
    pub fn accept(
        &mut self,
        id: u64,
        result: OperationResult<TypedValue>,
        registry: &SchemaRegistry,
        sources: &impl DiagnosticSourceResolver,
        b: &mut Budget,
    ) -> Result<(), DependencyError> {
        let position = self.validate_result(id, &result, registry, sources, b)?;
        self.results[position] = Some(result);
        self.remaining -= 1;
        Ok(())
    }
    /// Accept a terminal result and finish its Running lifetime atomically.
    /// `context` comes from the host's saved child invocation. Use `accept` when
    /// the transport has already committed a validated terminal lifetime.
    /// Any rejection leaves both this collection and the lifetime unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn accept_active(
        &mut self,
        id: u64,
        context: Digest,
        result: OperationResult<TypedValue>,
        registry: &SchemaRegistry,
        sources: &impl DiagnosticSourceResolver,
        lifetimes: &mut RequestLifetimes,
        b: &mut Budget,
    ) -> Result<(), DependencyError> {
        let position = self.validate_result(id, &result, registry, sources, b)?;
        let map = |e| match e {
            LifetimeError::Stopped(s) => DependencyError::Stopped(s),
            e => DependencyError::Lifetime(e),
        };
        lifetimes
            .check_reply(id, &self.calls[position].operation, context, b)
            .map_err(map)?;
        lifetimes.finish(id, b).map_err(map)?;
        self.results[position] = Some(result);
        self.remaining -= 1;
        Ok(())
    }
    fn validate_result(
        &self,
        id: u64,
        result: &OperationResult<TypedValue>,
        registry: &SchemaRegistry,
        sources: &impl DiagnosticSourceResolver,
        b: &mut Budget,
    ) -> Result<usize, DependencyError> {
        b.poll()?;
        if self.consumed {
            return Err(DependencyError::Consumed);
        }
        let (mut lo, mut hi) = (0, self.index.len());
        let position = loop {
            if lo == hi {
                return Err(DependencyError::UnknownId);
            }
            b.charge(Resource::Work, 1)?;
            let mid = lo + (hi - lo) / 2;
            match self.index[mid].0.cmp(&id) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => break self.index[mid].1,
            }
        };
        if self.results[position].is_some() {
            return Err(DependencyError::DuplicateReply);
        }
        result.validate_for(&self.calls[position].operation, registry, sources, b)?;
        Ok(position)
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
    /// Assemble results in call order. Budget failure preserves the entire batch;
    /// success consumes it once. The host then commits its lifetime transition.
    pub fn take_resume(&mut self, b: &mut Budget) -> Result<Resume, DependencyError> {
        b.poll()?;
        if self.consumed {
            return Err(DependencyError::Consumed);
        }
        if self.remaining != 0 {
            return Err(DependencyError::Incomplete);
        }
        b.charge(Resource::Work, self.results.len() as u64)?;
        if self.results.iter().any(Option::is_none) {
            return Err(DependencyError::Incomplete);
        }
        let saved = &*self.continuation;
        b.charge(
            Resource::Work,
            (saved.provider.name.len() as u64)
                .saturating_add(saved.provider.schema.package.len() as u64)
                .saturating_add(1),
        )?;
        b.charge(
            Resource::AllocationUnits,
            (saved.provider.name.len() as u64)
                .saturating_add(saved.provider.schema.package.len() as u64),
        )?;
        let continuation = Continuation {
            provider: saved.provider.clone(),
            parent_request: saved.parent_request,
            snapshot_digest: saved.snapshot_digest,
            state: saved.state.clone_with_budget(b)?,
        };
        let mut dependency_results = Vec::new();
        reserve(&mut dependency_results, self.results.len(), b)?;
        b.charge(Resource::Work, self.results.len() as u64)?;
        // Every slot was checked above; no fallible operation follows extraction.
        dependency_results.extend(
            self.results
                .iter_mut()
                .filter_map(Option::take)
                .map(OperationReply::Result),
        );
        assert_eq!(dependency_results.len(), self.calls.len());
        self.consumed = true;
        Ok(Resume {
            request_id: saved.parent_request,
            continuation,
            dependency_results,
        })
    }
}

impl PendingDependencies<'static> {
    /// Move an owned Await generation into host scheduling state without cloning
    /// its inputs. Their construction was charged by the producer; this method
    /// charges the same correlation index and result storage as `new`.
    /// Schema, grants and lifetime admission remain the host's responsibility.
    /// Failure consumes the supplied inputs. Construct this state before
    /// publishing lifetimes, or cancel admitted requests on failure.
    pub fn from_owned(
        continuation: Continuation,
        calls: Vec<Invoke>,
        b: &mut Budget,
    ) -> Result<Self, DependencyError> {
        Self::from_inputs(Cow::Owned(continuation), Cow::Owned(calls), b)
    }
}
