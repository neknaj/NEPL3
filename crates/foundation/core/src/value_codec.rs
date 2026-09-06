//! Host-selected typed value boundary. Normal reader execution does not serialize.
use crate::origin::Origin;
use crate::{
    budget::Budget,
    source::{Digest, SourceSnapshot, Span},
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, SchemaRef},
    view::ViewBundle,
};
use alloc::vec::Vec;

/// Implementations must enforce the registered foundation schema and its semantic
/// source/view/environment constraints. This is not a provider-supplied callback.
pub trait FoundationValueCodec {
    type Error;
    fn foundation_schema(&self) -> &SchemaRef;
    fn encode_origins(
        &mut self,
        value: &[Origin],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_origins(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<Origin>, Self::Error>;
    fn admit_source(
        &mut self,
        value: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), Self::Error>;
    fn environment_digest(
        &mut self,
        value: &Environment,
        budget: &mut Budget,
    ) -> Result<Digest, Self::Error>;
    fn encode_environment(
        &mut self,
        value: &EnvironmentEntry,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_environment(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<EnvironmentEntry, Self::Error>;
    fn encode_sources(
        &mut self,
        value: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_sources(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<SourceSnapshot>, Self::Error>;
    fn encode_span(&mut self, value: &Span, budget: &mut Budget) -> Result<NdfValue, Self::Error>;
    fn decode_span(&mut self, value: &NdfValue, budget: &mut Budget) -> Result<Span, Self::Error>;
    fn encode_views(
        &mut self,
        value: &ViewBundle,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_views(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<ViewBundle, Self::Error>;
}
