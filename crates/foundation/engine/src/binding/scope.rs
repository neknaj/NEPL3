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
        let id = ScopeId(next_id(&self.facts()?.scopes, |v| v.id.0, budget)?);
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
                let entity = self.entity(*id, budget)?;
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
    pub(super) fn namespace_stage(
        &self,
        bundle: usize,
        lexical: StageId,
        namespace: NamespaceRef,
    ) -> Result<StageId, BindingError> {
        let policy = self
            .facts()?
            .namespaces
            .get(namespace.0 as usize)
            .ok_or(BindingError::MissingNamespace)?
            .policy;
        if policy == NamespacePolicy::Global {
            Ok(self.bundles.get(bundle).ok_or(BindingError::Target)?.root)
        } else {
            Ok(lexical)
        }
    }
    pub(super) fn unique_global(
        &mut self,
        stage: StageId,
        namespace: NamespaceRef,
        name: &str,
        selection: &Span,
        allowed: Option<EntityId>,
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        self.unique_global_optional(stage, namespace, name, Some(selection), allowed, budget)
    }
    pub(super) fn unique_global_optional(
        &mut self,
        stage: StageId,
        namespace: NamespaceRef,
        name: &str,
        selection: Option<&Span>,
        allowed: Option<EntityId>,
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        let previous = match self.resolve(stage, namespace, name, budget)? {
            ReferenceResolution::Unresolved(_) => return Ok(()),
            ReferenceResolution::Resolved(id) if Some(id) == allowed => return Ok(()),
            ReferenceResolution::Resolved(id) => id,
            _ => return Err(BindingError::Target),
        };
        let previous = self.entity(previous, budget)?;
        let previous = previous
            .selection
            .as_ref()
            .or(previous.definition.as_ref())
            .map(|location| span(location, budget))
            .transpose()?;
        self.diagnostic_optional_at(
            "DuplicateGlobalName",
            namespace,
            name,
            (selection, previous.as_ref()),
            budget,
        )?;
        Err(BindingError::DuplicateGlobal)
    }
    pub(super) fn diagnostic(
        &mut self,
        code: &str,
        namespace: NamespaceRef,
        name: &str,
        span: &Span,
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        self.diagnostic_at(code, namespace, name, (span, None), budget)
    }
    pub(super) fn diagnostic_at(
        &mut self,
        code: &str,
        namespace: NamespaceRef,
        name: &str,
        (primary, related): (&Span, Option<&Span>),
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        self.diagnostic_optional_at(code, namespace, name, (Some(primary), related), budget)
    }
    pub(super) fn diagnostic_optional_at(
        &mut self,
        code: &str,
        namespace: NamespaceRef,
        name: &str,
        (primary, related): (Option<&Span>, Option<&Span>),
        budget: &mut Budget,
    ) -> Result<(), BindingError> {
        use nepl3_core::{
            diagnostic::{Diagnostic, Related, Severity},
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
        let arguments = TypedValue::Record(Record {
            schema: schema.clone(),
            kind: text("BindingDiagnosticArguments", budget)?,
            fields,
        });
        let mut related_locations = Vec::new();
        if let Some(location) = related {
            push(
                &mut related_locations,
                Related {
                    span: Some(super::span(location, budget)?),
                    code: text("PreviousDefinition", budget)?,
                    arguments: arguments.clone_with_budget(budget)?,
                },
                budget,
            )?;
        }
        let diagnostic = Diagnostic {
            schema: schema.clone(),
            code: text(code, budget)?,
            severity: Severity::Error,
            stage: text("binding", budget)?,
            arguments,
            primary: primary
                .map(|location| super::span(location, budget))
                .transpose()?,
            related: related_locations,
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
