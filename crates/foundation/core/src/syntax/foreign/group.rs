//! Explicit closed groups share owner provenance without rebasing local IDs.
use super::*;
mod capture;
mod select;
mod session;
pub use capture::CaptureField;
pub(super) use capture::copy_provenance;
pub use session::ForeignGroupValidation;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProvenanceRef(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OwnerEnvironmentRef(pub u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignProvenance {
    pub origins: Vec<Origin>,
    pub sources: Vec<SourceSnapshot>,
    pub source_maps: Vec<Mapping>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignOwnerEnvironment {
    pub provenance: ProvenanceRef,
    pub environment: EnvironmentEntry,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignGroupEntry {
    pub owner: OwnerEnvironmentRef,
    pub syntax: ForeignSyntax,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ForeignClosureGroup {
    pub provenances: Vec<ForeignProvenance>,
    pub environments: Vec<ForeignOwnerEnvironment>,
    pub entries: Vec<ForeignGroupEntry>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupError {
    Stopped(StopReason),
    Syntax(SyntaxError),
    Provenance(ProvenanceRef),
    Environment(OwnerEnvironmentRef),
    DuplicateEnvironment { provenance: ProvenanceRef, id: u64 },
    UnusedProvenance(ProvenanceRef),
    UnusedEnvironment(OwnerEnvironmentRef),
    DepthCount,
    Owner(u64),
    Entry(u64),
    DuplicateOwner { first: u64, second: u64 },
}
impl GroupError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(s) => Some(*s),
            Self::Syntax(e) => e.stop_reason(),
            _ => None,
        }
    }
}
impl From<StopReason> for GroupError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl From<SyntaxError> for GroupError {
    fn from(error: SyntaxError) -> Self {
        match error.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Syntax(error),
        }
    }
}
pub struct ValidatedForeignGroup<'a> {
    value: &'a ForeignClosureGroup,
}
impl<'a> ValidatedForeignGroup<'a> {
    pub fn value(&self) -> &'a ForeignClosureGroup {
        self.value
    }
    /// Borrow a guest's explicit components without materializing owner tables.
    /// This view itself is not a transferable validation proof.
    pub fn entry(&self, number: usize) -> Option<ForeignClosureView<'a>> {
        self.value.entry(number)
    }
}
impl ForeignClosureGroup {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.environments.is_empty() && self.provenances.is_empty()
    }
    /// Bounds-checked borrowed components, not a validation proof.
    pub fn entry(&self, number: usize) -> Option<ForeignClosureView<'_>> {
        let entry = self.entries.get(number)?;
        let owner = self
            .environments
            .get(usize::try_from(entry.owner.0).ok()?)?;
        let provenance = self
            .provenances
            .get(usize::try_from(owner.provenance.0).ok()?)?;
        Some(ForeignClosureView {
            syntax: &entry.syntax,
            environment: &owner.environment,
            origins: &provenance.origins,
            sources: &provenance.sources,
            source_maps: &provenance.source_maps,
        })
    }
}
fn index(value: u64, len: usize) -> Option<usize> {
    usize::try_from(value).ok().filter(|index| *index < len)
}
fn scratch<T: Clone>(len: usize, value: T, b: &mut Budget) -> Result<Vec<T>, GroupError> {
    let bytes = len
        .checked_mul(core::mem::size_of::<T>())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    b.charge(Resource::Work, len as u64)?;
    let mut out = Vec::new();
    out.try_reserve_exact(len)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    out.resize(len, value);
    Ok(out)
}
impl ForeignClosureGroup {
    /// Check explicit source closure at the current depth. This does not certify
    /// semantic occurrence depths in any consuming language.
    pub fn validate<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignGroup<'a>, GroupError> {
        self.validate_inner(None, registry, b, admission)
    }
    /// One depth per stored entry: the maximum across every checked semantic
    /// occurrence referring to that entry, relative to the current caller.
    /// Component-specific maxima preserve shared
    /// provenance/environment depth without imposing unrelated guest depths.
    pub fn validate_at_depths<'a>(
        &'a self,
        depths: &[u64],
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignGroup<'a>, GroupError> {
        self.validate_inner(Some(depths), registry, b, admission)
    }
    fn validate_inner<'a>(
        &'a self,
        depths: Option<&[u64]>,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignGroup<'a>, GroupError> {
        b.poll()?;
        if !registry.is_finalized() {
            return Err(SyntaxError::Schema(SchemaError::Unfinalized).into());
        }
        if depths.is_some_and(|depths| depths.len() != self.entries.len()) {
            return Err(GroupError::DepthCount);
        }
        let mut provenance_depth = scratch(self.provenances.len(), None::<u64>, b)?;
        let mut environment_depth = scratch(self.environments.len(), None::<u64>, b)?;
        for owner in &self.environments {
            b.charge(Resource::Work, 1)?;
            if index(owner.provenance.0, self.provenances.len()).is_none() {
                return Err(GroupError::Provenance(owner.provenance));
            }
        }
        let ids = crate::syntax::environment::sorted(
            self.environments.len(),
            |a, z, b| {
                b.charge(Resource::Work, 1)?;
                let a = &self.environments[a];
                let z = &self.environments[z];
                Ok((a.provenance, a.environment.id).cmp(&(z.provenance, z.environment.id)))
            },
            b,
        )?;
        for pair in ids.windows(2) {
            b.charge(Resource::Work, 1)?;
            let a = &self.environments[pair[0]];
            let z = &self.environments[pair[1]];
            if a.provenance == z.provenance && a.environment.id == z.environment.id {
                return Err(GroupError::DuplicateEnvironment {
                    provenance: a.provenance,
                    id: a.environment.id,
                });
            }
        }
        let base = b.current_depth();
        for (number, entry) in self.entries.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let env = index(entry.owner.0, self.environments.len())
                .ok_or(GroupError::Environment(entry.owner))?;
            let owner = &self.environments[env];
            let provenance = index(owner.provenance.0, self.provenances.len())
                .ok_or(GroupError::Provenance(owner.provenance))?;
            let depth = depths.map_or(0, |depths| depths[number]);
            environment_depth[env] =
                Some(environment_depth[env].map_or(depth, |old| old.max(depth)));
            provenance_depth[provenance] =
                Some(provenance_depth[provenance].map_or(depth, |old| old.max(depth)));
            association(&entry.syntax, &owner.environment)?;
            let absolute = base
                .checked_add(depth)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            b.with_depth_at_least::<_, GroupError>(absolute, |b| {
                validate_syntax(&entry.syntax, registry, b, admission)?;
                Ok(())
            })?;
        }
        for (number, (provenance, depth)) in
            self.provenances.iter().zip(provenance_depth).enumerate()
        {
            b.charge(Resource::Work, 1)?;
            let depth = depth.ok_or(GroupError::UnusedProvenance(ProvenanceRef(number as u64)))?;
            let absolute = base
                .checked_add(depth)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            b.with_depth_at_least::<_, GroupError>(absolute, |b| {
                validate_provenance(
                    &provenance.origins,
                    &provenance.sources,
                    &provenance.source_maps,
                    b,
                    admission,
                )?;
                Ok(())
            })?;
        }
        for (number, (owner, depth)) in self.environments.iter().zip(environment_depth).enumerate()
        {
            b.charge(Resource::Work, 1)?;
            let depth = depth.ok_or(GroupError::UnusedEnvironment(OwnerEnvironmentRef(
                number as u64,
            )))?;
            let absolute = base
                .checked_add(depth)
                .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
            let provenance = index(owner.provenance.0, self.provenances.len())
                .ok_or(GroupError::Provenance(owner.provenance))?;
            b.with_depth_at_least::<_, GroupError>(absolute, |b| {
                environment(
                    &owner.environment.value,
                    self.provenances[provenance].origins.len(),
                    registry,
                    b,
                )?;
                Ok(())
            })?;
        }
        b.poll()?;
        Ok(ValidatedForeignGroup { value: self })
    }
}
