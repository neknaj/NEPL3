use super::*;
use nepl3_engine::{
    analysis::{
        expected::ExpectedReadRequest,
        insertion::name,
        probe::{
            candidates::{self, ProbeCandidateOutcome},
            read::{self, ReadOutcome},
        },
    },
    binding::probe::ProbeOutcome,
};

#[test]
fn traced_missing_prefix_preserves_exact_hit_consumers_and_nonhit_outcomes() -> Result<(), String> {
    let compiled = missing_probe::named_lambda()?;
    for input in [
        "lambda x",
        "let outer 1 guest lambda inner",
        "lambda x x",
        "1",
    ] {
        with_source_profile(&compiled, None, input, |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let tree = match &parsed {
                ParseCompletion::Continue(complete) => complete.tree(),
                ParseCompletion::Break(reply) => match &reply.outcome {
                    ParseOutcome::Complete { tree, .. } | ParseOutcome::Recovered { tree, .. } => {
                        tree
                    }
                    _ => return Err("tree".into()),
                },
            };
            let empty = SourceStore::default();
            let prepared = {
                let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
                keyed::prepare(
                    "birth-probe",
                    tree,
                    BindingOptions,
                    b.limits(),
                    profile,
                    &mut codec,
                    b,
                )
                .map_err(err)?
            };
            let mut measured = budget();
            let bound = prepared
                .probe_missing_reference_with_births(&mut measured, &mut SourceAdmission::default())
                .map_err(err)?;
            assert!(core::ptr::eq(bound.profile(), profile));
            let key = bound.probe().key();
            let report = bound.probe().reply().report.clone();
            if input.ends_with("lambda x") || input.ends_with("lambda inner") {
                let ProbeOutcome::Hit(hit) = &bound.probe().reply().outcome else {
                    return Err("hit".into());
                };
                let request = ExpectedReadRequest {
                    key,
                    source: source.reference(),
                    offset: input.len() as u64,
                };
                let correlation =
                    read::correlate(bound.probe(), &prepared, &request, b, a).map_err(err)?;
                let ReadOutcome::Hit(read) = correlation.outcome() else {
                    return Err("read hit".into());
                };
                assert!(core::ptr::eq(read.hit(), hit.as_ref()));
                let names = candidates::names(
                    bound.probe(),
                    &candidates::ProbeCandidateRequest {
                        key,
                        source: &request.source,
                        offset: request.offset,
                        prefix: "",
                    },
                    b,
                    a,
                )
                .map_err(err)?;
                let ProbeCandidateOutcome::Hit { candidates, .. } = names.outcome() else {
                    return Err("candidates".into());
                };
                assert!(!candidates.is_empty());
                let choice = name::select(&names, read, 0, b).map_err(err)?;
                assert!(core::ptr::eq(choice.read().reply(), bound.probe()));
                let ReferenceResolution::Resolved(id) = choice.candidate().resolution else {
                    return Err("candidate entity".into());
                };
                let BirthLookup::Born { entity, birth } =
                    bound.entity_birth(&key, id, b).map_err(err)?
                else {
                    return Err("prefix birth".into());
                };
                assert_eq!(entity.name, choice.candidate().name);
                assert_eq!(birth.entity, id);
                let mut stopped_budget = budget();
                stopped_budget
                    .charge(
                        Resource::AllocationUnits,
                        stopped_budget.limits().allocation_units
                            - measured.usage().allocation_units
                            + 1,
                    )
                    .map_err(err)?;
                let stopped = prepared
                    .probe_missing_reference_with_births(
                        &mut stopped_budget,
                        &mut SourceAdmission::default(),
                    )
                    .map_err(err)?;
                assert!(!stopped.births().is_empty());
                assert!(matches!(
                    stopped.probe().reply().outcome,
                    ProbeOutcome::Stopped(StopReason::AllocationLimit)
                ));
                assert!(matches!(
                    stopped.entity_birth(&key, id, &mut budget()),
                    Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete))
                ));
            } else {
                assert!(matches!(bound.probe().reply().outcome, ProbeOutcome::NoHit));
                assert!(matches!(
                    bound.entity_birth(&key, EntityId(0), &mut budget()),
                    Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete))
                ));
                assert_eq!(bound.births().is_empty(), input == "1");
            }
            for i in 0..4 {
                let mut stale = key;
                match i {
                    0 => stale.tree_digest.0[0] ^= 1,
                    1 => stale.profile_digest.0[0] ^= 1,
                    2 => stale.execution_digest.0[0] ^= 1,
                    _ => stale.request_digest.0[0] ^= 1,
                };
                assert!(matches!(
                    bound.entity_birth(&stale, EntityId(0), &mut budget()),
                    Err(EntityBirthAccessError::Access(
                        BindingAccessError::StaleAnalysis
                    ))
                ));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                bound.entity_birth(&key, EntityId(0), &mut cancelled),
                Err(EntityBirthAccessError::Access(BindingAccessError::Stopped(
                    StopReason::Cancelled
                )))
            ));
            let mut limits = b.limits();
            limits.depth -= 1;
            let mut mismatched = Budget::new(limits);
            mismatched.cancel();
            assert!(matches!(
                bound.entity_birth(&key, EntityId(0), &mut mismatched),
                Err(EntityBirthAccessError::Access(
                    BindingAccessError::LimitsMismatch
                ))
            ));
            assert_eq!(mismatched.usage().work, 0);
            assert_eq!(bound.probe().reply().report, report);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn blocked_prefix_after_birth_keeps_metadata_without_a_hit_certificate() -> Result<(), String> {
    let mut compiled = missing_probe::named_lambda_from(custom::compiled()?)?;
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
    with_input(&compiled, "lambda x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let prepared = {
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            keyed::prepare(
                "blocked-birth",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?
        };
        let bound = prepared
            .probe_missing_reference_with_births(b, a)
            .map_err(err)?;
        assert!(matches!(
            bound.probe().reply().outcome,
            ProbeOutcome::Blocked(BindingError::MissingProvider)
        ));
        assert_eq!(bound.births().len(), 1);
        assert!(matches!(
            bound.entity_birth(
                &bound.probe().key(),
                bound.births()[0].entity,
                &mut budget()
            ),
            Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete))
        ));
        let native = prepared.trace_named(b, a).map_err(err)?;
        assert!(matches!(
            native.references().trace().reply().outcome,
            BindingOutcome::Invalid {
                error: BindingError::MissingProvider,
                ..
            }
        ));
        assert_eq!(native.births().len(), 1);
        assert!(matches!(
            native.entity_birth(
                &native.references().key(),
                native.births()[0].entity,
                &mut budget()
            ),
            Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete))
        ));
        Ok(())
    })
}
