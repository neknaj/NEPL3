//! Native Doc reader dispatch for an explicitly identified host implementation.
//! This services the existing typed engine boundary without exporting a copy of
//! the growing parser state for every synchronous reader call.
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceReservation, SourceStore},
};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::{ProviderCall, ReadRequest},
    runtime::ProviderReply,
    tokenizer::ReservationRequest,
};
use nepl3_wire::foundation::FoundationCodec;

pub struct NativeHost<'a> {
    registry: &'a SchemaRegistry,
    providers: Vec<ProviderImplementation>,
    reservation_prefix: String,
    reservation_id: u64,
}
impl<'a> NativeHost<'a> {
    /// `implementation` identifies the actual executable/worker containing these
    /// providers; the caller must use these registrations when resolving its
    /// Profile. `reservation_prefix` is host-reserved for this operation only.
    pub fn new(
        registry: &'a SchemaRegistry,
        implementation: Digest,
        reservation_prefix: String,
        budget: &mut Budget,
    ) -> Result<Self, ParseError> {
        budget.charge(
            Resource::AllocationUnits,
            4 * core::mem::size_of::<nepl3_core::value::OperationRef>() as u64,
        )?;
        let mut operations = Vec::with_capacity(4);
        for kind in [
            BuiltinReader::Name,
            BuiltinReader::Number,
            BuiltinReader::Trivia,
        ] {
            operations.push(provider::operation(kind, registry, budget)?);
        }
        operations.push(super::reader::signature(registry, budget)?.operation);
        budget.charge(
            Resource::AllocationUnits,
            4 * core::mem::size_of::<ProviderImplementation>() as u64,
        )?;
        let mut providers = Vec::with_capacity(4);
        for operation in operations {
            budget.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<nepl3_core::value::OperationRef>() + operation.name.len())
                    as u64,
            )?;
            providers.push(ProviderImplementation {
                provider: operation.name.clone(),
                revision: 1,
                implementation_digest: implementation,
                operations: vec![operation],
            });
        }
        Ok(Self {
            registry,
            providers,
            reservation_prefix,
            reservation_id: 0,
        })
    }
    pub fn providers(&self) -> &[ProviderImplementation] {
        &self.providers
    }
}
impl ParseHost for NativeHost<'_> {
    fn provider(
        &mut self,
        call: &ProviderCall,
        requirement: &ProviderRequirement,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ParseError> {
        let ProviderCall::Read {
            operation, request, ..
        } = call
        else {
            return Ok(None);
        };
        budget.charge(
            Resource::Work,
            self.providers.len() as u64
                * (requirement.provider.len()
                    + operation.name.len()
                    + operation.schema.package.len()
                    + 66) as u64,
        )?;
        if requirement.operation != *operation
            || !self.providers.iter().any(|p| {
                p.provider == requirement.provider
                    && p.revision == requirement.revision
                    && p.implementation_digest == requirement.implementation_digest
                    && p.operations.contains(operation)
            })
        {
            return Err(ParseError::Context);
        }
        let mut sources = SourceStore::default();
        for source in &request.sources {
            sources.insert_with_budget(source.clone_with_budget(budget)?, budget)?;
        }
        let snapshot = sources
            .resolve(&request.snapshot)
            .ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
        let mut codec = FoundationCodec::new(self.registry, &sources, admission).map_err(|_| {
            budget
                .poll()
                .err()
                .map(ParseError::Stopped)
                .unwrap_or(ParseError::Context)
        })?;
        let context = request
            .context
            .check(&mut codec, &sources, self.registry, budget)
            .map_err(|_| {
                budget
                    .poll()
                    .err()
                    .map(ParseError::Stopped)
                    .unwrap_or(ParseError::Context)
            })?;
        let read = if operation.schema.package == "nepl3.doc.reader" {
            super::reader::read
        } else {
            provider::read
        };
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<nepl3_reader::model::ReadReply>() as u64,
        )?;
        let reply = read(
            operation,
            ReadRequest {
                snapshot,
                start: request.start,
                limit: request.limit,
                final_input: request.final_input,
                context: &context,
                state: &request.state,
            },
            self.registry,
            &sources,
            budget,
            admission,
        )?;
        Ok(Some(ProviderReply::Read(Box::new(reply))))
    }
    fn reservation(
        &mut self,
        _: &ReservationRequest,
        budget: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ParseError> {
        budget.charge(Resource::Work, self.reservation_prefix.len() as u64 + 21)?;
        budget.charge(
            Resource::AllocationUnits,
            (self.reservation_prefix.len() as u64 + 21) * 2 + 7,
        )?;
        self.reservation_id = self
            .reservation_id
            .checked_add(1)
            .ok_or(ParseError::Reference)?;
        let id = format!("{}{}", self.reservation_prefix, self.reservation_id);
        Ok(Some(SourceReservation {
            source_id: nepl3_core::source::SourceId(id.clone()),
            revision: 0,
            uri: format!("memory:{id}"),
        }))
    }
}
