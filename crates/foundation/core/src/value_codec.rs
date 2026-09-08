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
pub trait FoundationCodecError {
    /// Extract the original stop even when typed Source/Schema/Origin/View or
    /// another boundary cause wraps it. An unrelated Budget stop must not turn
    /// an ordinary validation error into a stopped result.
    fn stop_reason(&self) -> Option<crate::budget::StopReason>;
}
pub trait FoundationValueCodec {
    type Error: FoundationCodecError;
    /// SHA-256 of `domain || canonical NDF/1 CBOR(value)`. This checks intrinsic
    /// value invariants; the operation owner validates its expected schema first.
    /// Encoding and hashing consume this operation's Budget. No source authority
    /// is inferred from records contained in the value.
    /// A streaming implementation may feed canonical chunks directly into the
    /// hash without materializing a CBOR buffer. It charges encoding and hashing
    /// Work, traversal storage and the 32-byte digest output. If an implementation
    /// materializes CBOR bytes, it must also account for that intermediate output
    /// and storage; it cannot simply omit their charges.
    fn canonical_value_digest(
        &mut self,
        domain: &[u8],
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Digest, Self::Error>;
    /// Encode a symbolic type description as foundation data. This does not
    /// assert that a described Named type resolves in the current registry.
    fn encode_type_descriptor(
        &mut self,
        value: &crate::schema::TypeDescriptor,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn foundation_schema(&self) -> &SchemaRef;
    fn source_admission(&mut self) -> &mut crate::source::SourceAdmission;
    fn encode_fact_set(
        &mut self,
        value: &crate::facts::FactSet,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_fact_set(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::facts::FactSet, Self::Error>;
    fn encode_fact_delta(
        &mut self,
        value: &crate::facts::FactDelta,
        base: &crate::facts::CheckedFactSet<'_>,
        authority: &crate::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_fact_delta(
        &mut self,
        value: &NdfValue,
        base: &crate::facts::CheckedFactSet<'_>,
        authority: &crate::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<crate::facts::FactDelta, Self::Error>;
    /// Structural representation only. The operation owner must compare this
    /// value with the authority actually issued by its host.
    fn encode_fact_authority(
        &mut self,
        value: &crate::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_fact_authority(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::facts::FactAuthority, Self::Error>;
    /// Structural data only; membership and authorized resolution transitions
    /// are checked against the enclosing facts operation.
    fn encode_reference_resolution(
        &mut self,
        value: &crate::facts::ReferenceResolution,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_reference_resolution(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::facts::ReferenceResolution, Self::Error>;
    fn encode_mappings(
        &mut self,
        value: &[crate::origin::Mapping],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_mappings(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<crate::origin::Mapping>, Self::Error>;
    /// Rebind source resolution while borrowing the same registry/admission.
    /// A caller must supply the operation's explicit declaration closure.
    fn scoped<'a>(
        &'a mut self,
        sources: &'a crate::source::SourceStore,
    ) -> impl FoundationValueCodec<Error = Self::Error> + 'a;
    fn encode_syntax(
        &mut self,
        value: &crate::syntax::SyntaxBundle,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn encode_foreign_closure(
        &mut self,
        value: &crate::syntax::ForeignClosure,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_foreign_closure(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::syntax::ForeignClosure, Self::Error>;
    fn decode_syntax(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::syntax::SyntaxBundle, Self::Error>;
    fn encode_report(
        &mut self,
        value: &crate::diagnostic::Report,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_report(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::diagnostic::Report, Self::Error>;
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
