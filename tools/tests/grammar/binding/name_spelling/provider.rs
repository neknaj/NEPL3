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
