//! Request-bound host reservation for an explicitly selected provider.
use nepl3_core::{
    budget::{Budget, StopReason, Usage},
    operation::{Invoke, request::InputValidationError},
    schema::SchemaRegistry,
    source::Digest,
};
use nepl3_suite::{
    grants::AuthorizedInvoke,
    suspension::delegation::{IssuedBudget, SettlementError},
};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Input(InputValidationError),
    Context(nepl3_wire::WireError),
    ObservationBinding,
    Settlement(SettlementError),
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
        let request = authorized.request();
        request
            .validate_input(&request.operation, registry, validation)
            .map_err(Error::Input)?;
        let context =
            nepl3_wire::operation::context_digest(request, implementation, registry, validation)
                .map_err(Error::Context)?;
        let budget = IssuedBudget::issue(parent, request.limits).map_err(Error::Stopped)?;
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
    ) -> Result<T, StopReason> {
        self.budget.run_local(operation)
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
