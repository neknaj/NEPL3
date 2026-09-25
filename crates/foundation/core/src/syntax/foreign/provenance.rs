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

/// Checked immutable owner tables and registry. Each use admits their sources
/// into the caller's explicit admission context before reusing graph checks.
/// Their required relative depth is applied to the current caller's Budget;
/// earlier validation never grants additional depth or source authority.
pub struct ValidatedOwnerProvenance<'a> {
    owner: &'a OwnerProvenance,
    registry: &'a SchemaRegistry,
    depth: u64,
}

impl<'r> ValidatedOwnerProvenance<'r> {
    /// Validate the complete closure and retain the immutable registry borrow
    /// with its guest graph proof for subsequent portable encoding.
    pub fn validate_closure_syntax<'s>(
        &self,
        closure: &'s ForeignClosure,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<RegistryValidatedSyntaxBundle<'s, 'r>, SyntaxError> {
        let validated = self.validate_closure(closure, b, admission)?;
        Ok(RegistryValidatedSyntaxBundle {
            syntax: validated.syntax,
            registry: self.registry,
        })
    }

    /// Whether this proof borrows the same immutable tables. This comparison
    /// grants no guest, depth or source-admission proof; use `validate_closure`
    /// for those checks in the receiving operation.
    pub fn matches_owner(&self, owner: &OwnerProvenance) -> bool {
        self.owner.same_tables(owner)
    }
    /// Validate each guest and its selected environment. Only the unchanged
    /// owner tables reuse validation; guest graphs and environment values are
    /// checked on every call. Native environment hash recomputation remains a
    /// portable codec responsibility.
    pub fn validate_closure<'s>(
        &self,
        closure: &'s ForeignClosure,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedForeignClosure<'s>, SyntaxError> {
        b.charge(Resource::Work, 1)?;
        if !self.matches_owner(&closure.provenance) {
            return Err(SyntaxError::Reference);
        }
        b.observe_depth(self.depth)?;
        for source in self.owner.sources() {
            b.charge(Resource::Work, 1)?;
            admission.admit_existing(source, b)?;
        }
        closure.validate_contents(self.registry, b, admission)
    }
}

impl OwnerProvenance {
    /// Check complete owner tables once. The returned proof borrows all inputs;
    /// it cannot outlive or mutate their table order, source identity or spans.
    pub fn validate<'a>(
        &'a self,
        registry: &'a SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedOwnerProvenance<'a>, SyntaxError> {
        b.poll()?;
        if !registry.is_finalized() {
            return Err(crate::schema::SchemaError::Unfinalized.into());
        }
        let mut store = SourceStore::default();
        for source in self.sources() {
            admission.admit_existing(source, b)?;
            if !store.insert_distinct_ref_with_budget(source, b)? {
                return Err(SyntaxError::DuplicateSource);
            }
        }
        let origins_depth = OriginGraph::validation_depth(self.origins(), &store, b)?;
        let maps_depth = SourceMap::validation_depth(self.source_maps(), &store, b)?;
        Ok(ValidatedOwnerProvenance {
            owner: self,
            registry,
            depth: origins_depth.max(maps_depth),
        })
    }

    fn same_tables(&self, other: &Self) -> bool {
        // Exact borrowed slices identify immutable data, never portable owner
        // authority. Independent empty tables are equivalent here. Deep copies
        // on targets without pointer atomics require their own validation.
        core::ptr::eq(self.origins(), other.origins())
            && core::ptr::eq(self.sources(), other.sources())
            && core::ptr::eq(self.source_maps(), other.source_maps())
    }
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

// Own the immutable tables and borrow the registry for the proof's lifetime.
// Reconstructing a borrowed proof avoids self-referential storage.
struct PreparedOwner<'a> {
    provenance: OwnerProvenance,
    registry: &'a SchemaRegistry,
    depth: u64,
}

/// One immutable owner's captures share its complete provenance and owner
/// validation. Registry changes revalidate the owner. Each capture validates
/// guest and environment, current depth, and the explicit source admission.
pub struct ForeignCapture<'a> {
    owner: &'a ValidatedSyntaxBundle<'a>,
    prepared: Option<PreparedOwner<'a>>,
}
impl<'a> ForeignCapture<'a> {
    pub fn new(owner: &'a ValidatedSyntaxBundle<'a>) -> Self {
        Self {
            owner,
            prepared: None,
        }
    }

    pub fn capture_at(
        &mut self,
        node: NodeRef,
        field: usize,
        registry: &'a SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ForeignClosure, SyntaxError> {
        b.charge(Resource::Work, 1)?;
        let owner = self.owner.bundle();
        let Some(FieldValue::Foreign(syntax)) = owner.node(node)?.fields.get(field) else {
            return Err(SyntaxError::Reference);
        };
        if self.prepared.is_none() {
            if !registry.is_finalized() {
                return Err(crate::schema::SchemaError::Unfinalized.into());
            }
            // The checked owner already certifies these immutable source,
            // origin and mapping tables. Copying preserves their geometry and
            // relative depth. Guest contents and admission remain per capture.
            let provenance = OwnerProvenance::capture(owner, b)?;
            self.prepared = Some(PreparedOwner {
                provenance,
                registry,
                depth: self.owner.owner_depth,
            });
        }
        let prepared = self.prepared.as_mut().ok_or(SyntaxError::Reference)?;
        if !core::ptr::eq(prepared.registry, registry) {
            let depth = prepared.provenance.validate(registry, b, admission)?.depth;
            prepared.registry = registry;
            prepared.depth = depth;
        }
        let result = ForeignClosure::copy_selected(
            syntax,
            owner,
            prepared.provenance.clone_with_budget(b)?,
            b,
        )?;
        let proof = ValidatedOwnerProvenance {
            owner: &prepared.provenance,
            registry: prepared.registry,
            depth: prepared.depth,
        };
        if proof.matches_owner(&result.provenance) {
            proof.validate_closure(&result, b, admission)?;
        } else {
            // Non-atomic targets own deep copies; those copies establish their
            // own proof under the receiving operation's budget and admission.
            result.validate(registry, b, admission)?;
        }
        Ok(result)
    }
}
