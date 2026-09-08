use super::*;
use crate::package::{
    CheckedLanguagePackage, EntryContext, LanguagePackage, PackageError, ReadSpec, ReadSpecId,
};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeShape},
};
mod head;
mod identity;

#[derive(Debug, Eq, PartialEq)]
pub enum ProfileError {
    Stopped(StopReason),
    Package(PackageError),
    Schema(SchemaError),
    EmptyName,
    Duplicate,
    MissingPackage,
    PackageIdentity,
    MissingSchema,
    MissingAlias,
    MissingCategory,
    MissingMode,
    MissingProvider,
    ProviderIdentity,
    NotAllowed,
    MissingResource,
    ResourceIdentity,
    HeadSignature,
}
impl From<StopReason> for ProfileError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<PackageError> for ProfileError {
    fn from(v: PackageError) -> Self {
        match v {
            PackageError::Stopped(v) => Self::Stopped(v),
            v => Self::Package(v),
        }
    }
}
impl From<SchemaError> for ProfileError {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(v) => Self::Stopped(v),
            v => Self::Schema(v),
        }
    }
}

/// Trusted registration boundary, supplied independently of the external profile.
pub struct RuntimeCatalog<'a> {
    pub packages: &'a [&'a LanguagePackage],
    pub providers: &'a [ProviderImplementation],
    pub resources: &'a [ResourceSnapshot],
}
pub struct ResolvedParseProfile<'a> {
    pub(super) profile: &'a ParseProfile,
    pub(super) registry: &'a SchemaRegistry,
    pub(super) packages: Vec<CheckedLanguagePackage<'a>>,
    execution_digests: Vec<Digest>,
    pub(super) digest: Digest,
}
fn nonempty(name: &str) -> Result<(), ProfileError> {
    if name.is_empty() {
        Err(ProfileError::EmptyName)
    } else {
        Ok(())
    }
}
fn lookup(budget: &mut Budget, count: usize, width: usize) -> Result<(), ProfileError> {
    budget.charge(
        Resource::Work,
        (count as u64)
            .saturating_mul(width as u64 + 1)
            .saturating_add(1),
    )?;
    Ok(())
}
fn unique<'a>(
    names: impl Iterator<Item = &'a str>,
    budget: &mut Budget,
) -> Result<(), ProfileError> {
    let mut prior = Vec::new();
    for name in names {
        nonempty(name)?;
        for p in &prior {
            budget.charge(Resource::Work, name.len() as u64 + 1)?;
            if *p == name {
                return Err(ProfileError::Duplicate);
            }
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<&str>() as u64,
        )?;
        prior.push(name);
    }
    Ok(())
}
fn selected(
    profile: &ParseProfile,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<(), ProfileError> {
    budget.charge(
        Resource::Work,
        profile.schemas.len() as u64 * (schema.package.len() as u64 + 33) + 1,
    )?;
    if !profile.schemas.contains(schema) {
        return Err(ProfileError::MissingSchema);
    }
    Ok(())
}
fn type_closure(
    profile: &ParseProfile,
    ty: &TypeDescriptor,
    budget: &mut Budget,
) -> Result<(), ProfileError> {
    let mut ty = ty;
    let mut depth = 1;
    loop {
        budget.charge(Resource::Work, 1)?;
        budget.observe_depth(depth)?;
        match ty {
            TypeDescriptor::List(v) | TypeDescriptor::Option(v) => {
                ty = v;
                depth += 1;
            }
            TypeDescriptor::Named(v) => {
                budget.charge(
                    Resource::Work,
                    profile.schemas.len() as u64 * (v.package.len() as u64 + 1),
                )?;
                if !profile
                    .schemas
                    .iter()
                    .any(|s| s.package == v.package && s.revision == v.revision)
                {
                    return Err(ProfileError::MissingSchema);
                }
                return Ok(());
            }
            _ => return Ok(()),
        }
    }
}
impl ParseProfile {
    pub fn resolve<'a>(
        &'a self,
        catalog: &RuntimeCatalog<'a>,
        registry: &'a SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<ResolvedParseProfile<'a>, ProfileError> {
        budget.charge(Resource::Work, 1)?;
        nonempty(&self.id)?;
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        unique(self.languages.iter().map(|v| v.alias.as_str()), budget)?;
        unique(self.resources.iter().map(|v| v.id.as_str()), budget)?;
        unique(catalog.resources.iter().map(|v| v.id.as_str()), budget)?;
        unique(
            catalog.providers.iter().map(|v| v.provider.as_str()),
            budget,
        )?;
        for (i, schema) in self.schemas.iter().enumerate() {
            for prior in &self.schemas[..i] {
                budget.charge(Resource::Work, schema.package.len() as u64 + 1)?;
                if prior.package == schema.package && prior.revision == schema.revision {
                    return Err(ProfileError::Duplicate);
                }
            }
            let descriptor = registry
                .descriptor(schema)
                .ok_or(ProfileError::MissingSchema)?;
            for ty in &descriptor.types {
                match &ty.shape {
                    TypeShape::Record { fields } => {
                        for f in fields {
                            type_closure(self, &f.ty, budget)?;
                        }
                    }
                    TypeShape::Variant { variants } => {
                        for v in variants {
                            for f in &v.fields {
                                type_closure(self, &f.ty, budget)?;
                            }
                        }
                    }
                }
            }
            for op in &descriptor.operations {
                type_closure(self, &op.input, budget)?;
                type_closure(self, &op.output, budget)?;
            }
        }
        let mut packages = Vec::new();
        let mut execution_digests = Vec::new();
        for language in &self.languages {
            selected(self, &language.package.schema, budget)?;
            let mut found = None;
            for candidate in catalog.packages {
                budget.charge(
                    Resource::Work,
                    language.package.schema.package.len() as u64 + 33,
                )?;
                if candidate.schema != language.package.schema {
                    continue;
                }
                let checked = candidate.check(registry, budget)?;
                if checked.semantic_identity(budget)? != language.package {
                    continue;
                }
                if found.replace(checked).is_some() {
                    return Err(ProfileError::Duplicate);
                }
            }
            let checked = found.ok_or(ProfileError::PackageIdentity)?;
            let candidate = checked.package();
            lookup(
                budget,
                candidate.categories.len(),
                language.default_category.len(),
            )?;
            candidate
                .category(&language.default_category)
                .map_err(|_| ProfileError::MissingCategory)?;
            for schema in &candidate.payload_schemas {
                selected(self, schema, budget)?;
            }
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<CheckedLanguagePackage<'_>>() as u64,
            )?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Digest>() as u64,
            )?;
            execution_digests.push(checked.execution_digest(budget)?);
            packages.push(checked);
        }
        for (i, provider) in self.providers.iter().enumerate() {
            nonempty(&provider.provider)?;
            selected(self, &provider.operation.schema, budget)?;
            for prior in &self.providers[..i] {
                budget.charge(
                    Resource::Work,
                    provider.operation.name.len() as u64
                        + provider.operation.schema.package.len() as u64
                        + 33,
                )?;
                if prior.operation == provider.operation {
                    return Err(ProfileError::Duplicate);
                }
            }
            lookup(budget, catalog.providers.len(), provider.provider.len())?;
            let host = catalog
                .providers
                .iter()
                .find(|v| v.provider == provider.provider)
                .ok_or(ProfileError::MissingProvider)?;
            if host.revision != provider.revision
                || host.implementation_digest != provider.implementation_digest
            {
                return Err(ProfileError::ProviderIdentity);
            }
            lookup(
                budget,
                host.operations.len(),
                provider.operation.schema.package.len() + provider.operation.name.len() + 32,
            )?;
            if !host.operations.contains(&provider.operation) {
                return Err(ProfileError::MissingProvider);
            }
            let descriptor = registry
                .descriptor(&provider.operation.schema)
                .ok_or(ProfileError::MissingSchema)?;
            lookup(
                budget,
                descriptor.operations.len(),
                provider.operation.name.len(),
            )?;
            if !descriptor
                .operations
                .iter()
                .any(|v| v.name == provider.operation.name)
            {
                return Err(ProfileError::MissingProvider);
            }
        }
        for (i, op) in self.allowlist.iter().enumerate() {
            budget.charge(
                Resource::Work,
                (self.providers.len() + i) as u64
                    * (op.name.len() as u64 + op.schema.package.len() as u64 + 33),
            )?;
            if self.allowlist[..i].contains(op) {
                return Err(ProfileError::Duplicate);
            }
            if !self.providers.iter().any(|v| &v.operation == op) {
                return Err(ProfileError::MissingProvider);
            }
        }
        for resource in &self.resources {
            lookup(budget, catalog.resources.len(), resource.id.len())?;
            let host = catalog
                .resources
                .iter()
                .find(|v| v.id == resource.id)
                .ok_or(ProfileError::MissingResource)?;
            budget.charge(Resource::Work, host.bytes.len() as u64 + 1)?;
            if Digest::of(&host.bytes) != resource.digest {
                return Err(ProfileError::ResourceIdentity);
            }
        }
        let digest = identity::digest(self, budget)?;
        let resolved = ResolvedParseProfile {
            profile: self,
            registry,
            packages,
            execution_digests,
            digest,
        };
        resolved.validate_heads(budget)?;
        for (i, override_) in self.category_modes.iter().enumerate() {
            lookup(budget, i, override_.alias.len() + override_.category.len())?;
            if self.category_modes[..i]
                .iter()
                .any(|v| v.alias == override_.alias && v.category == override_.category)
            {
                return Err(ProfileError::Duplicate);
            }
            let package = resolved.language(&override_.alias, budget)?;
            lookup(budget, package.categories.len(), override_.category.len())?;
            package
                .category(&override_.category)
                .map_err(|_| ProfileError::MissingCategory)?;
            lookup(budget, package.modes.len(), override_.mode.len())?;
            if !package.modes.iter().any(|v| v.name == override_.mode) {
                return Err(ProfileError::MissingMode);
            }
        }
        for checked in &resolved.packages {
            let package = checked.package();
            for (i, read) in package.reads.iter().enumerate() {
                match read {
                    ReadSpec::Foreign { alias, category } => {
                        let guest = resolved.language(alias, budget)?;
                        lookup(budget, guest.categories.len(), category.len())?;
                        guest
                            .category(category)
                            .map_err(|_| ProfileError::MissingCategory)?;
                    }
                    ReadSpec::WithMode { mode, .. } => {
                        let owner = resolved.read_owner(package, ReadSpecId(i as u64), budget)?;
                        lookup(budget, owner.modes.len(), mode.len())?;
                        if !owner.modes.iter().any(|v| &v.name == mode) {
                            return Err(ProfileError::MissingMode);
                        }
                    }
                    _ => {}
                }
            }
            for provider in &package.reader.providers {
                resolved.provider(&provider.operation, budget)?;
            }
        }
        Ok(resolved)
    }
}
impl<'a> ResolvedParseProfile<'a> {
    pub fn profile(&self) -> &'a ParseProfile {
        self.profile
    }
    pub fn registry(&self) -> &'a SchemaRegistry {
        self.registry
    }
    pub fn digest(&self) -> Digest {
        self.digest
    }
    pub fn execution_digest(
        &self,
        alias: &str,
        budget: &mut Budget,
    ) -> Result<Digest, ProfileError> {
        lookup(budget, self.profile.languages.len(), alias.len())?;
        let index = self
            .profile
            .languages
            .iter()
            .position(|v| v.alias == alias)
            .ok_or(ProfileError::MissingAlias)?;
        self.execution_digests
            .get(index)
            .copied()
            .ok_or(ProfileError::MissingPackage)
    }
    pub fn validate_entry(
        &self,
        entry: &EntryContext,
        budget: &mut Budget,
    ) -> Result<&CheckedLanguagePackage<'a>, ProfileError> {
        let checked = self.checked(&entry.alias, budget)?;
        lookup(budget, self.profile.languages.len(), entry.alias.len())?;
        let registered = self
            .profile
            .languages
            .iter()
            .find(|v| v.alias == entry.alias)
            .ok_or(ProfileError::MissingAlias)?;
        budget.charge(
            Resource::Work,
            entry.package.schema.package.len() as u64 + 65,
        )?;
        if registered.package != entry.package {
            return Err(ProfileError::PackageIdentity);
        }
        lookup(
            budget,
            checked.package().categories.len(),
            entry.category.len(),
        )?;
        checked
            .package()
            .category(&entry.category)
            .map_err(|_| ProfileError::MissingCategory)?;
        lookup(budget, checked.package().modes.len(), entry.mode.len())?;
        if !checked.package().modes.iter().any(|v| v.name == entry.mode) {
            return Err(ProfileError::MissingMode);
        }
        Ok(checked)
    }
    pub fn language(
        &self,
        alias: &str,
        budget: &mut Budget,
    ) -> Result<&'a LanguagePackage, ProfileError> {
        Ok(self.checked(alias, budget)?.package())
    }
    pub fn checked(
        &self,
        alias: &str,
        budget: &mut Budget,
    ) -> Result<&CheckedLanguagePackage<'a>, ProfileError> {
        budget.charge(
            Resource::Work,
            self.profile.languages.len() as u64 * (alias.len() as u64 + 1),
        )?;
        let index = self
            .profile
            .languages
            .iter()
            .position(|v| v.alias == alias)
            .ok_or(ProfileError::MissingAlias)?;
        self.packages.get(index).ok_or(ProfileError::MissingPackage)
    }
    pub fn provider(
        &self,
        operation: &OperationRef,
        budget: &mut Budget,
    ) -> Result<&'a ProviderRequirement, ProfileError> {
        budget.charge(
            Resource::Work,
            (self.profile.allowlist.len() + self.profile.providers.len()) as u64
                * (operation.name.len() as u64 + operation.schema.package.len() as u64 + 33),
        )?;
        if !self.profile.allowlist.contains(operation) {
            return Err(ProfileError::NotAllowed);
        }
        self.profile
            .providers
            .iter()
            .find(|v| &v.operation == operation)
            .ok_or(ProfileError::MissingProvider)
    }
    pub fn entry(
        &self,
        alias: &str,
        category: Option<&str>,
        budget: &mut Budget,
    ) -> Result<EntryContext, ProfileError> {
        let package = self.language(alias, budget)?;
        lookup(budget, self.profile.languages.len(), alias.len())?;
        let registration = self
            .profile
            .languages
            .iter()
            .find(|v| v.alias == alias)
            .ok_or(ProfileError::MissingAlias)?;
        let category = category.unwrap_or(&registration.default_category);
        lookup(budget, package.categories.len(), category.len())?;
        let declared = package
            .category(category)
            .map_err(|_| ProfileError::MissingCategory)?;
        lookup(
            budget,
            self.profile.category_modes.len(),
            alias.len() + category.len(),
        )?;
        let mode = self
            .profile
            .category_modes
            .iter()
            .find(|v| v.alias == alias && v.category == category)
            .map(|v| v.mode.as_str())
            .unwrap_or(&declared.mode);
        budget.charge(
            Resource::AllocationUnits,
            (registration.package.schema.package.len() + alias.len() + category.len() + mode.len())
                as u64,
        )?;
        Ok(EntryContext {
            package: registration.package.clone(),
            alias: alias.into(),
            category: category.into(),
            mode: mode.into(),
        })
    }
    /// A form child starts at its registration/category default. WithMode overrides
    /// only this root. A list tail instead retains the already resolved spine entry.
    pub fn read_entry(
        &self,
        parent: &EntryContext,
        mut id: ReadSpecId,
        budget: &mut Budget,
    ) -> Result<super::ResolvedRead, ProfileError> {
        let package = self.validate_entry(parent, budget)?.package();
        let mut mode = None;
        loop {
            budget.charge(Resource::Work, 1)?;
            match package.read(id)? {
                ReadSpec::WithMode {
                    mode: selected,
                    read,
                } => {
                    mode = Some(selected.as_str());
                    id = *read;
                }
                read => {
                    let (alias, category, foreign, local_read) = match read {
                        ReadSpec::Local { category } => {
                            (parent.alias.as_str(), category.as_str(), false, None)
                        }
                        ReadSpec::Foreign { alias, category } => {
                            (alias.as_str(), category.as_str(), true, None)
                        }
                        ReadSpec::Builtin { .. } | ReadSpec::ListOf { .. } => (
                            parent.alias.as_str(),
                            parent.category.as_str(),
                            false,
                            Some(id),
                        ),
                        ReadSpec::WithMode { .. } => return Err(PackageError::InvalidRead.into()),
                    };
                    let mut entry = self.entry(alias, Some(category), budget)?;
                    if let Some(mode) = mode {
                        let owner = self.language(alias, budget)?;
                        lookup(budget, owner.modes.len(), mode.len())?;
                        if !owner.modes.iter().any(|m| m.name == mode) {
                            return Err(ProfileError::MissingMode);
                        }
                        budget.charge(Resource::AllocationUnits, mode.len() as u64)?;
                        budget.charge(Resource::Work, mode.len() as u64)?;
                        entry.mode = mode.into();
                    }
                    return Ok(super::ResolvedRead {
                        entry,
                        read: local_read,
                        foreign,
                    });
                }
            }
        }
    }
    fn read_owner(
        &self,
        package: &'a LanguagePackage,
        mut id: ReadSpecId,
        budget: &mut Budget,
    ) -> Result<&'a LanguagePackage, ProfileError> {
        loop {
            budget.charge(Resource::Work, 1)?;
            match package.read(id)? {
                ReadSpec::WithMode { read, .. } => id = *read,
                ReadSpec::Foreign { alias, .. } => return self.language(alias, budget),
                _ => return Ok(package),
            }
        }
    }
}
