//! Concrete host boundary used by crates that depend on core rather than wire.
use crate::{WireError, environment::*, source::*, view::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    source::{Digest, SourceAdmission, SourceSnapshot, SourceStore, Span},
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, SchemaRef},
    value_codec::FoundationValueCodec,
    view::ViewBundle,
};

pub struct FoundationCodec<'a> {
    schema: &'a SchemaRef,
    registry: &'a SchemaRegistry,
    sources: &'a SourceStore,
    admission: &'a mut SourceAdmission,
}
impl<'a> FoundationCodec<'a> {
    pub fn new(
        registry: &'a SchemaRegistry,
        sources: &'a SourceStore,
        admission: &'a mut SourceAdmission,
    ) -> Result<Self, WireError> {
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        let schema = registry
            .selected("nepl3.foundation", 1)
            .ok_or(SchemaError::UnknownSchema)?;
        Ok(Self {
            schema,
            registry,
            sources,
            admission,
        })
    }
    fn validate(&self, value: &NdfValue, name: &str, budget: &mut Budget) -> Result<(), WireError> {
        budget.charge(
            Resource::AllocationUnits,
            ("nepl3.foundation".len() + name.len()) as u64,
        )?;
        self.registry.validate(&expected(name), value, budget)?;
        Ok(())
    }
    fn span_admission(&mut self, span: &Span, budget: &mut Budget) -> Result<(), WireError> {
        budget.charge(
            Resource::AllocationUnits,
            span.snapshot_ref().source.0.len() as u64,
        )?;
        let source = self
            .sources
            .get(span.snapshot())
            .ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
        self.admission.admit_existing(source, budget)?;
        source.slice(span)?;
        Ok(())
    }
}
impl FoundationValueCodec for FoundationCodec<'_> {
    type Error = WireError;
    fn encode_report(
        &mut self,
        value: &nepl3_core::diagnostic::Report,
        budget: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        for source in self.sources.snapshots() {
            self.admission.admit_existing(source, budget)?;
        }
        let value =
            crate::report::report_value(value, self.schema, self.registry, self.sources, budget)?;
        self.validate(&value, "Report", budget)?;
        Ok(value)
    }
    fn decode_report(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<nepl3_core::diagnostic::Report, WireError> {
        self.validate(value, "Report", budget)?;
        for source in self.sources.snapshots() {
            self.admission.admit_existing(source, budget)?;
        }
        crate::report::report_from(value, self.schema, self.registry, self.sources, budget)
    }
    fn encode_origins(
        &mut self,
        value: &[nepl3_core::origin::Origin],
        budget: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        nepl3_core::origin::OriginGraph::validate_origins(value, self.sources, budget)?;
        crate::boundary::sequence(value, budget, |value, budget| {
            crate::origin::origin_value(value, self.schema, budget)
        })
    }
    fn decode_origins(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<nepl3_core::origin::Origin>, WireError> {
        let origins = collect(list(value)?, budget, |value, budget| {
            crate::origin::origin_from(value, self.schema, self.sources, budget)
        })?;
        nepl3_core::origin::OriginGraph::validate_origins(&origins, self.sources, budget)?;
        Ok(origins)
    }
    fn foundation_schema(&self) -> &SchemaRef {
        self.schema
    }
    fn admit_source(
        &mut self,
        value: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), WireError> {
        self.admission.admit_existing(value, budget)?;
        Ok(())
    }
    fn environment_digest(
        &mut self,
        value: &Environment,
        budget: &mut Budget,
    ) -> Result<Digest, WireError> {
        environment_digest(value, self.schema, self.registry, budget)
    }
    fn encode_environment(
        &mut self,
        value: &EnvironmentEntry,
        budget: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let value = entry_value(value, self.schema, self.registry, budget)?;
        self.validate(&value, "EnvironmentEntry", budget)?;
        Ok(value)
    }
    fn decode_environment(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<EnvironmentEntry, WireError> {
        self.validate(value, "EnvironmentEntry", budget)?;
        entry_from(value, self.schema, self.registry, budget)
    }
    fn encode_sources(
        &mut self,
        value: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let value = sources_value(value, self.schema, self.admission, budget)?;
        self.registry.validate(
            &TypeDescriptor::List(alloc::boxed::Box::new(expected("SourceContent"))),
            &value,
            budget,
        )?;
        Ok(value)
    }
    fn decode_sources(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<Vec<SourceSnapshot>, WireError> {
        self.registry.validate(
            &TypeDescriptor::List(alloc::boxed::Box::new(expected("SourceContent"))),
            value,
            budget,
        )?;
        sources_from(value, self.schema, self.admission, budget)
    }
    fn encode_span(&mut self, value: &Span, budget: &mut Budget) -> Result<NdfValue, WireError> {
        self.span_admission(value, budget)?;
        let value = span_value(value, self.schema, budget)?;
        self.validate(&value, "Span", budget)?;
        Ok(value)
    }
    fn decode_span(&mut self, value: &NdfValue, budget: &mut Budget) -> Result<Span, WireError> {
        self.validate(value, "Span", budget)?;
        let value = span_from_value(value, self.schema, self.sources, budget)?;
        self.span_admission(&value, budget)?;
        Ok(value)
    }
    fn encode_views(
        &mut self,
        value: &ViewBundle,
        budget: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        for view in &value.elements {
            self.span_admission(&view.span, budget)?;
        }
        value.validate(self.sources, self.registry, budget)?;
        let value = views_value(value, self.schema, budget)?;
        self.validate(&value, "ViewBundle", budget)?;
        Ok(value)
    }
    fn decode_views(
        &mut self,
        value: &NdfValue,
        budget: &mut Budget,
    ) -> Result<ViewBundle, WireError> {
        self.validate(value, "ViewBundle", budget)?;
        let value = views_from(value, self.schema, self.sources, budget)?;
        for view in &value.elements {
            self.span_admission(&view.span, budget)?;
        }
        value.validate(self.sources, self.registry, budget)?;
        Ok(value)
    }
}
