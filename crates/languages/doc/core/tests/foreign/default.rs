//! Test provider that deliberately uses the trait's unfused default method.
use nepl3_core::{
    budget::Budget,
    origin::Origin,
    source::{Digest, SourceSnapshot, Span},
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, SchemaRef},
    value_codec::{CanonicalDigestInput, FoundationValueCodec},
    view::ViewBundle,
};
pub(super) enum ArchiveComponents {
    Exact,
    Short,
    Long,
}
pub(super) struct DefaultCodec<C>(pub C, pub ArchiveComponents);
impl<C> DefaultCodec<C> {
    pub(super) fn new(inner: C) -> Self {
        Self(inner, ArchiveComponents::Exact)
    }
}
impl<C: FoundationValueCodec> FoundationValueCodec for DefaultCodec<C> {
    type Error = C::Error;
    fn canonical_value_digest(
        &mut self,
        domain: &[u8],
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Digest, Self::Error> {
        self.0.canonical_value_digest(domain, value, budget)
    }
    fn canonical_value_digests(
        &mut self,
        inputs: &[CanonicalDigestInput<'_>],
        budget: &mut Budget,
    ) -> Result<Vec<Digest>, Self::Error> {
        self.0.canonical_value_digests(inputs, budget)
    }
    fn encode_type_descriptor(
        &mut self,
        value: &nepl3_core::schema::TypeDescriptor,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_type_descriptor(value, budget)
    }
    fn decode_type_descriptor(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::schema::TypeDescriptor, Self::Error> {
        self.0.decode_type_descriptor(value, budget)
    }
    fn foundation_schema(&self) -> &SchemaRef {
        self.0.foundation_schema()
    }
    fn source_admission(&mut self) -> &mut nepl3_core::source::SourceAdmission {
        self.0.source_admission()
    }
    fn encode_fact_set(
        &mut self,
        value: &nepl3_core::facts::FactSet,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_fact_set(value, budget)
    }
    fn decode_fact_set(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::facts::FactSet, Self::Error> {
        self.0.decode_fact_set(value, budget)
    }
    fn encode_fact_delta(
        &mut self,
        value: &nepl3_core::facts::FactDelta,
        base: &nepl3_core::facts::CheckedFactSet<'_>,
        authority: &nepl3_core::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_fact_delta(value, base, authority, budget)
    }
    fn decode_fact_delta(
        &mut self,
        value: &NdfValue,
        base: &nepl3_core::facts::CheckedFactSet<'_>,
        authority: &nepl3_core::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<nepl3_core::facts::FactDelta, Self::Error> {
        self.0.decode_fact_delta(value, base, authority, budget)
    }
    fn encode_fact_authority(
        &mut self,
        value: &nepl3_core::facts::FactAuthority,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_fact_authority(value, budget)
    }
    fn decode_fact_authority(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::facts::FactAuthority, Self::Error> {
        self.0.decode_fact_authority(value, budget)
    }
    fn encode_reference_resolution(
        &mut self,
        value: &nepl3_core::facts::ReferenceResolution,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_reference_resolution(value, budget)
    }
    fn decode_reference_resolution(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::facts::ReferenceResolution, Self::Error> {
        self.0.decode_reference_resolution(value, budget)
    }
    fn encode_mappings(
        &mut self,
        value: &[nepl3_core::origin::Mapping],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_mappings(value, budget)
    }
    fn decode_mappings(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<nepl3_core::origin::Mapping>, Self::Error> {
        self.0.decode_mappings(value, budget)
    }
    fn scoped<'a>(
        &'a mut self,
        sources: &'a nepl3_core::source::SourceStore,
    ) -> impl FoundationValueCodec<Error = Self::Error> + 'a {
        DefaultCodec::new(self.0.scoped(sources))
    }
    fn scoped_with_admission<'a>(
        &'a mut self,
        sources: &'a nepl3_core::source::SourceStore,
        admission: &'a mut nepl3_core::source::SourceAdmission,
    ) -> impl FoundationValueCodec<Error = Self::Error> + 'a {
        DefaultCodec::new(self.0.scoped_with_admission(sources, admission))
    }
    fn scoped_with_mappings<'a>(
        &'a mut self,
        sources: &'a nepl3_core::source::SourceStore,
        mappings: &'a [nepl3_core::origin::Mapping],
    ) -> impl FoundationValueCodec<Error = Self::Error> + 'a {
        DefaultCodec::new(self.0.scoped_with_mappings(sources, mappings))
    }
    fn encode_syntax(
        &mut self,
        value: &nepl3_core::syntax::SyntaxBundle,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_syntax(value, budget)
    }
    fn encode_foreign_closure(
        &mut self,
        value: &nepl3_core::syntax::ForeignClosure,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_foreign_closure(value, budget)
    }
    fn encode_foreign_view(
        &mut self,
        value: &nepl3_core::syntax::ForeignClosureView<'_>,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_foreign_view(value, budget)
    }
    fn decode_foreign_closure(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::syntax::ForeignClosure, Self::Error> {
        self.0.decode_foreign_closure(value, budget)
    }
    fn encode_foreign_group(
        &mut self,
        value: &nepl3_core::syntax::group::ForeignClosureGroup,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_foreign_group(value, budget)
    }
    fn decode_foreign_group(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::syntax::group::ForeignClosureGroup, Self::Error> {
        self.0.decode_foreign_group(value, budget)
    }
    fn foreign_group_entry_digests(
        &mut self,
        value: &nepl3_core::syntax::group::ForeignClosureGroup,
        budget: &mut Budget,
    ) -> Result<Vec<Digest>, Self::Error> {
        self.0.foreign_group_entry_digests(value, budget)
    }
    fn decode_syntax(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::syntax::SyntaxBundle, Self::Error> {
        self.0.decode_syntax(value, budget)
    }
    fn encode_report(
        &mut self,
        value: &nepl3_core::diagnostic::Report,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_report(value, budget)
    }
    fn decode_report(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::diagnostic::Report, Self::Error> {
        self.0.decode_report(value, budget)
    }
    fn encode_origins(
        &mut self,
        value: &[Origin],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_origins(value, budget)
    }
    fn decode_origins(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<Origin>, Self::Error> {
        self.0.decode_origins(value, budget)
    }
    fn admit_source(
        &mut self,
        value: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), Self::Error> {
        self.0.admit_source(value, budget)
    }
    fn environment_digest(
        &mut self,
        value: &Environment,
        budget: &mut Budget,
    ) -> Result<Digest, Self::Error> {
        self.0.environment_digest(value, budget)
    }
    fn encode_environment(
        &mut self,
        value: &EnvironmentEntry,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_environment(value, budget)
    }
    fn decode_environment(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<EnvironmentEntry, Self::Error> {
        self.0.decode_environment(value, budget)
    }
    fn encode_sources(
        &mut self,
        value: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_sources(value, budget)
    }
    fn decode_sources(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<SourceSnapshot>, Self::Error> {
        self.0.decode_sources(value, budget)
    }
    fn encode_span(&mut self, value: &Span, budget: &mut Budget) -> Result<NdfValue, Self::Error> {
        self.0.encode_span(value, budget)
    }
    fn decode_span(&mut self, value: &NdfValue, budget: &mut Budget) -> Result<Span, Self::Error> {
        self.0.decode_span(value, budget)
    }
    fn encode_views(
        &mut self,
        value: &ViewBundle,
        budget: &mut Budget,
    ) -> Result<NdfValue, Self::Error> {
        self.0.encode_views(value, budget)
    }
    fn decode_views(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<ViewBundle, Self::Error> {
        self.0.decode_views(value, budget)
    }
}
