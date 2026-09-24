use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Tables {
    origins: Vec<Origin>,
    sources: Vec<SourceSnapshot>,
    maps: Vec<Mapping>,
}

#[cfg(target_has_atomic = "ptr")]
type Storage = alloc::sync::Arc<Tables>;
#[cfg(not(target_has_atomic = "ptr"))]
type Storage = Tables;

/// Immutable owner tables. IDs retain their original order across all captures.
/// This is storage, not a validation proof. Wire decoding still validates every
/// received table. Targets without pointer atomics use budgeted owned copies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerProvenance {
    storage: Storage,
}

impl OwnerProvenance {
    /// Construct unvalidated immutable tables under the caller's allocation
    /// policy. Budgeted processing uses `new`.
    pub fn from_parts(
        origins: Vec<Origin>,
        sources: Vec<SourceSnapshot>,
        maps: Vec<Mapping>,
    ) -> Self {
        let storage = Tables {
            origins,
            sources,
            maps,
        };
        #[cfg(target_has_atomic = "ptr")]
        let storage = alloc::sync::Arc::new(storage);
        Self { storage }
    }
    /// Take ownership of already allocated tables, charging the storage wrapper.
    pub fn new(
        origins: Vec<Origin>,
        sources: Vec<SourceSnapshot>,
        maps: Vec<Mapping>,
        b: &mut Budget,
    ) -> Result<Self, StopReason> {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Tables>() as u64,
        )?;
        #[cfg(target_has_atomic = "ptr")]
        b.charge(
            Resource::AllocationUnits,
            (2 * core::mem::size_of::<usize>()) as u64,
        )?;
        Ok(Self::from_parts(origins, sources, maps))
    }

    pub fn origins(&self) -> &[Origin] {
        &self.storage.origins
    }
    pub fn sources(&self) -> &[SourceSnapshot] {
        &self.storage.sources
    }
    pub fn source_maps(&self) -> &[Mapping] {
        &self.storage.maps
    }

    pub fn charge_clone(&self, b: &mut Budget) -> Result<(), StopReason> {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64,
        )?;
        #[cfg(not(target_has_atomic = "ptr"))]
        {
            for origin in self.origins() {
                origin.charge_clone(b)?;
            }
            for source in self.sources() {
                source.charge_clone(b)?;
            }
            for map in self.source_maps() {
                map.charge_clone(b)?;
            }
        }
        Ok(())
    }

    pub fn clone_with_budget(&self, b: &mut Budget) -> Result<Self, StopReason> {
        self.charge_clone(b)?;
        Ok(self.clone())
    }

    pub(super) fn capture(owner: &SyntaxBundle, b: &mut Budget) -> Result<Self, StopReason> {
        let mut origins = array::<Origin>(owner.origins.len(), b)?;
        for origin in &owner.origins {
            origins.push(origin.clone_with_budget(b)?);
        }
        let mut sources = array::<SourceSnapshot>(owner.sources.len(), b)?;
        for source in &owner.sources {
            sources.push(source.clone_with_budget(b)?);
        }
        let mut maps = array::<Mapping>(owner.source_maps.len(), b)?;
        for map in &owner.source_maps {
            maps.push(map.clone_with_budget(b)?);
        }
        Self::new(origins, sources, maps, b)
    }
}

fn array<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, StopReason> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(values)
}

/// One immutable owner's captures share its complete provenance. The context
/// cannot be rebound to another owner. Each closure still receives full validation.
pub struct ForeignCapture<'a> {
    owner: &'a ValidatedSyntaxBundle<'a>,
    provenance: Option<OwnerProvenance>,
}
impl<'a> ForeignCapture<'a> {
    pub fn new(owner: &'a ValidatedSyntaxBundle<'a>) -> Self {
        Self {
            owner,
            provenance: None,
        }
    }

    pub fn capture_at(
        &mut self,
        node: NodeRef,
        field: usize,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ForeignClosure, SyntaxError> {
        b.charge(Resource::Work, 1)?;
        let owner = self.owner.bundle();
        let Some(FieldValue::Foreign(syntax)) = owner.node(node)?.fields.get(field) else {
            return Err(SyntaxError::Reference);
        };
        if self.provenance.is_none() {
            self.provenance = Some(OwnerProvenance::capture(owner, b)?);
        }
        let provenance = self.provenance.as_ref().ok_or(SyntaxError::Reference)?;
        ForeignClosure::capture_selected(
            syntax,
            owner,
            provenance.clone_with_budget(b)?,
            registry,
            b,
            admission,
        )
    }
}
