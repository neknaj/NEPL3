//! Request-bound host reservation for an explicitly selected provider.
use nepl3_core::{
    budget::{Budget, StopReason, Usage},
    operation::{
        Invoke,
        request::{InputValidationError, RequestBindingError},
    },
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission},
};
use nepl3_suite::{
    grants::AuthorizedInvoke,
    suspension::delegation::{IssuedBudget, LocalFailure, SettlementError, child},
};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Input(InputValidationError),
    Context(nepl3_wire::WireError),
    ObservationBinding,
    Settlement(SettlementError),
}

/// Failure from the optional complete-request settlement entry.
/// The existing `Error` returned by legacy issuance/settlement is unchanged.
#[derive(Debug)]
pub enum SavedSettlementError {
    Stopped(StopReason),
    RequestBinding(RequestBindingError),
    Settlement(Error),
}

/// Immutable request/grant binding retained by the authorized host. The host
/// selects the executable and verifies its measurement channel independently.
/// No received Report is promoted to an authenticated observation by this API.
#[must_use = "settle verified consumption or cancel the outstanding provider"]
pub struct IssuedInvocation<'request, 'budget> {
    authorized: AuthorizedInvoke<'request>,
    implementation: Digest,
    context: Digest,
    budget: IssuedBudget<'budget>,
}

/// Locally executed observation associated with the immutable admitted request.
/// The callback received that exact borrowed Invoke. The context digest covers
/// admitted environment/source/resource context and implementation configuration;
/// it is not an exact Invoke digest or a remote accounting token.
pub struct LocalExecution<T> {
    request_id: u64,
    implementation: Digest,
    context: Digest,
    execution: child::Execution<T>,
}
impl<T> LocalExecution<T> {
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn implementation(&self) -> Digest {
        self.implementation
    }
    pub fn context(&self) -> Digest {
        self.context
    }
    pub fn execution(&self) -> &child::Execution<T> {
        &self.execution
    }
    pub fn into_execution(self) -> child::Execution<T> {
        self.execution
    }
}

impl<'request, 'budget> IssuedInvocation<'request, 'budget> {
    /// The saved Invoke's Limits are the relative remote quota. The caller
    /// selects it before admission, within the parent/ancestor capacity. Depth
    /// remains an absolute peak. Validation uses a separate finite host budget.
    pub fn issue(
        authorized: AuthorizedInvoke<'request>,
        implementation: Digest,
        registry: &SchemaRegistry,
        parent: &'budget mut Budget,
        validation: &mut Budget,
    ) -> Result<Self, Error> {
        parent.poll().map_err(Error::Stopped)?;
        let context =
            validation_context(authorized.request(), implementation, registry, validation)?;
        Self::reserve(authorized, implementation, context, parent)
    }

    /// Validate input and compute context on the actual parent Budget before
    /// reserving child capacity. Successful charges remain on error; rejected
    /// charges are not added, and resource stops remain sticky. Validation errors
    /// create no outstanding reservation or reservation-derived cancellation.
    ///
    /// Validation may leave insufficient capacity for the unchanged child grant.
    /// It uses the parent's current depth and retains historical peak; successful
    /// issuance does not bypass the later local child's historical-depth check.
    /// Context hashing keeps its distinct source-admission ledger and charges it
    /// here. Grants construction/admission and other host work remain caller work.
    pub fn issue_with_parent_validation(
        authorized: AuthorizedInvoke<'request>,
        implementation: Digest,
        registry: &SchemaRegistry,
        parent: &'budget mut Budget,
    ) -> Result<Self, Error> {
        parent.poll().map_err(Error::Stopped)?;
        let context = validation_context(authorized.request(), implementation, registry, parent)?;
        Self::reserve(authorized, implementation, context, parent)
    }

    fn reserve(
        authorized: AuthorizedInvoke<'request>,
        implementation: Digest,
        context: Digest,
        parent: &'budget mut Budget,
    ) -> Result<Self, Error> {
        let budget =
            IssuedBudget::issue(parent, authorized.request().limits).map_err(Error::Stopped)?;
        Ok(Self {
            authorized,
            implementation,
            context,
            budget,
        })
    }

    pub fn request(&self) -> &Invoke {
        self.authorized.request()
    }
    pub fn context(&self) -> Digest {
        self.context
    }
    pub fn parent_usage(&self) -> Usage {
        self.budget.parent_usage()
    }

    /// Execute trusted local work while withholding remote capacity. Transport
    /// errors can be returned as the closure's value and handled by the host;
    /// dropping this outstanding invocation cancels its accounting reservation.
    pub fn run_local<T>(
        &mut self,
        operation: impl FnOnce(&mut Budget) -> Result<T, StopReason>,
    ) -> Result<T, LocalFailure<T>> {
        self.budget.run_local(operation)
    }

    /// Execute a trusted terminal callback in an explicitly separate admission
    /// context. Its Budget contains the actual parent's cumulative history,
    /// whereas the immutable Invoke retains relative additive grants. Callbacks
    /// must use the supplied Budget rather than reconstruct it from request.limits.
    /// The host still matches this Invoke to the intended saved Reader/session
    /// and supplies its authorized source scope. Binding the Invoke alone does
    /// not perform that mapping or validate a nested Reader source table.
    /// No common dispatcher or remote accounting protocol is implied.
    pub fn execute_local_child<T>(
        self,
        saved_depth: u64,
        operation: impl FnOnce(&Invoke, &mut Budget, &mut SourceAdmission) -> T,
    ) -> Result<LocalExecution<T>, child::Failure<T>> {
        let request = self.authorized.request();
        let execution = self
            .budget
            .execute_local_child(saved_depth, |b, a| operation(request, b, a))?;
        Ok(LocalExecution {
            request_id: request.request_id,
            implementation: self.implementation,
            context: self.context,
            execution,
        })
    }

    /// Compare an independently associated observed request to the complete
    /// saved Invoke, then settle its trusted terminal observation. Comparison
    /// uses the actual parent's unreserved local capacity; failed comparisons
    /// retain charges and drop the unsettled reservation (cancelling its parent).
    /// Unlike `settle`, this entry cannot validate after the parent has stopped.
    ///
    /// Request equality does not authenticate a received measurement or identify
    /// a distinct execution attempt. The host still establishes that provenance,
    /// terminal cleanup and same-attempt association independently. Source and
    /// resource table order is significant; context_digest remains unchanged.
    pub fn settle_saved_request(
        mut self,
        observed_request: &Invoke,
        implementation: Digest,
        context: Digest,
        observed: Usage,
    ) -> Result<(), SavedSettlementError> {
        let saved = self.authorized.request();
        self.budget
            .run_local(|b| Ok(observed_request.check_saved(saved, b)))
            .map_err(|failure| SavedSettlementError::Stopped(failure.reason))?
            .map_err(SavedSettlementError::RequestBinding)?;
        self.settle(
            observed_request.request_id,
            implementation,
            context,
            observed,
        )
        .map_err(SavedSettlementError::Settlement)
    }

    /// Settle the measurement of this exact host-issued request. Identity
    /// matching prevents accidental misrouting; it does not authenticate a
    /// peer's claims. Call only after the host has independently established the
    /// observation's provenance and completion. The grant is consumed once.
    pub fn settle(
        self,
        request_id: u64,
        implementation: Digest,
        context: Digest,
        observed: Usage,
    ) -> Result<(), Error> {
        if request_id != self.request().request_id
            || implementation != self.implementation
            || context != self.context
        {
            return Err(Error::ObservationBinding);
        }
        self.budget.settle(observed).map_err(Error::Settlement)
    }
}

fn validation_context(
    request: &Invoke,
    implementation: Digest,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Digest, Error> {
    request
        .validate_input(&request.operation, registry, budget)
        .map_err(Error::Input)?;
    nepl3_wire::operation::context_digest(request, implementation, registry, budget)
        .map_err(Error::Context)
}
