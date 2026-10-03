use super::*;
use nepl3_engine::{
    analysis::{BindingAccessError, BindingOptions, trace::named::EntityBirthAccessError},
    binding::trace::{
        birth::{BirthAccessError, BirthKind, BirthLookup, BirthPhase},
        named,
    },
    portable::analysis as keyed,
};

#[test]
fn named_trace_preserves_actual_births_and_same_execution_reference_view() -> Result<(), String> {
    let compiled = execution()?;
    for (input, count) in [
        ("1", 0),
        ("free", 0),
        ("lambda x x", 1),
        ("let outer 1 guest lambda inner inner", 2),
        ("recursive cons define a 1 cons define a 2 nil a", 2),
        ("repeat cons define x x nil x", 2),
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let trace = named::analyze(
                "birth",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let (analysis, references) = trace.references().complete().ok_or("complete")?;
            assert_eq!(trace.births().len(), count, "{input}");
            assert_eq!(analysis.facts().entities.len(), count);
            for row in trace.births() {
                let BirthLookup::Born { entity, birth } =
                    trace.entity_birth(row.entity, &mut budget()).map_err(err)?
                else {
                    return Err("built-in birth".into());
                };
                assert!(core::ptr::eq(row, birth));
                assert_eq!(entity.id, row.entity);
                assert_eq!(entity.scope, row.scope);
                assert_eq!(entity.namespace, row.namespace);
                if input.starts_with("recursive") || input.starts_with("repeat") {
                    assert_eq!(row.kind, BirthKind::Export);
                    assert!(matches!(row.phase, BirthPhase::Header { .. }));
                } else {
                    assert_eq!(row.kind, BirthKind::Bind);
                }
            }
            if input.starts_with("repeat") {
                let rows = trace.births();
                assert_eq!(rows[0].owner, rows[1].owner);
                assert_eq!(rows[0].name_target, rows[1].name_target);
                assert_eq!(rows[0].binding, rows[1].binding);
                assert_ne!(rows[0].entity, rows[1].entity);
                assert!(rows[0].execution_step < rows[1].execution_step);
                assert_ne!(rows[0].phase, rows[1].phase);
            }
            for (index, row) in references.iter().enumerate() {
                assert_eq!(
                    trace
                        .references()
                        .final_reference(index, &mut budget())
                        .map_err(err)?
                        .occurrence
                        .id,
                    row.occurrence
                );
            }
            assert!(matches!(
                trace.entity_birth(EntityId(u64::MAX), &mut budget()),
                Err(BirthAccessError::MissingEntity)
            ));
            let report = trace.references().reply().report.clone();
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                trace.entity_birth(EntityId(0), &mut cancelled),
                Err(BirthAccessError::Stopped(StopReason::Cancelled))
            ));
            let mut limits = budget().limits();
            limits.depth -= 1;
            let mut mismatch = Budget::new(limits);
            mismatch.cancel();
            assert!(matches!(
                trace.entity_birth(EntityId(0), &mut mismatch),
                Err(BirthAccessError::LimitsMismatch)
            ));
            assert_eq!(mismatch.usage().work, 0);
            assert_eq!(trace.references().reply().report, report);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn named_trace_keeps_custom_entities_untraced_and_own_final_updates() -> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let prepared = {
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            keyed::prepare(
                "birth",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?
        };
        let mut host = custom::query_host(true, false);
        let first = prepared
            .trace_named_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let mut host = custom::query_host(false, false);
        let second = prepared
            .trace_named_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(first.references().key(), second.references().key());
        for (trace, updates) in [(&first, true), (&second, false)] {
            let key = trace.references().key();
            assert!(
                matches!(trace.entity_birth(&key, EntityId(100), &mut budget()).map_err(err)?, BirthLookup::Untraced(entity) if entity.name == "x")
            );
            assert!(trace.births().iter().all(|v| v.entity != EntityId(100)));
            assert_eq!(
                trace
                    .references()
                    .final_reference(&key, 0, &mut budget())
                    .map_err(err)?
                    .occurrence
                    .resolution
                    == ReferenceResolution::Resolved(EntityId(100)),
                updates
            );
            for i in 0..4 {
                let mut stale = key;
                match i {
                    0 => stale.tree_digest.0[0] ^= 1,
                    1 => stale.profile_digest.0[0] ^= 1,
                    2 => stale.execution_digest.0[0] ^= 1,
                    _ => stale.request_digest.0[0] ^= 1,
                };
                assert!(matches!(
                    trace.entity_birth(&stale, EntityId(100), &mut budget()),
                    Err(EntityBirthAccessError::Access(
                        BindingAccessError::StaleAnalysis
                    ))
                ));
            }
            assert!(matches!(
                trace.entity_birth(&key, EntityId(u64::MAX), &mut budget()),
                Err(EntityBirthAccessError::Birth(
                    BirthAccessError::MissingEntity
                ))
            ));
        }
        let invalid = prepared
            .trace_named(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            invalid.references().trace().reply().outcome,
            BindingOutcome::Invalid {
                error: BindingError::MissingProvider,
                ..
            }
        ));
        assert!(matches!(
            invalid.entity_birth(&invalid.references().key(), EntityId(100), &mut budget()),
            Err(EntityBirthAccessError::Birth(BirthAccessError::Incomplete))
        ));
        let mut cancelled = budget();
        cancelled.cancel();
        let stopped = prepared
            .trace_named(&mut cancelled, &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            stopped.references().trace().reply().outcome,
            BindingOutcome::Stopped {
                reason: StopReason::Cancelled,
                ..
            }
        ));
        Ok(())
    })
}

#[path = "named_trace/atomic.rs"]
mod atomic;

#[cfg(target_pointer_width = "64")]
#[path = "named_trace/compatibility.rs"]
mod compatibility;

#[path = "named_trace/probe.rs"]
mod probe;

#[test]
fn ordinary_repetition_memo_reuse_and_global_stage_remain_distinct() -> Result<(), String> {
    use nepl3_engine::package::{Binding, NamespacePolicy};
    for (input, form, count, global) in [
        ("lambda x x", "lambda", 2, false),
        ("unimported define x 1", "define", 2, false),
        ("recursive cons define x 1 nil x", "define", 1, false),
        ("lambda x x", "", 1, true),
    ] {
        let mut compiled = execution()?;
        if global {
            compiled.package.namespaces[0].policy = NamespacePolicy::Global;
        } else {
            let f = compiled
                .package
                .forms
                .iter()
                .find(|f| f.spelling == form)
                .ok_or("form")?;
            let actions = match &mut compiled.package.bindings[f.binding.0 as usize] {
                Binding::Scope(v) | Binding::Group(v) => v,
                _ => return Err("group".into()),
            };
            let first = *actions.first().ok_or("action")?;
            actions.insert(1, first);
        }
        with_input(&compiled, input, |tree, profile, _, _| {
            let trace = named::analyze(
                "repeat-birth",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let (analysis, _) = trace
                .references()
                .complete()
                .ok_or_else(|| format!("{:?}", trace.references().reply().outcome))?;
            assert_eq!(trace.births().len(), count);
            if count == 2 {
                let rows = trace.births();
                assert_eq!(rows[0].owner, rows[1].owner);
                assert_eq!(rows[0].name_target, rows[1].name_target);
                assert_eq!(rows[0].binding, rows[1].binding);
                assert_ne!(rows[0].entity, rows[1].entity);
                assert!(rows[0].execution_step < rows[1].execution_step);
            }
            if global {
                let birth = &trace.births()[0];
                assert_eq!(
                    analysis.result().stages[birth.namespace_stage.0 as usize].scope,
                    birth.scope
                );
                let occurrence = analysis
                    .facts()
                    .occurrences
                    .iter()
                    .find(|v| v.role == OccurrenceRole::Definition)
                    .ok_or("definition")?;
                let point = analysis
                    .result()
                    .occurrence_stages
                    .iter()
                    .find(|v| v.occurrence == occurrence.id)
                    .ok_or("point")?;
                assert_ne!(point.namespace_stage, birth.namespace_stage);
                assert!(
                    analysis.result().stages[point.namespace_stage.0 as usize]
                        .introduced
                        .contains(&birth.entity)
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn birth_lookup_stops_after_an_initial_match_without_changing_native_report() -> Result<(), String>
{
    let compiled = execution()?;
    with_input(
        &compiled,
        "recursive cons define a 1 cons define b 2 nil a",
        |tree, profile, _, _| {
            let trace = named::analyze(
                "query-birth",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let id = trace.births().first().ok_or("birth")?.entity;
            let mut measured = budget();
            trace.entity_birth(id, &mut measured).map_err(err)?;
            let entity_count = trace
                .references()
                .complete()
                .ok_or("complete")?
                .0
                .facts()
                .entities
                .len() as u64;
            let birth_count = trace.births().len() as u64;
            assert_eq!(entity_count, 2);
            assert_eq!(birth_count, 2);
            let expected_work = 1 + entity_count + birth_count;
            let expected_nodes = entity_count + birth_count;
            assert_eq!(measured.usage().work, expected_work);
            assert_eq!(measured.usage().nodes, expected_nodes);
            let report = trace.references().reply().report.clone();
            // Target 0 matches first. Stop once in the remaining entity scan and
            // once in the remaining birth scan, using independent count-derived bounds.
            for (resource, limit, available, reason) in [
                (
                    Resource::Work,
                    measured.limits().work,
                    2,
                    StopReason::WorkLimit,
                ),
                (
                    Resource::Nodes,
                    measured.limits().nodes,
                    1,
                    StopReason::NodeLimit,
                ),
                (
                    Resource::Work,
                    measured.limits().work,
                    expected_work - 1,
                    StopReason::WorkLimit,
                ),
                (
                    Resource::Nodes,
                    measured.limits().nodes,
                    expected_nodes - 1,
                    StopReason::NodeLimit,
                ),
            ] {
                let mut stopped = budget();
                stopped.charge(resource, limit - available).map_err(err)?;
                assert!(
                    matches!(trace.entity_birth(id, &mut stopped), Err(BirthAccessError::Stopped(actual)) if actual == reason)
                );
                assert_eq!(trace.references().reply().report, report);
            }
            Ok(())
        },
    )
}

#[test]
fn birth_phases_keep_header_skips_and_body_bind_issuance_explicit() -> Result<(), String> {
    use nepl3_engine::package::{Binding, BindingId, NameSelector};
    let mut compiled = execution()?;
    let define = compiled
        .package
        .forms
        .iter()
        .find(|v| v.spelling == "define")
        .ok_or("define")?;
    let binding = define.binding;
    let bind = BindingId(compiled.package.bindings.len() as u64);
    compiled.package.bindings.push(Binding::Bind {
        namespace: "Value".into(),
        name: NameSelector::Field("name".into()),
    });
    let reference = BindingId(compiled.package.bindings.len() as u64);
    compiled.package.bindings.push(Binding::Reference {
        namespace: "Value".into(),
        name: NameSelector::Field("name".into()),
    });
    let Binding::Group(actions) = &mut compiled.package.bindings[binding.0 as usize] else {
        return Err("group".into());
    };
    actions.insert(1, bind);
    actions.insert(2, reference);
    with_input(
        &compiled,
        "recursive cons define x 1 nil x",
        |tree, profile, _, _| {
            let trace = named::analyze(
                "phases",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let (_, references) = trace.references().complete().ok_or("complete")?;
            assert_eq!(trace.births().len(), 2);
            assert_eq!(trace.births()[0].kind, BirthKind::Export);
            assert_eq!(trace.births()[1].kind, BirthKind::Bind);
            let BirthPhase::Header { group } = trace.births()[0].phase else {
                return Err("header".into());
            };
            assert_eq!(trace.births()[1].phase, BirthPhase::Body { group });
            assert_eq!(
                references.iter().filter(|v| v.binding == reference).count(),
                1
            );
            Ok(())
        },
    )
}

#[path = "named_trace/canonical.rs"]
mod canonical;

#[path = "named_trace/provider.rs"]
mod provider;
