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
/// An index in the exact package being checked, never a portable source position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageSubject {
    Root,
    Category(u64),
    ModeSkip {
        mode: u64,
        rule: u64,
    },
    ModeTake {
        mode: u64,
        rule: u64,
    },
    Extension(u64),
    Read(ReadSpecId),
    Form(u64),
    FormField {
        form: u64,
        field: u64,
    },
    Leaf(u64),
    Binding {
        owner: Option<BindingOwner>,
        binding: Option<BindingId>,
    },
    Provenance(u64),
}
#[derive(Debug, Eq, PartialEq)]
pub struct PackageFailure {
    pub error: PackageError,
    pub subject: Option<PackageSubject>,
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
            (&shape.styles, &shape.selection_rules),
            self.registry,
            budget,
        )
    }
}
impl LanguagePackage {
    /// Validate as a standalone operation with a fresh source admission ledger.
    /// Use [`Self::check_with_admission`] when checking a compiled package within
    /// the operation that has already admitted its provenance sources.
    pub fn check<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<CheckedLanguagePackage<'a>, PackageError> {
        self.check_with_admission(registry, budget, &mut SourceAdmission::default())
    }
    /// Check within an existing operation, admitting each provenance snapshot
    /// only once through the caller's shared source ledger.
    pub fn check_with_admission<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedLanguagePackage<'a>, PackageError> {
        self.check_detailed_with_admission(registry, budget, admission)
            .map_err(|failure| failure.error)
    }
    /// Validate with the same rules as `check`, retaining the inspected package subject.
    pub fn check_detailed<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<CheckedLanguagePackage<'a>, PackageFailure> {
        self.check_detailed_with_admission(registry, budget, &mut SourceAdmission::default())
    }
    /// Detailed validation within the caller's existing source admission operation.
    pub fn check_detailed_with_admission<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedLanguagePackage<'a>, PackageFailure> {
        let mut subject = None;
        self.check_inner(registry, budget, admission, &mut subject)
            .map_err(|error| PackageFailure { error, subject })
    }
    fn check_inner<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
        subject: &mut Option<PackageSubject>,
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
        *subject = Some(PackageSubject::Root);
        self.category(&self.root)?;
        for (index, category) in self.categories.iter().enumerate() {
            *subject = Some(PackageSubject::Category(index as u64));
            budget.charge(Resource::Work, self.modes.len() as u64 + 1)?;
            if !self.modes.iter().any(|v| v.name == category.mode) {
                return Err(PackageError::MissingMode);
            }
        }
        for (mode_index, mode) in self.modes.iter().enumerate() {
            for (rule, reader) in mode
                .skip
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    (
                        PackageSubject::ModeSkip {
                            mode: mode_index as u64,
                            rule: i as u64,
                        },
                        &v.reader,
                    )
                })
                .chain(mode.take.iter().enumerate().map(|(i, v)| {
                    (
                        PackageSubject::ModeTake {
                            mode: mode_index as u64,
                            rule: i as u64,
                        },
                        &v.reader,
                    )
                }))
            {
                *subject = Some(rule);
                budget.charge(Resource::Work, self.reader.rules.len() as u64 + 1)?;
                if let TokenReader::Rule(name) = reader {
                    self.reader.rule(name)?;
                }
            }
            for (rule, take) in mode.take.iter().enumerate() {
                *subject = Some(PackageSubject::ModeTake {
                    mode: mode_index as u64,
                    rule: rule as u64,
                });
                super::shape::record_kind(&take.kind, registry, budget)?;
            }
        }
        for (index, extension) in self.extensions.iter().enumerate() {
            *subject = Some(PackageSubject::Extension(index as u64));
            budget.charge(Resource::Work, 1)?;
            if extension.provider.is_empty() || extension.signature.is_empty() {
                return Err(PackageError::EmptyName);
            }
            if extension.signature == "facts/v1"
                && !crate::facts::signature(&extension.input, &extension.output, extension.pure)
            {
                return Err(PackageError::SignatureMismatch);
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
        *subject = None;
        super::shape::check(self, registry, budget, subject)?;
        super::bindings::check(self, registry, budget, subject)?;
        *subject = None;
        self.recovery.validate(self, registry, budget)?;
        let mut sources = SourceStore::default();
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
        for (index, origin) in self.provenance.declarations.iter().enumerate() {
            *subject = Some(PackageSubject::Provenance(index as u64));
            budget.charge(Resource::Work, 1)?;
            if origin.name.is_empty() || origin.origin.0 >= self.provenance.origins.len() as u64 {
                return Err(PackageError::Provenance);
            }
            match origin.kind {
                DeclarationKind::Form | DeclarationKind::Leaf => {
                    let category = origin
                        .category
                        .as_deref()
                        .filter(|v| !v.is_empty())
                        .ok_or(PackageError::Provenance)?;
                    budget.charge(
                        Resource::Work,
                        (self.categories.len() as u64).saturating_mul(category.len() as u64 + 1),
                    )?;
                    self.category(category)?;
                }
                _ if origin.category.is_some() => return Err(PackageError::Provenance),
                _ => {}
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
