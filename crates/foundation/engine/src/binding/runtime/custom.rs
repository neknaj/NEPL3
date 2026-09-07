use super::*;
use crate::facts::{FactsEmitter, FactsHeader, FactsPhase, FactsRequestView};
use nepl3_core::value::OperationRef;
mod history;
fn header(
    group: ScopeId,
    provider: &crate::profile::ProviderRequirement,
    request: FactsRequestView<'_>,
    delta: &FactDelta,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<FactsHeader, BindingError> {
    let mut entities = Vec::new();
    for entity in &delta.entities {
        push(&mut entities, entity.id, budget)?;
    }
    let mut exports = Vec::new();
    for occurrence in &delta.occurrences {
        budget.charge(Resource::Work, exports.len() as u64 + 1)?;
        if occurrence.role == OccurrenceRole::Export
            && let ReferenceResolution::Resolved(id) = occurrence.resolution
            && !exports.contains(&id)
        {
            push(&mut exports, id, budget)?;
        }
    }
    let target =
        crate::facts::phase::target(request.tree, request.path, request.node, registry, budget)?;
    let size = (provider.provider.len()
        + provider.operation.name.len()
        + provider.operation.schema.package.len()) as u64;
    budget.charge(Resource::Work, size + 80)?;
    budget.charge(
        Resource::AllocationUnits,
        size + core::mem::size_of::<FactsHeader>() as u64,
    )?;
    Ok(FactsHeader {
        group,
        provider: provider.clone(),
        target,
        entities,
        exports,
    })
}
fn publishes(delta: &FactDelta, id: EntityId, budget: &mut Budget) -> Result<bool, BindingError> {
    let mut export = false;
    let mut definition = false;
    for occurrence in &delta.occurrences {
        budget.charge(Resource::Work, 1)?;
        if occurrence.resolution == ReferenceResolution::Resolved(id) {
            match occurrence.role {
                OccurrenceRole::Definition | OccurrenceRole::Import => definition = true,
                OccurrenceRole::Export => export = true,
                OccurrenceRole::Reference => {}
            }
        }
    }
    Ok(!export || definition)
}

fn latest(
    stages: &[BindingStage],
    scope: ScopeId,
    budget: &mut Budget,
) -> Result<Option<StageId>, BindingError> {
    for (index, stage) in stages.iter().enumerate().rev() {
        budget.charge(Resource::Work, 1)?;
        if stage.scope == scope {
            return Ok(Some(StageId(index as u64)));
        }
    }
    Ok(None)
}
impl<'a, 'p> Machine<'a, 'p> {
    pub(super) fn custom(
        &mut self,
        frame: &mut Frame,
        tree: &'a crate::recovery::ParseTree,
        invocation: (&OperationRef, &FactsPhase),
        host: &mut Option<&mut dyn BindingHost>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<FactsHeader>, BindingError> {
        let (operation, phase) = invocation;
        budget.poll()?;
        let host = host.as_deref_mut().ok_or(BindingError::MissingProvider)?;
        let provider = self.profile.provider(operation, budget)?;
        let owner = self
            .bundles
            .get(frame.target.bundle)
            .ok_or(BindingError::Target)?
            .bundle;
        let mut path = None;
        for context in &tree.contexts {
            budget.charge(Resource::Work, 1)?;
            if core::ptr::eq(
                crate::tree::path(&tree.bundle, &context.path, self.registry, budget)?,
                owner,
            ) {
                path = Some(context.path.as_slice());
                break;
            }
        }
        let path = path.ok_or(BindingError::Target)?;
        let call = BindingCall {
            provider,
            tree,
            path,
            node: frame.target.node,
            scope: self.stage(frame.stage)?.scope,
            existing: self.facts()?,
            phase,
        };
        let authority = host
            .authorize(&call, budget)?
            .ok_or(BindingError::MissingProvider)?;
        budget.poll()?;
        let result = {
            let existing = self.progress.facts.as_ref().ok_or(BindingError::Target)?;
            let request = FactsRequestView {
                tree,
                path,
                node: frame.target.node,
                existing,
                authority: &authority,
                phase,
            }
            .issue(self.profile, budget, admission)?;
            let mut emit = FactsEmitter::new(
                self.registry,
                &mut self.report,
                &mut self.progress.sources,
                (
                    &mut self.progress.source_maps,
                    &mut self
                        .progress
                        .bundle_scopes
                        .get_mut(frame.target.bundle)
                        .ok_or(BindingError::Target)?
                        .custom_source_maps,
                ),
                budget,
                admission,
            );
            host.facts(provider, &request, &mut emit)
        };
        // Only values emitted through the checked sink survive an already
        // stopped callback. Its remaining raw delta has no acceptance proof.
        budget.poll()?;
        let (delta, failure) = match result? {
            CustomOutcome::Complete(delta) => (Some(delta), None),
            CustomOutcome::Invalid(delta) => (delta, Some(BindingError::ProviderInvalid)),
            CustomOutcome::Stopped { reason, partial } => {
                (partial, Some(BindingError::Stopped(reason)))
            }
        };
        let mut receipt = None;
        if let Some(delta) = delta {
            let request = FactsRequestView {
                tree,
                path,
                node: frame.target.node,
                existing: self.facts()?,
                authority: &authority,
                phase,
            };
            let base = request
                .existing
                .validate(self.registry, budget, admission)?;
            delta.validate(&base, &authority, budget, admission)?;
            crate::facts::phase::delta(phase, &delta, budget)?;
            if let FactsPhase::Header { group } = phase {
                receipt = Some(header(
                    *group,
                    provider,
                    request,
                    &delta,
                    self.registry,
                    budget,
                )?);
            }
            crate::facts::check::closure_view(request, Some(&delta), &[], &[], budget, admission)?;
            let mut writable = Vec::new();
            for scope in &authority.writable_scopes {
                push(&mut writable, *scope, budget)?;
            }
            let history = self.history(
                &delta,
                history::Invocation {
                    tree,
                    path,
                    node: frame.target.node,
                    provider,
                    authority,
                },
                budget,
            )?;
            self.apply_custom(frame, delta, history, &writable, budget)?;
            if let FactsPhase::Body { header } = phase {
                for id in &header.exports {
                    budget.charge(Resource::Work, frame.exports.len() as u64 + 1)?;
                    if !frame.exports.contains(id) {
                        push(&mut frame.exports, *id, budget)?;
                    }
                }
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(receipt),
        }
    }
    fn apply_custom(
        &mut self,
        frame: &mut Frame,
        mut delta: FactDelta,
        history: Option<BindingResolutionBatch>,
        writable: &[ScopeId],
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        // Prepare the report's owned source closure separately from FactSet.
        let mut sources = Vec::new();
        for source in &delta.sources {
            budget.charge(
                Resource::Work,
                self.progress.sources.len() as u64 * (source.identity().source.0.len() as u64 + 41),
            )?;
            if !self
                .progress
                .sources
                .iter()
                .any(|v| v.identity() == source.identity())
            {
                push(&mut sources, source.clone_with_budget(budget)?, budget)?;
            }
        }
        let mut maps = Vec::new();
        for map in &delta.source_maps {
            push(&mut maps, map.clone_with_budget(budget)?, budget)?;
        }
        budget.charge(
            Resource::AllocationUnits,
            (sources.len() as u64)
                .saturating_mul(core::mem::size_of::<nepl3_core::source::SourceSnapshot>() as u64)
                .saturating_add(
                    (maps.len() as u64)
                        .saturating_mul(core::mem::size_of::<nepl3_core::origin::Mapping>() as u64),
                ),
        )?;
        let owner = self
            .progress
            .bundle_scopes
            .get_mut(frame.target.bundle)
            .ok_or(BindingError::Target)?;
        budget.charge(Resource::Work, maps.len() as u64)?;
        budget.charge(
            Resource::AllocationUnits,
            (maps.len() * core::mem::size_of::<u64>()) as u64,
        )?;
        for index in 0..maps.len() {
            owner
                .custom_source_maps
                .push((self.progress.source_maps.len() + index) as u64);
        }
        // These source declarations/maps were independently validated while the
        // budget ran. Retain them before emitting a semantic duplicate error.
        self.progress.sources.append(&mut sources);
        self.progress.source_maps.append(&mut maps);
        // Structural FactDelta authority does not waive a namespace's semantic
        // duplicate rule. Check both earlier publications and this same batch.
        for (index, entity) in delta.entities.iter().enumerate() {
            let ns = self
                .facts()?
                .namespaces
                .get(entity.namespace.0 as usize)
                .ok_or(BindingError::MissingNamespace)?;
            if ns.policy != NamespacePolicy::Global {
                continue;
            }
            let root = ns.root;
            if entity.scope != root {
                return Err(BindingError::NamespaceBoundary);
            }
            if !publishes(&delta, entity.id, budget)? {
                continue;
            }
            let current =
                latest(&self.progress.stages, root, budget)?.ok_or(BindingError::Target)?;
            let selection = entity.selection.as_ref().or(entity.definition.as_ref());
            self.unique_global_optional(
                current,
                entity.namespace,
                &entity.name,
                selection,
                None,
                budget,
            )?;
            for prior in &delta.entities[..index] {
                budget.charge(
                    Resource::Work,
                    prior.name.len() as u64 + entity.name.len() as u64 + 2,
                )?;
                if prior.namespace == entity.namespace
                    && prior.name == entity.name
                    && publishes(&delta, prior.id, budget)?
                {
                    self.diagnostic_optional_at(
                        "DuplicateGlobalName",
                        entity.namespace,
                        &entity.name,
                        (
                            selection,
                            prior.selection.as_ref().or(prior.definition.as_ref()),
                        ),
                        budget,
                    )?;
                    return Err(BindingError::DuplicateGlobal);
                }
            }
        }
        for occurrence in &delta.occurrences {
            if occurrence.role != OccurrenceRole::Import {
                continue;
            }
            let ns = self
                .facts()?
                .namespaces
                .get(occurrence.namespace.0 as usize)
                .ok_or(BindingError::MissingNamespace)?;
            if ns.policy == NamespacePolicy::Global {
                budget.charge(Resource::Work, writable.len() as u64)?;
                if !writable.contains(&ns.root) {
                    return Err(BindingError::Fact(FactError::Authority));
                }
                let ReferenceResolution::Resolved(id) = occurrence.resolution else {
                    return Err(BindingError::Fact(FactError::Resolution));
                };
                let current =
                    latest(&self.progress.stages, ns.root, budget)?.ok_or(BindingError::Target)?;
                self.unique_global_optional(
                    current,
                    occurrence.namespace,
                    &occurrence.name,
                    Some(&occurrence.span),
                    Some(id),
                    budget,
                )?;
            }
        }
        let mut resolutions = Vec::new();
        for update in &delta.resolutions {
            budget.charge(Resource::Work, self.facts()?.occurrences.len() as u64)?;
            let index = self
                .facts()?
                .occurrences
                .iter()
                .position(|v| v.id == update.occurrence)
                .ok_or(BindingError::Fact(FactError::MissingOccurrence))?;
            push(
                &mut resolutions,
                (index, history::resolution(&update.resolution, budget)?),
                budget,
            )?;
        }
        let mut open_inputs = Vec::new();
        for occurrence in self.facts()?.occurrences.iter().chain(&delta.occurrences) {
            budget.charge(Resource::Work, delta.resolutions.len() as u64 + 1)?;
            let resolution = delta
                .resolutions
                .iter()
                .find(|v| v.occurrence == occurrence.id)
                .map_or(&occurrence.resolution, |v| &v.resolution);
            if occurrence.role == OccurrenceRole::Reference
                && self
                    .facts()?
                    .namespaces
                    .get(occurrence.namespace.0 as usize)
                    .ok_or(BindingError::MissingNamespace)?
                    .policy
                    == NamespacePolicy::Open
                && matches!(resolution, ReferenceResolution::Unresolved(_))
            {
                push(&mut open_inputs, occurrence.id, budget)?;
            }
        }
        let offset = self.progress.stages.len() as u64;
        let mut stages = Vec::<BindingStage>::new();
        let current_scope = self.stage(frame.stage)?.scope;
        let find = |scope,
                    added: &[BindingStage],
                    budget: &mut Budget|
         -> Result<Option<StageId>, BindingError> {
            if let Some(stage) = latest(added, scope, budget)? {
                return Ok(Some(StageId(offset + stage.0)));
            }
            if scope == current_scope {
                return Ok(Some(frame.stage));
            }
            latest(&self.progress.stages, scope, budget)
        };
        let mut remaining: Vec<_> = Vec::new();
        for scope in &delta.scopes {
            push(&mut remaining, scope, budget)?;
        }
        while !remaining.is_empty() {
            let mut progressed = false;
            let mut index = 0;
            while index < remaining.len() {
                budget.charge(Resource::Work, 1)?;
                let scope = remaining[index];
                let parent = scope
                    .parent
                    .ok_or(BindingError::Fact(FactError::Authority))?;
                if let Some(previous) = find(parent, &stages, budget)? {
                    push(
                        &mut stages,
                        BindingStage {
                            scope: scope.id,
                            previous: Some(previous),
                            introduced: Vec::new(),
                        },
                        budget,
                    )?;
                    remaining.swap_remove(index);
                    progressed = true;
                } else {
                    index += 1;
                }
            }
            if !progressed {
                return Err(BindingError::Fact(FactError::Cycle));
            }
        }
        let mut exports = Vec::new();
        for entity in &delta.entities {
            let mut export_only = false;
            let mut defined = false;
            for occurrence in &delta.occurrences {
                budget.charge(Resource::Work, 1)?;
                if occurrence.resolution == ReferenceResolution::Resolved(entity.id) {
                    match occurrence.role {
                        OccurrenceRole::Definition | OccurrenceRole::Import => defined = true,
                        OccurrenceRole::Export => export_only = true,
                        OccurrenceRole::Reference => {}
                    }
                }
            }
            if export_only && !defined {
                continue;
            }
            let previous = find(entity.scope, &stages, budget)?.ok_or(BindingError::Target)?;
            let mut introduced = Vec::new();
            push(&mut introduced, entity.id, budget)?;
            push(
                &mut stages,
                BindingStage {
                    scope: entity.scope,
                    previous: Some(previous),
                    introduced,
                },
                budget,
            )?;
        }
        for occurrence in &delta.occurrences {
            let ReferenceResolution::Resolved(id) = occurrence.resolution else {
                continue;
            };
            match occurrence.role {
                OccurrenceRole::Export => {
                    budget.charge(Resource::Work, exports.len() as u64)?;
                    if !exports.contains(&id) {
                        push(&mut exports, id, budget)?;
                    }
                }
                OccurrenceRole::Import => {
                    let ns = self
                        .facts()?
                        .namespaces
                        .get(occurrence.namespace.0 as usize)
                        .ok_or(BindingError::MissingNamespace)?;
                    let scope = if ns.policy == NamespacePolicy::Global {
                        ns.root
                    } else {
                        occurrence.scope
                    };
                    let previous = find(scope, &stages, budget)?.ok_or(BindingError::Target)?;
                    let mut introduced = Vec::new();
                    push(&mut introduced, id, budget)?;
                    push(
                        &mut stages,
                        BindingStage {
                            scope,
                            previous: Some(previous),
                            introduced,
                        },
                        budget,
                    )?;
                }
                _ => {}
            }
        }
        let mut occurrences = Vec::new();
        for occurrence in &delta.occurrences {
            let stage = find(occurrence.scope, &stages, budget)?.ok_or(BindingError::Target)?;
            let ns = self
                .facts()?
                .namespaces
                .get(occurrence.namespace.0 as usize)
                .ok_or(BindingError::MissingNamespace)?;
            let namespace_stage = if ns.policy == NamespacePolicy::Global {
                find(ns.root, &stages, budget)?.ok_or(BindingError::Target)?
            } else {
                stage
            };
            push(
                &mut occurrences,
                OccurrenceStage {
                    occurrence: occurrence.id,
                    stage,
                    namespace_stage,
                },
                budget,
            )?;
        }
        let next = find(current_scope, &stages, budget)?.ok_or(BindingError::Target)?;
        let mut root_updates = Vec::new();
        for (index, bundle) in self.bundles.iter().enumerate() {
            let scope = self.stage(bundle.root)?.scope;
            if let Some(stage) = find(scope, &stages, budget)? {
                push(&mut root_updates, (index, stage), budget)?;
            }
        }
        // Charge destination rows before changing any ID/reference column.
        let bytes = delta.scopes.len() * core::mem::size_of::<Scope>()
            + delta.entities.len() * core::mem::size_of::<Entity>()
            + delta.occurrences.len() * core::mem::size_of::<Occurrence>()
            + delta.relations.len() * core::mem::size_of::<Relation>()
            + delta.edges.len() * core::mem::size_of::<ScopeEdge>()
            + delta.origins.len() * core::mem::size_of::<Origin>()
            + delta.sources.len() * core::mem::size_of::<nepl3_core::source::SourceSnapshot>()
            + delta.source_maps.len() * core::mem::size_of::<nepl3_core::origin::Mapping>()
            + stages.len() * core::mem::size_of::<BindingStage>()
            + occurrences.len() * core::mem::size_of::<OccurrenceStage>()
            + exports.len() * core::mem::size_of::<EntityId>();
        budget.charge(Resource::AllocationUnits, bytes as u64)?;
        if history.is_some() {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<BindingResolutionBatch>() as u64,
            )?;
        }
        let facts = self.facts_mut()?;
        for (index, resolution) in resolutions {
            facts.occurrences[index].resolution = resolution;
        }
        facts.scopes.append(&mut delta.scopes);
        facts.entities.append(&mut delta.entities);
        facts.occurrences.append(&mut delta.occurrences);
        facts.relations.append(&mut delta.relations);
        facts.edges.append(&mut delta.edges);
        facts.origins.append(&mut delta.origins);
        facts.sources.append(&mut delta.sources);
        facts.source_maps.append(&mut delta.source_maps);
        self.progress.stages.append(&mut stages);
        self.progress.occurrence_stages.append(&mut occurrences);
        self.progress.open_inputs = open_inputs;
        if let Some(history) = history {
            self.progress.resolution_history.push(history);
        }
        frame.exports.append(&mut exports);
        frame.stage = next;
        for (index, stage) in root_updates {
            self.bundles[index].root = stage;
        }
        Ok(())
    }
}
