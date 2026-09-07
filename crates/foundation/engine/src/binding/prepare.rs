use super::*;
use crate::recovery::ParseTree;
impl<'a, 'p> Machine<'a, 'p> {
    pub(super) fn prepare(
        &mut self,
        tree: &'a ParseTree,
        analysis_id: String,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<usize, BindingError> {
        // Admit and own all explicit tree sources before any diagnostic can refer to them.
        for context in &tree.contexts {
            let bundle = crate::tree::path(&tree.bundle, &context.path, self.registry, budget)?;
            for source in &bundle.sources {
                admission.admit_existing(source, budget)?;
                budget.charge(
                    Resource::Work,
                    (self.progress.sources.len() as u64)
                        .saturating_mul(source.identity().source.0.len() as u64 + 41),
                )?;
                if !self
                    .progress
                    .sources
                    .iter()
                    .any(|v| v.identity() == source.identity())
                {
                    push(
                        &mut self.progress.sources,
                        source.clone_with_budget(budget)?,
                        budget,
                    )?;
                }
            }
            for mapping in &bundle.source_maps {
                budget.charge(
                    Resource::Work,
                    (self.progress.source_maps.len() as u64).saturating_mul(
                        (mapping.source.snapshot_ref().source.0.len()
                            + mapping.target.snapshot_ref().source.0.len())
                            as u64
                            + 100,
                    ),
                )?;
                if !self.progress.source_maps.contains(mapping) {
                    push(
                        &mut self.progress.source_maps,
                        mapping.clone_with_budget(budget)?,
                        budget,
                    )?;
                }
            }
        }
        let mut facts = FactSet {
            analysis_id,
            namespaces: Vec::new(),
            scopes: Vec::new(),
            entities: Vec::new(),
            occurrences: Vec::new(),
            relations: Vec::new(),
            edges: Vec::new(),
            sources: Vec::new(),
            origins: Vec::new(),
            source_maps: Vec::new(),
        };
        for source in &self.progress.sources {
            push(
                &mut facts.sources,
                source.clone_with_budget(budget)?,
                budget,
            )?;
        }
        for mapping in &self.progress.source_maps {
            push(
                &mut facts.source_maps,
                mapping.clone_with_budget(budget)?,
                budget,
            )?;
        }
        self.progress.facts = Some(facts);
        let mut root = None;
        for context in &tree.contexts {
            let bundle = crate::tree::path(&tree.bundle, &context.path, self.registry, budget)?;
            let origin_base = self.facts()?.origins.len() as u64;
            // Origins may point forward inside this bundle. Publish the whole
            // rebased graph atomically, so a stopped partial FactSet stays closed.
            let mut origins = Vec::new();
            for origin in &bundle.origins {
                let mut origin = origin.clone_with_budget(budget)?;
                let inputs = match &mut origin {
                    Origin::Composite(ids) | Origin::Generated { inputs: ids, .. } => Some(ids),
                    _ => None,
                };
                if let Some(inputs) = inputs {
                    for id in inputs {
                        budget.charge(Resource::Work, 1)?;
                        id.0 = id.0.checked_add(origin_base).ok_or(BindingError::Target)?;
                    }
                }
                push(&mut origins, origin, budget)?;
            }
            budget.charge(
                Resource::AllocationUnits,
                (origins.len() as u64).saturating_mul(core::mem::size_of::<Origin>() as u64),
            )?;
            self.facts_mut()?.origins.append(&mut origins);
            let stage = self.scope(
                None,
                Some(OriginId(
                    origin_base
                        + bundle
                            .node(bundle.root)
                            .map_err(crate::tree::TreeError::from)?
                            .origin
                            .0,
                )),
                budget,
            )?;
            for selection in &context.nodes {
                let package = self.profile.language(&selection.entry.alias, budget)?;
                for namespace in &package.namespaces {
                    let scope = self.stage(stage)?.scope;
                    budget.charge(
                        Resource::Work,
                        (self.facts()?.namespaces.len() as u64).saturating_mul(
                            (package.schema.package.len() + namespace.name.len()) as u64 + 42,
                        ),
                    )?;
                    if self.facts()?.namespaces.iter().any(|v| {
                        v.schema == package.schema && v.name == namespace.name && v.root == scope
                    }) {
                        continue;
                    }
                    budget.charge(
                        Resource::AllocationUnits,
                        package.schema.package.len() as u64,
                    )?;
                    let fact = FactNamespace {
                        schema: package.schema.clone(),
                        name: text(&namespace.name, budget)?,
                        policy: match namespace.policy {
                            package::NamespacePolicy::Lexical => NamespacePolicy::Lexical,
                            package::NamespacePolicy::Global => NamespacePolicy::Global,
                            package::NamespacePolicy::Open => NamespacePolicy::Open,
                        },
                        root: scope,
                    };
                    push(&mut self.facts_mut()?.namespaces, fact, budget)?;
                }
            }
            if context.path.is_empty() {
                root = Some(self.bundles.len());
            }
            push(
                &mut self.bundles,
                LocalBundle {
                    bundle,
                    selections: &context.nodes,
                    origin_base,
                    root: stage,
                },
                budget,
            )?;
        }
        root.ok_or(BindingError::Target)
    }
}
