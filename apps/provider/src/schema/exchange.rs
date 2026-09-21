use super::*;
use crate::{Connection, TransportError};
use nepl3_core::{
    operation::ProviderFrame,
    source::{SourceAdmission, SourceStore},
};
use std::io::{Read, Write};

#[derive(Debug)]
pub enum SchemaExchangeError {
    Transport(TransportError),
    Admission(SchemaAdmissionError),
    UnexpectedFrame,
}
impl From<TransportError> for SchemaExchangeError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}
impl From<SchemaAdmissionError> for SchemaExchangeError {
    fn from(error: SchemaAdmissionError) -> Self {
        Self::Admission(error)
    }
}
impl From<StopReason> for SchemaExchangeError {
    fn from(error: StopReason) -> Self {
        Self::Admission(error.into())
    }
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

impl<R: Read, W: Write> Connection<R, W> {
    /// Run the host side of the one-shot setup exchange before any operation.
    /// `expected` is trusted host policy. Any failure closes this connection.
    /// Blocking I/O interruption remains the stream/process owner's duty.
    pub fn request_schemas(
        &mut self,
        bootstrap: SchemaRegistry,
        expected: &[SchemaRef],
        budget: &mut Budget,
    ) -> Result<SchemaRegistry, SchemaExchangeError> {
        let result = (|| {
            let mut schemas = Vec::new();
            reserve(&mut schemas, expected.len(), budget)?;
            for identity in expected {
                budget.charge(Resource::AllocationUnits, identity.package.len() as u64)?;
                budget.charge(Resource::Work, identity.package.len() as u64 + 40)?;
                schemas.push(identity.clone());
            }
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            self.send(
                &ProviderFrame::SchemaRequest { schemas },
                &bootstrap,
                &sources,
                &mut admission,
                budget,
            )?;
            let Some(ProviderFrame::SchemaReply { descriptors }) =
                self.receive(&bootstrap, &sources, &mut admission, budget)?
            else {
                return Err(SchemaExchangeError::UnexpectedFrame);
            };
            let mut views = Vec::new();
            reserve(&mut views, descriptors.len(), budget)?;
            for descriptor in &descriptors {
                budget.charge(Resource::Work, 1)?;
                views.push(descriptor.as_slice());
            }
            Ok(admit(bootstrap, expected, &views, budget)?)
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }

    /// Serve only exact identities from the host-selected finalized catalogue.
    /// No implementation or capability is installed by sending a descriptor.
    pub fn serve_schemas(
        &mut self,
        catalog: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), SchemaExchangeError> {
        let result = (|| {
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let Some(ProviderFrame::SchemaRequest { schemas }) =
                self.receive(catalog, &sources, &mut admission, budget)?
            else {
                return Err(SchemaExchangeError::UnexpectedFrame);
            };
            let mut descriptors = Vec::new();
            reserve(&mut descriptors, schemas.len(), budget)?;
            for (index, identity) in schemas.iter().enumerate() {
                for prior in &schemas[..index] {
                    budget.charge(
                        Resource::Work,
                        (identity.package.len() as u64)
                            .saturating_add(prior.package.len() as u64)
                            .saturating_add(8),
                    )?;
                    if identity.package == prior.package && identity.revision == prior.revision {
                        return Err(SchemaAdmissionError::DuplicateSelection.into());
                    }
                }
                let descriptor = catalog
                    .descriptor(identity)
                    .ok_or(SchemaAdmissionError::Schema(SchemaError::UnknownSchema))?;
                descriptors.push(
                    nepl3_wire::schema::encode(descriptor, catalog, budget)
                        .map_err(SchemaAdmissionError::from)?,
                );
            }
            self.send(
                &ProviderFrame::SchemaReply { descriptors },
                catalog,
                &sources,
                &mut admission,
                budget,
            )?;
            Ok(())
        })();
        if result.is_err() {
            self.closed = true;
        }
        result
    }
}
