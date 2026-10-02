//! Shared captured-stage lookup for plan execution and declaration candidates.
use super::*;

pub(crate) fn stage_at(
    stages: &[BindingStage],
    id: StageId,
) -> Result<&BindingStage, BindingError> {
    usize::try_from(id.0)
        .ok()
        .and_then(|i| stages.get(i))
        .ok_or(BindingError::Target)
}
pub(crate) fn entity<'a>(
    facts: &'a FactSet,
    id: EntityId,
    budget: &mut Budget,
) -> Result<&'a Entity, BindingError> {
    let values = &facts.entities;
    budget.charge(Resource::Work, 1)?;
    if let Some(value) = usize::try_from(id.0)
        .ok()
        .and_then(|i| values.get(i))
        .filter(|v| v.id == id)
    {
        return Ok(value);
    }
    budget.charge(Resource::Work, values.len() as u64)?;
    values
        .iter()
        .find(|v| v.id == id)
        .ok_or(BindingError::Target)
}
pub(crate) fn resolve(
    stages: &[BindingStage],
    facts: &FactSet,
    mut stage: StageId,
    namespace: NamespaceRef,
    name: &str,
    budget: &mut Budget,
) -> Result<ReferenceResolution, BindingError> {
    let mut scope = stage_at(stages, stage)?.scope;
    let mut candidates = Vec::new();
    loop {
        budget.charge(Resource::Work, 1)?;
        let current = stage_at(stages, stage)?;
        if current.scope != scope {
            if !candidates.is_empty() {
                break;
            }
            scope = current.scope;
        }
        for id in &current.introduced {
            budget.charge(Resource::Work, name.len() as u64 + 1)?;
            let entity = entity(facts, *id, budget)?;
            if entity.namespace == namespace && entity.name == name {
                budget.charge(Resource::Work, candidates.len() as u64 + 1)?;
                if !candidates.contains(id) {
                    push(&mut candidates, *id, budget)?;
                }
            }
        }
        match current.previous {
            Some(previous) => stage = previous,
            None => break,
        }
    }
    Ok(match candidates.len() {
        0 => ReferenceResolution::Unresolved(text(name, budget)?),
        1 => ReferenceResolution::Resolved(candidates[0]),
        _ => ReferenceResolution::Ambiguous(candidates),
    })
}
