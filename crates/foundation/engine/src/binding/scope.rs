use super::*;
impl Machine<'_, '_> {
    pub(super) fn stage(&self, id: StageId) -> Result<&BindingStage, BindingError> {
        self.progress
            .stages
            .get(id.0 as usize)
            .ok_or(BindingError::Target)
    }
    pub(super) fn scope(
        &mut self,
        parent: Option<StageId>,
        origin: Option<OriginId>,
        budget: &mut Budget,
    ) -> Result<StageId, BindingError> {
        let id = ScopeId(self.facts()?.scopes.len() as u64);
        let parent_scope = parent
            .map(|id| self.stage(id).map(|v| v.scope))
            .transpose()?;
        push(
            &mut self.facts_mut()?.scopes,
            Scope {
                id,
                parent: parent_scope,
                origin,
            },
            budget,
        )?;
        let stage = StageId(self.progress.stages.len() as u64);
        push(
            &mut self.progress.stages,
            BindingStage {
                scope: id,
                previous: parent,
                introduced: Vec::new(),
            },
            budget,
        )?;
        Ok(stage)
    }
    pub(super) fn introduce(
        &mut self,
        stage: StageId,
        entity: EntityId,
        budget: &mut Budget,
    ) -> Result<StageId, BindingError> {
        let scope = self.stage(stage)?.scope;
        let mut introduced = Vec::new();
        push(&mut introduced, entity, budget)?;
        let id = StageId(self.progress.stages.len() as u64);
        push(
            &mut self.progress.stages,
            BindingStage {
                scope,
                previous: Some(stage),
                introduced,
            },
            budget,
        )?;
        Ok(id)
    }
    pub(super) fn namespace(
        &self,
        bundle: usize,
        package: &LanguagePackage,
        name: &str,
        budget: &mut Budget,
    ) -> Result<NamespaceRef, BindingError> {
        let root = self
            .stage(self.bundles.get(bundle).ok_or(BindingError::Target)?.root)?
            .scope;
        for (i, namespace) in self.facts()?.namespaces.iter().enumerate() {
            budget.charge(
                Resource::Work,
                (name.len() + package.schema.package.len()) as u64 + 42,
            )?;
            if namespace.schema == package.schema
                && namespace.name == name
                && namespace.root == root
            {
                return Ok(NamespaceRef(i as u64));
            }
        }
        Err(BindingError::MissingNamespace)
    }
    pub(super) fn resolve(
        &self,
        mut stage: StageId,
        namespace: NamespaceRef,
        name: &str,
        budget: &mut Budget,
    ) -> Result<ReferenceResolution, BindingError> {
        let mut scope = self.stage(stage)?.scope;
        let mut candidates = Vec::new();
        loop {
            budget.charge(Resource::Work, 1)?;
            let current = self.stage(stage)?;
            if current.scope != scope {
                if !candidates.is_empty() {
                    break;
                }
                scope = current.scope;
            }
            for id in &current.introduced {
                budget.charge(Resource::Work, name.len() as u64 + 1)?;
                let entity = self
                    .facts()?
                    .entities
                    .get(id.0 as usize)
                    .ok_or(BindingError::Target)?;
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
    pub(super) fn diagnostic(
        &mut self,
        code: &str,
        namespace: NamespaceRef,
        name: &str,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        use nepl3_core::{
            diagnostic::{Diagnostic, Severity},
            value::{Record, TypedValue},
        };
        let schema = self
            .registry
            .selected("nepl3.engine", 1)
            .ok_or(nepl3_core::schema::SchemaError::UnknownSchema)?;
        budget.charge(Resource::AllocationUnits, schema.package.len() as u64 * 2)?;
        let mut fields = Vec::new();
        let namespace_name = &self
            .facts()?
            .namespaces
            .get(namespace.0 as usize)
            .ok_or(BindingError::MissingNamespace)?
            .name;
        push(
            &mut fields,
            NdfValue::Text(text(namespace_name, budget)?),
            budget,
        )?;
        push(&mut fields, NdfValue::Text(text(name, budget)?), budget)?;
        let diagnostic = Diagnostic {
            schema: schema.clone(),
            code: text(code, budget)?,
            severity: Severity::Error,
            stage: text("binding", budget)?,
            arguments: TypedValue::Record(Record {
                schema: schema.clone(),
                kind: text("BindingDiagnosticArguments", budget)?,
                fields,
            }),
            primary: Some(super::span(span, budget)?),
            related: Vec::new(),
            fixes: Vec::new(),
        };
        // All arguments are schema-owned and the primary comes from the admitted tree.
        self.registry
            .validate_typed(&diagnostic.arguments, budget)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Diagnostic>() as u64,
        )?;
        budget.charge(Resource::Diagnostics, 1)?;
        self.report.diagnostics.push(diagnostic);
        Ok(())
    }
}
