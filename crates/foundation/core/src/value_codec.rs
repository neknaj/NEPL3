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

/// One domain-separated digest of an immutably borrowed value. These references
/// are local API inputs; their addresses are not part of a portable identity.
pub struct CanonicalDigestInput<'a> {
    pub domain: &'a [u8],
    pub value: &'a NdfValue,
}

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
    /// Return the same digests as independent calls, in request order. An
    /// implementation may encode an enclosing value and requested descendants
    /// in one traversal. Charge each actual encoding once, each hash's bytes,
    /// temporary storage, and each 32-byte result. Failure returns no partial
    /// result and never refunds consumed resources. Earlier enclosing inputs
    /// permit sharing with later descendant inputs; disjoint inputs also work.
    fn canonical_value_digests(
        &mut self,
        inputs: &[CanonicalDigestInput<'_>],
        budget: &mut Budget,
    ) -> Result<Vec<Digest>, Self::Error>;
    /// Encode a symbolic type description as foundation data. This does not
    /// assert that a described Named type resolves in the current registry.
    fn encode_type_descriptor(
        &mut self,
        value: &crate::schema::TypeDescriptor,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    /// Decode symbolic type data after checking the registered foundation schema.
    /// Named references retain their identity; resolution belongs to the caller.
    fn decode_type_descriptor(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::schema::TypeDescriptor, Self::Error>;
    /// Exchange a descriptor as data after canonical name/shape validation.
    /// This neither registers it nor resolves its symbolic dependencies.
    fn encode_schema_descriptor(
        &mut self,
        value: &crate::schema::SchemaDescriptor,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    /// Check the registered foundation representation, canonical descriptor
    /// invariants and full host-selected identity. Success does not grant trust,
    /// registry membership, execution permissions or resource authority.
    fn decode_schema_descriptor(
        &mut self,
        value: &NdfValue,
        expected: &SchemaRef,
        budget: &mut Budget,
    ) -> Result<crate::schema::SchemaDescriptor, Self::Error>;
    /// Symbolic foundation data only: these adapters check shape and nominal
    /// ownership, without resolving target schemas or granting execution authority.
    fn encode_kind_ref(
        &mut self,
        value: &crate::value::KindRef,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_kind_ref(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::value::KindRef, Self::Error>;
    fn encode_operation_ref(
        &mut self,
        value: &crate::value::OperationRef,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_operation_ref(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::value::OperationRef, Self::Error>;
    fn encode_presentation_class(
        &mut self,
        value: &crate::view::PresentationClass,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_presentation_class(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::view::PresentationClass, Self::Error>;
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
    /// Rebind explicit sources and a distinct operation's admission ledger.
    /// This scope clears mapping authority and validation caches, never borrowing
    /// or transferring source charges from the parent codec's admission. Pair the
    /// supplied admission with that operation's Budget; source declarations alone
    /// do not grant authority. The parent admission remains unchanged.
    fn scoped_with_admission<'a>(
        &'a mut self,
        sources: &'a crate::source::SourceStore,
        admission: &'a mut crate::source::SourceAdmission,
    ) -> impl FoundationValueCodec<Error = Self::Error> + 'a;
    /// Rebind sources and the explicit mapping closure used to validate Views.
    /// Mapping endpoints and geometry must be checked against these sources.
    /// Ordinary `scoped` clears this mapping context rather than inheriting it.
    fn scoped_with_mappings<'a>(
        &'a mut self,
        sources: &'a crate::source::SourceStore,
        mappings: &'a [crate::origin::Mapping],
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
    /// Encode a borrowed closure with the same validation and wire layout as
    /// an owned ForeignClosure, without materializing duplicate owner data.
    fn encode_foreign_view(
        &mut self,
        value: &crate::syntax::ForeignClosureView<'_>,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_foreign_closure(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::syntax::ForeignClosure, Self::Error>;
    /// Closed shared ownership tables; identity and source closure remain explicit.
    /// Successful decoding does not prove consuming-language occurrence depths.
    fn encode_foreign_group(
        &mut self,
        value: &crate::syntax::group::ForeignClosureGroup,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error>;
    fn decode_foreign_group(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<crate::syntax::group::ForeignClosureGroup, Self::Error>;
    /// Versioned component identities in entry order. Validate and encode under
    /// the current registry/admission/Budget, then hash each owner component once.
    /// Group-local indices are excluded; explicit ownership is never inferred
    /// from equal digests. These identities do not prove consumer occurrence depth.
    /// Framing is specified by the shared foreign owner contract, not the legacy
    /// flat ForeignClosure digest or a consuming operation's domain.
    fn foreign_group_entry_digests(
        &mut self,
        value: &crate::syntax::group::ForeignClosureGroup,
        budget: &mut Budget,
    ) -> Result<Vec<Digest>, Self::Error>;
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
