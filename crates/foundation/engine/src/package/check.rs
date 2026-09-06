use super::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{OriginError, OriginGraph, SourceMap},
    schema::{SchemaError, SchemaRegistry},
    source::{SourceAdmission, SourceError, SourceStore},
};
use nepl3_reader::{
    plan::{CheckedPlan, PlanError},
    tokenizer::TokenReader,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageError {
    Stopped(StopReason),
    Schema(SchemaError),
    Reader(PlanError),
    Source(SourceError),
    Origin(OriginError),
    EmptyName,
    DuplicateName,
    MissingMode,
    MissingCategory,
    MissingReader,
    MissingNamespace,
    MissingExtension,
    SignatureMismatch,
    InvalidRead,
    InvalidBinding,
    DirectCycle,
    KindShape,
    InvalidSelector,
    UnvisitedField,
    Provenance,
}
impl From<StopReason> for PackageError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<SchemaError> for PackageError {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::Stopped(e) => Self::Stopped(e),
            e => Self::Schema(e),
        }
    }
}
impl From<PlanError> for PackageError {
    fn from(e: PlanError) -> Self {
        match e {
            PlanError::Stopped(e) => Self::Stopped(e),
            e => Self::Reader(e),
        }
    }
}
impl From<SourceError> for PackageError {
    fn from(e: SourceError) -> Self {
        match e {
            SourceError::Stopped(e) => Self::Stopped(e),
            e => Self::Source(e),
        }
    }
}
impl From<OriginError> for PackageError {
    fn from(e: OriginError) -> Self {
        match e {
            OriginError::Stopped(e) => Self::Stopped(e),
            e => Self::Origin(e),
        }
    }
}

/// Proves local metadata references and declared shape compatibility.
/// Foreign aliases/categories/modes still require a resolved profile; no parser,
/// package semantic digest, or domain provider execution is implied.
pub struct CheckedLanguagePackage<'a> {
    pub(super) package: &'a LanguagePackage,
    pub(super) registry: &'a SchemaRegistry,
    pub(super) reader: CheckedPlan<'a>,
}
impl<'a> CheckedLanguagePackage<'a> {
    pub fn package(&self) -> &'a LanguagePackage {
        self.package
    }
    pub fn registry(&self) -> &'a SchemaRegistry {
        self.registry
    }
    pub fn reader(&self) -> &CheckedPlan<'a> {
        &self.reader
    }
    pub fn validate_head_shape(
        &self,
        shape: &crate::selection::HeadShape,
        budget: &mut Budget,
    ) -> Result<(), PackageError> {
        budget.charge(Resource::Work, 1)?;
        if shape.kind.schema != self.package.schema {
            return Err(PackageError::KindShape);
        }
        let expected = super::shape::record_kind(&shape.kind, self.registry, budget)?;
        if expected.len() != shape.fields.len() {
            return Err(PackageError::KindShape);
        }
        for (field, expected) in shape.fields.iter().zip(expected) {
            let name = if matches!(
                super::shape::terminal(self.package, field.read, budget)?,
                ReadSpec::Foreign { .. }
            ) {
                "ForeignSyntax"
            } else {
                "NodeRef"
            };
            if field.name != expected.name
                || !matches!(&expected.ty,nepl3_core::schema::TypeDescriptor::Named(v) if v.package=="nepl3.foundation"&&v.revision==1&&v.name==name)
            {
                return Err(PackageError::KindShape);
            }
        }
        super::bindings::owner(
            self.package,
            &shape.fields,
            None,
            shape.binding,
            &shape.styles,
            self.registry,
            budget,
        )
    }
}
impl LanguagePackage {
    pub fn check<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<CheckedLanguagePackage<'a>, PackageError> {
        budget.charge(Resource::Work, 1)?;
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        for schema in core::iter::once(&self.schema).chain(&self.payload_schemas) {
            budget.charge(Resource::Work, 1)?;
            if registry.descriptor(schema).is_none() {
                return Err(SchemaError::UnknownSchema.into());
            }
        }
        for (i, schema) in self.payload_schemas.iter().enumerate() {
            for prior in &self.payload_schemas[..i] {
                budget.charge(
                    Resource::Work,
                    schema.package.len().min(prior.package.len()) as u64 + 33,
                )?;
                if prior == schema {
                    return Err(PackageError::DuplicateName);
                }
            }
        }
        if self.reader.schema != self.schema {
            return Err(PackageError::KindShape);
        }
        super::reader::check_dag(&self.reader, budget)?;
        let reader = self.reader.check(registry, budget)?;
        unique(self.categories.iter().map(|v| v.name.as_str()), budget)?;
        unique(self.modes.iter().map(|v| v.name.as_str()), budget)?;
        unique(self.namespaces.iter().map(|v| v.name.as_str()), budget)?;
        unique(self.extensions.iter().map(|v| v.alias.as_str()), budget)?;
        self.category(&self.root)?;
        for category in &self.categories {
            budget.charge(Resource::Work, self.modes.len() as u64 + 1)?;
            if !self.modes.iter().any(|v| v.name == category.mode) {
                return Err(PackageError::MissingMode);
            }
        }
        for mode in &self.modes {
            for rule in mode
                .skip
                .iter()
                .map(|v| &v.reader)
                .chain(mode.take.iter().map(|v| &v.reader))
            {
                budget.charge(Resource::Work, self.reader.rules.len() as u64 + 1)?;
                if let TokenReader::Rule(name) = rule {
                    self.reader.rule(name)?;
                }
            }
            for take in &mode.take {
                super::shape::record_kind(&take.kind, registry, budget)?;
            }
        }
        for extension in &self.extensions {
            budget.charge(Resource::Work, 1)?;
            if extension.provider.is_empty() || extension.signature.is_empty() {
                return Err(PackageError::EmptyName);
            }
            let descriptor = registry
                .descriptor(&extension.operation.schema)
                .ok_or(PackageError::MissingExtension)?;
            budget.charge(Resource::Work, descriptor.operations.len() as u64)?;
            let operation = descriptor
                .operations
                .iter()
                .find(|op| op.name == extension.operation.name)
                .ok_or(PackageError::MissingExtension)?;
            if operation.input != extension.input
                || operation.output != extension.output
                || operation.pure != extension.pure
            {
                return Err(PackageError::SignatureMismatch);
            }
        }
        super::shape::check(self, registry, budget)?;
        super::bindings::check(self, registry, budget)?;
        self.recovery.validate(self, registry, budget)?;
        let mut sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        for snapshot in &self.provenance.sources {
            admission.admit_existing(snapshot, budget)?;
            budget.charge(
                Resource::AllocationUnits,
                (snapshot.text().len() + snapshot.uri().len() + snapshot.identity().source.0.len())
                    as u64
                    + core::mem::size_of_val(snapshot) as u64,
            )?;
            sources.insert(snapshot.clone())?;
        }
        OriginGraph::validate_origins(&self.provenance.origins, &sources, budget)?;
        SourceMap::validate_mappings(&self.provenance.source_maps, &sources, budget)?;
        for origin in &self.provenance.declarations {
            budget.charge(Resource::Work, 1)?;
            if origin.name.is_empty() || origin.origin.0 >= self.provenance.origins.len() as u64 {
                return Err(PackageError::Provenance);
            }
        }
        Ok(CheckedLanguagePackage {
            package: self,
            registry,
            reader,
        })
    }
    pub fn category(&self, name: &str) -> Result<&Category, PackageError> {
        self.categories
            .iter()
            .find(|v| v.name == name)
            .ok_or(PackageError::MissingCategory)
    }
    pub fn read(&self, id: ReadSpecId) -> Result<&ReadSpec, PackageError> {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.reads.get(i))
            .ok_or(PackageError::InvalidRead)
    }
}
pub(super) fn unique<'a>(
    values: impl Iterator<Item = &'a str>,
    budget: &mut Budget,
) -> Result<(), PackageError> {
    let mut names = Vec::new();
    for name in values {
        budget.charge(Resource::Work, 1)?;
        if name.is_empty() {
            return Err(PackageError::EmptyName);
        }
        for prior in &names {
            budget.charge(Resource::Work, name.len() as u64 + 1)?;
            if *prior == name {
                return Err(PackageError::DuplicateName);
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<&str>() as u64,
        )?;
        names.push(name);
    }
    Ok(())
}
