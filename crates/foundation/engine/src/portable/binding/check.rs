use super::*;
use nepl3_core::{
    facts::{NamespacePolicy, OccurrenceRole, ReferenceResolution},
    origin::SourceMap,
    source::SourceError,
};
pub(super) fn validate<E>(
    data: &Data<'_>,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, PortableError<E>> {
    if let Some(facts) = data.facts {
        facts
            .validate(registry, b, admission)
            .map_err(crate::facts::FactsError::from)?;
    } else if !data.stages.is_empty()
        || !data.occurrences.is_empty()
        || !data.open_inputs.is_empty()
        || !data.exports.is_empty()
    {
        return Err(PortableError::Shape);
    }
    let store = source_closure(data, b, admission)?;
    SourceMap::validate_mappings(data.maps, &store, b).map_err(crate::facts::FactsError::from)?;
    let Some(facts) = data.facts else {
        return Ok(store);
    };
    for (index, stage) in data.stages.iter().enumerate() {
        b.charge(Resource::Work, facts.scopes.len() as u64 + 1)?;
        let scope = facts
            .scopes
            .iter()
            .find(|s| s.id == stage.scope)
            .ok_or(PortableError::Shape)?;
        if let Some(previous) = stage.previous {
            let previous_index = usize::try_from(previous.0).map_err(|_| PortableError::Shape)?;
            if previous_index >= index {
                return Err(PortableError::Shape);
            }
            let parent = &data.stages[previous_index];
            if parent.scope != stage.scope && Some(parent.scope) != scope.parent {
                return Err(PortableError::Shape);
            }
        } else if scope.parent.is_some() {
            return Err(PortableError::Shape);
        }
        let mut cursor = Some(index);
        let mut depth = 0u64;
        while let Some(i) = cursor {
            b.charge(Resource::Work, 1)?;
            depth = depth.saturating_add(1);
            b.observe_depth(depth)?;
            cursor = data.stages[i].previous.map(|v| v.0 as usize);
        }
        for (i, id) in stage.introduced.iter().enumerate() {
            b.charge(Resource::Work, (facts.entities.len() + i + 1) as u64)?;
            if !facts.entities.iter().any(|e| e.id == *id) || stage.introduced[..i].contains(id) {
                return Err(PortableError::Shape);
            }
        }
    }
    if data.occurrences.len() != facts.occurrences.len() {
        return Err(PortableError::Shape);
    }
    for (i, point) in data.occurrences.iter().enumerate() {
        b.charge(Resource::Work, (facts.occurrences.len() + i + 1) as u64)?;
        let occurrence = facts
            .occurrences
            .iter()
            .find(|v| v.id == point.occurrence)
            .ok_or(PortableError::Shape)?;
        let stage = usize::try_from(point.stage.0)
            .ok()
            .and_then(|i| data.stages.get(i))
            .ok_or(PortableError::Shape)?;
        if stage.scope != occurrence.scope
            || data.occurrences[..i]
                .iter()
                .any(|v| v.occurrence == point.occurrence)
        {
            return Err(PortableError::Shape);
        }
        let namespace = usize::try_from(occurrence.namespace.0)
            .ok()
            .and_then(|i| facts.namespaces.get(i))
            .ok_or(PortableError::Shape)?;
        let namespace_stage = usize::try_from(point.namespace_stage.0)
            .ok()
            .and_then(|i| data.stages.get(i))
            .ok_or(PortableError::Shape)?;
        match namespace.policy {
            NamespacePolicy::Lexical | NamespacePolicy::Open => {
                if point.namespace_stage != point.stage {
                    return Err(PortableError::Shape);
                }
            }
            NamespacePolicy::Global => {
                if namespace_stage.scope != namespace.root {
                    return Err(PortableError::Shape);
                }
            }
        }
    }
    for (i, id) in data.open_inputs.iter().enumerate() {
        b.charge(Resource::Work, (facts.occurrences.len() + i + 1) as u64)?;
        let occurrence = facts
            .occurrences
            .iter()
            .find(|v| v.id == *id)
            .ok_or(PortableError::Shape)?;
        let namespace = usize::try_from(occurrence.namespace.0)
            .ok()
            .and_then(|i| facts.namespaces.get(i))
            .ok_or(PortableError::Shape)?;
        if occurrence.role != OccurrenceRole::Reference
            || namespace.policy != NamespacePolicy::Open
            || !matches!(occurrence.resolution, ReferenceResolution::Unresolved(_))
            || data.open_inputs[..i].contains(id)
        {
            return Err(PortableError::Shape);
        }
    }
    for (i, id) in data.exports.iter().enumerate() {
        b.charge(Resource::Work, (facts.entities.len() + i + 1) as u64)?;
        if !facts.entities.iter().any(|e| e.id == *id) || data.exports[..i].contains(id) {
            return Err(PortableError::Shape);
        }
    }
    Ok(store)
}
pub(super) fn source_closure<E>(
    data: &Data<'_>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, PortableError<E>> {
    let mut store = SourceStore::default();
    if let Some(facts) = data.facts {
        add(&mut store, &facts.sources, b, admission)?;
    }
    add(&mut store, data.sources, b, admission)?;
    Ok(store)
}
fn add<E>(
    store: &mut SourceStore,
    sources: &[SourceSnapshot],
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<(), PortableError<E>> {
    for (i, source) in sources.iter().enumerate() {
        for prior in &sources[..i] {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() as u64)
                    .saturating_add(prior.identity().source.0.len() as u64)
                    .saturating_add(34),
            )?;
            if source.identity().source == prior.identity().source
                && source.identity().revision == prior.identity().revision
            {
                return Err(SourceError::IdentityConflict.into());
            }
        }
        admission.admit_existing(source, b)?;
        let mut existing = false;
        for prior in store.snapshots() {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() as u64)
                    .saturating_add(prior.identity().source.0.len() as u64)
                    .saturating_add(34),
            )?;
            if source.identity().source == prior.identity().source
                && source.identity().revision == prior.identity().revision
            {
                b.charge(
                    Resource::Work,
                    (source.uri().len() as u64)
                        .saturating_add(prior.uri().len() as u64)
                        .saturating_add(source.text().len() as u64)
                        .saturating_add(prior.text().len() as u64),
                )?;
                if source != prior {
                    return Err(SourceError::IdentityConflict.into());
                }
                existing = true;
            }
        }
        if !existing {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SourceSnapshot>() as u64,
            )?;
            store.insert(source.clone_with_budget(b)?)?;
        }
    }
    Ok(())
}
