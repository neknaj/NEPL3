use super::*;
use nepl3_engine::facts::{CheckedFactsView, FactsEmitter};

struct RetargetDeclarations<H>(H);
impl<H: BindingHost> BindingHost for RetargetDeclarations<H> {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        let Some(mut authority) = self.0.authorize(call, b)? else {
            return Ok(None);
        };
        for occurrence in &call.existing.occurrences {
            b.charge(Resource::Work, occurrence.name.len() as u64 + 1)?;
            if matches!(
                occurrence.role,
                OccurrenceRole::Definition | OccurrenceRole::Export
            ) && occurrence.name == "x"
            {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<OccurrenceId>() as u64,
                )?;
                authority.resolution_updates.push(occurrence.id);
            }
        }
        Ok(Some(authority))
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        request: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        self.0.facts(provider, request, emit)
    }
}

#[test]
fn later_definition_and_export_updates_do_not_rewrite_entity_births() -> Result<(), String> {
    for export in [false, true] {
        let mut compiled = if export {
            custom::compiled()?
        } else {
            missing_probe::named_lambda_from(custom::compiled()?)?
        };
        if !export {
            let custom_id = compiled
                .package
                .bindings
                .iter()
                .position(|v| matches!(v, nepl3_engine::package::Binding::Custom(_)))
                .ok_or("custom")?;
            let lambda = compiled
                .package
                .forms
                .iter()
                .find(|v| v.spelling == "lambda")
                .ok_or("lambda")?;
            let nepl3_engine::package::Binding::Scope(actions) =
                &mut compiled.package.bindings[lambda.binding.0 as usize]
            else {
                return Err("scope".into());
            };
            actions.push(nepl3_engine::package::BindingId(custom_id as u64));
        }
        let input = if export {
            "recursive cons define x custom x 1 nil x"
        } else {
            "lambda x x"
        };
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut host = RetargetDeclarations(custom::query_host(true, false));
            let trace = named::analyze(
                "retarget-birth",
                tree,
                profile,
                Some(&mut host),
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let (analysis, _) = trace
                .references()
                .complete()
                .ok_or_else(|| format!("{:?}", trace.references().reply().outcome))?;
            assert_eq!(trace.births().len(), 1);
            let birth = &trace.births()[0];
            assert_eq!(birth.entity, EntityId(0));
            let role = if export {
                OccurrenceRole::Export
            } else {
                OccurrenceRole::Definition
            };
            let original_occurrence = analysis
                .facts()
                .occurrences
                .iter()
                .find(|v| v.role == role && v.id != OccurrenceId(200))
                .ok_or("original occurrence")?;
            assert_eq!(
                original_occurrence.resolution,
                ReferenceResolution::Resolved(EntityId(100))
            );
            let BirthLookup::Born {
                entity,
                birth: matched,
            } = trace
                .entity_birth(EntityId(0), &mut budget())
                .map_err(err)?
            else {
                return Err("born".into());
            };
            assert_eq!(entity.id, EntityId(0));
            assert!(core::ptr::eq(birth, matched));
            assert!(matches!(
                trace
                    .entity_birth(EntityId(100), &mut budget())
                    .map_err(err)?,
                BirthLookup::Untraced(_)
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn sparse_provider_entities_do_not_turn_entity_ids_into_row_indices() -> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "custom x lambda y y", |tree, profile, _, _| {
        let mut host = custom::query_host(false, false);
        let trace = named::analyze(
            "sparse-birth",
            tree,
            profile,
            Some(&mut host),
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(trace.references().complete().is_some());
        assert_eq!(trace.births().len(), 1);
        assert_eq!(trace.births()[0].entity, EntityId(101));
        assert!(
            matches!(trace.entity_birth(EntityId(101),&mut budget()).map_err(err)?,BirthLookup::Born{entity,..} if entity.name=="y")
        );
        assert!(matches!(
            trace
                .entity_birth(EntityId(100), &mut budget())
                .map_err(err)?,
            BirthLookup::Untraced(_)
        ));
        assert!(matches!(
            trace.entity_birth(EntityId(0), &mut budget()),
            Err(BirthAccessError::MissingEntity)
        ));
        Ok(())
    })
}
