use super::*;
use capture::push;

impl ValidatedForeignGroup<'_> {
    /// Copy selected entries in request order, sharing each selected owner once.
    /// Repeated entry selections remain repeated; owner tables retain input order.
    /// Only group-local references are remapped. Complete selected provenance and
    /// original environment IDs/digests are preserved. Validate the new closure
    /// under this call's registry, admission and Budget; no old proof is returned.
    /// The consuming language still checks its own semantic occurrence depths.
    pub fn select(
        &self,
        entries: &[u64],
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ForeignClosureGroup, GroupError> {
        b.poll()?;
        let value = self.value;
        let mut environments = scratch(value.environments.len(), None::<u64>, b)?;
        let mut provenances = scratch(value.provenances.len(), None::<u64>, b)?;
        for &number in entries {
            b.charge(Resource::Work, 1)?;
            let entry = value
                .entries
                .get(index(number, value.entries.len()).ok_or(GroupError::Entry(number))?)
                .ok_or(GroupError::Entry(number))?;
            let env = index(entry.owner.0, value.environments.len())
                .ok_or(GroupError::Environment(entry.owner))?;
            let owner = &value.environments[env];
            let provenance = index(owner.provenance.0, value.provenances.len())
                .ok_or(GroupError::Provenance(owner.provenance))?;
            environments[env] = Some(0);
            provenances[provenance] = Some(0);
        }
        let mut result = ForeignClosureGroup {
            provenances: Vec::new(),
            environments: Vec::new(),
            entries: Vec::new(),
        };
        for (number, owner) in value.provenances.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if provenances[number].is_some() {
                provenances[number] = Some(result.provenances.len() as u64);
                let owner = copy_provenance(&owner.origins, &owner.sources, &owner.source_maps, b)?;
                push(&mut result.provenances, owner, b)?;
            }
        }
        for (number, owner) in value.environments.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if environments[number].is_some() {
                environments[number] = Some(result.environments.len() as u64);
                let provenance = index(owner.provenance.0, provenances.len())
                    .and_then(|i| provenances[i])
                    .ok_or(GroupError::Provenance(owner.provenance))?;
                let owner = ForeignOwnerEnvironment {
                    provenance: ProvenanceRef(provenance),
                    environment: owner.environment.clone_with_budget(b)?,
                };
                push(&mut result.environments, owner, b)?;
            }
        }
        for &number in entries {
            b.charge(Resource::Work, 1)?;
            let entry = value
                .entries
                .get(index(number, value.entries.len()).ok_or(GroupError::Entry(number))?)
                .ok_or(GroupError::Entry(number))?;
            let owner = index(entry.owner.0, environments.len())
                .and_then(|i| environments[i])
                .ok_or(GroupError::Environment(entry.owner))?;
            let entry = ForeignGroupEntry {
                owner: OwnerEnvironmentRef(owner),
                syntax: clone_syntax(&entry.syntax, b)?,
            };
            push(&mut result.entries, entry, b)?;
        }
        result.validate(registry, b, admission)?;
        Ok(result)
    }
}
