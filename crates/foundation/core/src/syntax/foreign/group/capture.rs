use super::*;

/// Select a foreign field from the explicit input owner table. Numeric owner
/// indices belong to this capture call; they are not environment or Origin IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CaptureField {
    pub owner: u64,
    pub node: NodeRef,
    pub field: usize,
}
pub(super) fn push<T>(out: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    out.try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    out.push(value);
    Ok(())
}
pub(in crate::syntax::foreign) fn copy_provenance(
    origins: &[Origin],
    sources: &[SourceSnapshot],
    maps: &[Mapping],
    b: &mut Budget,
) -> Result<ForeignProvenance, StopReason> {
    let mut value = ForeignProvenance {
        origins: Vec::new(),
        sources: Vec::new(),
        source_maps: Vec::new(),
    };
    for origin in origins {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Origin>() as u64,
        )?;
        value.origins.push(origin.clone_with_budget(b)?);
    }
    for source in sources {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<SourceSnapshot>() as u64,
        )?;
        value.sources.push(source.clone_with_budget(b)?);
    }
    for map in maps {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Mapping>() as u64,
        )?;
        value.source_maps.push(map.clone_with_budget(b)?);
    }
    Ok(value)
}
impl ForeignClosureGroup {
    /// Capture all selected fields with complete, explicitly shared owner tables.
    /// The owner list must be unique by immutable input identity and fully used.
    /// Output order follows the supplied owner and field lists, never addresses.
    /// This local capture does not certify consumer semantic occurrence depths.
    pub fn capture(
        owners: &[&ValidatedSyntaxBundle<'_>],
        fields: &[CaptureField],
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, GroupError> {
        b.poll()?;
        // Pointer identity is used only to reject aliases within this live call.
        // No pointer, ordering by address, or ambient ownership enters the output.
        let sorted = crate::syntax::environment::sorted(
            owners.len(),
            |a, z, b| {
                b.charge(Resource::Work, 1)?;
                Ok((owners[a].bundle() as *const SyntaxBundle)
                    .cmp(&(owners[z].bundle() as *const SyntaxBundle)))
            },
            b,
        )?;
        for pair in sorted.windows(2) {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(owners[pair[0]].bundle(), owners[pair[1]].bundle()) {
                return Err(GroupError::DuplicateOwner {
                    first: pair[0] as u64,
                    second: pair[1] as u64,
                });
            }
        }
        let mut used = scratch(owners.len(), false, b)?;
        for field in fields {
            b.charge(Resource::Work, 1)?;
            let owner = index(field.owner, owners.len()).ok_or(GroupError::Owner(field.owner))?;
            used[owner] = true;
        }
        for (owner, used) in used.into_iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if !used {
                return Err(GroupError::UnusedProvenance(ProvenanceRef(owner as u64)));
            }
        }
        let mut group = Self {
            provenances: Vec::new(),
            environments: Vec::new(),
            entries: Vec::new(),
        };
        let mut orders = Vec::new();
        let mut cached = Vec::new();
        for owner in owners {
            let owner = owner.bundle();
            let order = crate::syntax::environment::sorted(
                owner.environments.len(),
                |a, z, b| {
                    b.charge(Resource::Work, 1)?;
                    Ok(owner.environments[a].id.cmp(&owner.environments[z].id))
                },
                b,
            )?;
            push(&mut orders, order, b)?;
            let slots = scratch(owner.environments.len(), None::<OwnerEnvironmentRef>, b)?;
            push(&mut cached, slots, b)?;
            let provenance =
                copy_provenance(&owner.origins, &owner.sources, &owner.source_maps, b)?;
            push(&mut group.provenances, provenance, b)?;
        }
        for field in fields {
            b.charge(Resource::Work, 1)?;
            let owner_number =
                index(field.owner, owners.len()).ok_or(GroupError::Owner(field.owner))?;
            let owner = owners[owner_number].bundle();
            let Some(FieldValue::Foreign(syntax)) = owner.node(field.node)?.fields.get(field.field)
            else {
                return Err(SyntaxError::Reference.into());
            };
            let order = &orders[owner_number];
            let (mut low, mut high) = (0, order.len());
            while low < high {
                b.charge(Resource::Work, 1)?;
                let mid = low + (high - low) / 2;
                if owner.environments[order[mid]].id < syntax.environment.id {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            let entry = *order.get(low).ok_or(SyntaxError::Environment)?;
            let environment = &owner.environments[entry];
            association(syntax, environment)?;
            let selected = match cached[owner_number][entry] {
                Some(reference) => reference,
                None => {
                    let reference = OwnerEnvironmentRef(group.environments.len() as u64);
                    let environment = environment.clone_with_budget(b)?;
                    push(
                        &mut group.environments,
                        ForeignOwnerEnvironment {
                            provenance: ProvenanceRef(field.owner),
                            environment,
                        },
                        b,
                    )?;
                    cached[owner_number][entry] = Some(reference);
                    reference
                }
            };
            let syntax = clone_syntax(syntax, b)?;
            push(
                &mut group.entries,
                ForeignGroupEntry {
                    owner: selected,
                    syntax,
                },
                b,
            )?;
        }
        group.validate(registry, b, admission)?;
        Ok(group)
    }
}
