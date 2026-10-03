use super::*;
use nepl3_engine::facts::{CheckedFactsView, FactsEmitter};

pub(super) struct RetargetBuiltIn<H>(pub H);
impl<H: BindingHost> BindingHost for RetargetBuiltIn<H> {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        self.0.authorize(call, b)
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let outcome = self.0.facts(provider, checked, emit)?;
        let CustomOutcome::Complete(mut delta) = outcome else {
            return Ok(outcome);
        };
        let request = checked.request();
        let mut target = None;
        for entity in &request.existing.entities {
            emit.budget()
                .charge(Resource::Work, entity.name.len() as u64 + 1)?;
            if entity.name == "x" && target.is_none() {
                target = Some(entity.id);
            }
        }
        let target = target.ok_or(BindingError::Target)?;
        for update in &mut delta.resolutions {
            emit.budget().charge(Resource::Work, 1)?;
            update.resolution = ReferenceResolution::Resolved(target);
        }
        Ok(CustomOutcome::Complete(delta))
    }
}

pub(super) struct ReportDiagnostic<H> {
    pub inner: H,
    pub severity: nepl3_core::diagnostic::Severity,
}
impl<H: BindingHost> BindingHost for ReportDiagnostic<H> {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        self.inner.authorize(call, b)
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let schema = checked
            .profile()
            .registry()
            .selected("nepl3.engine", 1)
            .ok_or(BindingError::Target)?
            .clone();
        emit.diagnostic(nepl3_core::diagnostic::Diagnostic {
            schema: schema.clone(),
            code: "QualityFixture".into(),
            severity: self.severity,
            stage: "binding".into(),
            arguments: nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                schema,
                kind: "BindingDiagnosticArguments".into(),
                fields: vec![NdfValue::Text("Value".into()), NdfValue::Text("x".into())],
            }),
            primary: None,
            related: vec![],
            fixes: vec![],
        })?;
        self.inner.facts(provider, checked, emit)
    }
}

#[derive(Clone, Copy)]
pub(super) enum ReferenceFixture {
    Unresolved,
    Ambiguous,
    Deferred,
}
pub(super) struct ExtraReference<H> {
    pub inner: H,
    pub kind: ReferenceFixture,
}
impl<H: BindingHost> BindingHost for ExtraReference<H> {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        self.inner.authorize(call, b)
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let outcome = self.inner.facts(provider, checked, emit)?;
        let CustomOutcome::Complete(mut delta) = outcome else {
            return Ok(outcome);
        };
        // Fixed fixture payload storage, in addition to the independently charged span.
        emit.budget().charge(Resource::AllocationUnits, 2048)?;
        emit.budget().charge(Resource::Work, 128)?;
        let existing = delta.occurrences.first().ok_or(BindingError::Target)?;
        let span = existing.span.clone_with_budget(emit.budget())?;
        let scope = existing.scope;
        let namespace = existing.namespace;
        let resolution = match self.kind {
            ReferenceFixture::Unresolved => ReferenceResolution::Unresolved("x".into()),
            ReferenceFixture::Ambiguous => {
                delta.entities.push(Entity {
                    id: EntityId(101),
                    scope,
                    namespace,
                    name: "x".into(),
                    definition: None,
                    selection: None,
                    origin: None,
                });
                ReferenceResolution::Ambiguous(vec![EntityId(100), EntityId(101)])
            }
            ReferenceFixture::Deferred => {
                let schema = checked
                    .profile()
                    .registry()
                    .selected("nepl3.engine", 1)
                    .ok_or(BindingError::Target)?
                    .clone();
                ReferenceResolution::Deferred(vec![nepl3_core::value::TypedValue::Record(
                    nepl3_core::value::Record {
                        schema,
                        kind: "BindingDiagnosticArguments".into(),
                        fields: vec![NdfValue::Text("Value".into()), NdfValue::Text("x".into())],
                    },
                )])
            }
        };
        delta.occurrences.push(Occurrence {
            id: OccurrenceId(201),
            scope,
            namespace,
            name: "x".into(),
            role: OccurrenceRole::Reference,
            span,
            origin: None,
            resolution,
        });
        Ok(CustomOutcome::Complete(delta))
    }
}
