use super::*;
use nepl3_engine::{
    analysis::{BindingAccessError, BindingOptions, completion::*},
    portable::analysis as keyed,
};

#[test]
fn captured_scope_name_candidates_preserve_visibility_and_shadowing() -> Result<(), String> {
    let lexical = execution()?;
    let global = global::compiled()?;
    for (input, prefix, global_mode, expected) in [
        ("lambda a probe", "", false, vec!["a"]),
        ("lambda a lambda b probe", "", false, vec!["b", "a"]),
        ("lambda a lambda a probe", "", false, vec!["a"]),
        ("let a probe a", "", false, vec![]),
        (
            "sequence cons define a 1 cons define b a nil probe",
            "",
            false,
            vec!["b", "a"],
        ),
        (
            "recursive cons define a 1 cons define a 2 nil probe",
            "",
            false,
            vec!["a"],
        ),
        ("lambda あ probe", "あ", false, vec!["あ"]),
        ("lambda あ probe", "い", false, vec![]),
        ("lambda a guest probe", "", false, vec![]),
        ("guest lambda a probe", "", false, vec!["a"]),
        ("apply lambda a a probe", "", false, vec![]),
        ("apply lambda a a probe", "", true, vec!["a"]),
        ("apply probe lambda a a", "", true, vec![]),
    ] {
        let compiled = if global_mode { &global } else { &lexical };
        with_input(compiled, input, |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let prepared = keyed::prepare(
                "scope-names",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let bound = prepared.execute(b, codec.source_admission()).map_err(err)?;
            let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
                return Err("complete binding".into());
            };
            let occurrence = analysis
                .facts()
                .occurrences
                .iter()
                .find(|o| o.role == OccurrenceRole::Reference && o.name == "probe")
                .ok_or("probe reference")?;
            let request = ScopeCandidateRequest {
                key: prepared.key(),
                occurrence: occurrence.id,
                prefix,
            };
            let mut operation = budget();
            let result = names(&bound, &request, &mut operation).map_err(err)?;
            assert_eq!(result.key, prepared.key());
            assert_eq!(result.occurrence, occurrence.id);
            assert_eq!(result.namespace, occurrence.namespace);
            assert_eq!(
                result
                    .candidates
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                expected,
                "{input}"
            );
            assert_eq!(result.report.usage, operation.usage());
            for candidate in &result.candidates {
                let ids = match &candidate.resolution {
                    ReferenceResolution::Resolved(id) => vec![*id],
                    ReferenceResolution::Ambiguous(ids) => ids.clone(),
                    _ => return Err("visible declarations must resolve".into()),
                };
                let selections = ids
                    .iter()
                    .map(|id| {
                        analysis
                            .facts()
                            .entities
                            .iter()
                            .find(|e| e.id == *id)
                            .and_then(|e| e.selection.as_ref())
                            .map(|s| s.start())
                            .ok_or("selection")
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if input == "lambda a lambda a probe" {
                    assert_eq!(selections, vec![16]); // inner declaration shadows outer byte 7
                }
                if input == "recursive cons define a 1 cons define a 2 nil probe" {
                    let mut positions = selections;
                    positions.sort();
                    assert_eq!(positions, vec![22, 38]); // both same-scope declarations remain ambiguous
                }
            }
            let report = &bound.reply().report;
            assert_eq!(operation.usage().source_bytes, 0);
            // One unit below successful Work cost must not publish a partial candidate list.
            let mut limits = budget().limits();
            limits.work = operation.usage().work - 1;
            assert!(matches!(
                names(&bound, &request, &mut Budget::new(limits)),
                Err(CandidateError::Stopped(StopReason::WorkLimit))
            ));
            assert!(core::ptr::eq(report, &bound.reply().report));
            for index in 0..4 {
                let mut wrong = ScopeCandidateRequest {
                    key: request.key,
                    occurrence: request.occurrence,
                    prefix,
                };
                let digest = Digest::of(b"stale");
                match index {
                    0 => wrong.key.tree_digest = digest,
                    1 => wrong.key.profile_digest = digest,
                    2 => wrong.key.execution_digest = digest,
                    _ => wrong.key.request_digest = digest,
                }
                assert!(matches!(
                    names(&bound, &wrong, &mut budget()),
                    Err(CandidateError::Access(BindingAccessError::StaleAnalysis))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn scope_candidates_reject_missing_nonreference_partial_and_stopped_inputs() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda a probe", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let prepared = keyed::prepare(
            "scope-gates",
            tree.tree(),
            BindingOptions,
            b.limits(),
            profile,
            &mut codec,
            b,
        )
        .map_err(err)?;
        let bound = prepared.execute(b, codec.source_admission()).map_err(err)?;
        let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
            return Err("complete".into());
        };
        let definition = analysis
            .facts()
            .occurrences
            .iter()
            .find(|o| o.role == OccurrenceRole::Definition)
            .ok_or("definition")?;
        let reference = analysis
            .facts()
            .occurrences
            .iter()
            .find(|o| o.role == OccurrenceRole::Reference)
            .ok_or("reference")?;
        for (id, expected) in [
            (definition.id, CandidateError::NotReference),
            (OccurrenceId(u64::MAX), CandidateError::NoOccurrence),
        ] {
            let request = ScopeCandidateRequest {
                key: prepared.key(),
                occurrence: id,
                prefix: "",
            };
            assert_eq!(names(&bound, &request, &mut budget()).err(), Some(expected));
        }
        let request = ScopeCandidateRequest {
            key: prepared.key(),
            occurrence: reference.id,
            prefix: "",
        };
        for resource in 0..3 {
            let mut limits = budget().limits();
            let reason = match resource {
                0 => {
                    limits.work = 0;
                    StopReason::WorkLimit
                }
                1 => {
                    limits.nodes = 0;
                    StopReason::NodeLimit
                }
                _ => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
            };
            assert_eq!(
                names(&bound, &request, &mut Budget::new(limits)).err(),
                Some(CandidateError::Stopped(reason))
            );
        }
        let mut cancelled = Budget::new(b.limits());
        cancelled.cancel();
        let stopped = prepared
            .execute(&mut cancelled, codec.source_admission())
            .map_err(err)?;
        assert_eq!(
            names(&stopped, &request, &mut budget()).err(),
            Some(CandidateError::Access(BindingAccessError::Incomplete))
        );
        let mut query = budget();
        query.cancel();
        assert_eq!(
            names(&bound, &request, &mut query).err(),
            Some(CandidateError::Stopped(StopReason::Cancelled))
        );
        Ok(())
    })
}

#[test]
fn scope_candidates_use_custom_declarations_and_reject_invalid_binding() -> Result<(), String> {
    let compiled = custom::compiled()?;
    for source_less in [false, true] {
        with_input(
            &compiled,
            "lambda a custom b probe",
            |tree, profile, b, a| {
                let empty = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
                let prepared = keyed::prepare(
                    "custom-candidates",
                    tree.tree(),
                    BindingOptions,
                    b.limits(),
                    profile,
                    &mut codec,
                    b,
                )
                .map_err(err)?;
                let mut host: Box<dyn BindingHost> = if source_less {
                    Box::new(custom::query_host(false, true))
                } else {
                    Box::new(custom::query_host_import())
                };
                let bound = prepared
                    .execute_with_host(host.as_mut(), b, codec.source_admission())
                    .map_err(err)?;
                let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
                    return Err("custom complete".into());
                };
                let reference = analysis
                    .facts()
                    .occurrences
                    .iter()
                    .find(|o| o.role == OccurrenceRole::Reference && o.name == "probe")
                    .ok_or("probe")?;
                let request = ScopeCandidateRequest {
                    key: prepared.key(),
                    occurrence: reference.id,
                    prefix: "",
                };
                let result = names(&bound, &request, &mut budget()).map_err(err)?;
                assert_eq!(
                    result
                        .candidates
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>(),
                    vec!["b", "a"]
                );
                assert_eq!(
                    result.candidates[0].resolution,
                    ReferenceResolution::Resolved(EntityId(100))
                );
                if source_less {
                    assert!(
                        analysis
                            .facts()
                            .entities
                            .iter()
                            .find(|e| e.id == EntityId(100))
                            .ok_or("custom entity")?
                            .definition
                            .is_none()
                    );
                } else {
                    let import = analysis
                        .facts()
                        .occurrences
                        .iter()
                        .find(|o| o.role == OccurrenceRole::Import)
                        .ok_or("import")?;
                    let request = ScopeCandidateRequest {
                        key: prepared.key(),
                        occurrence: import.id,
                        prefix: "",
                    };
                    assert_eq!(
                        names(&bound, &request, &mut budget()).err(),
                        Some(CandidateError::NotReference)
                    );
                }
                Ok(())
            },
        )?;
    }
    let compiled = global::compiled()?;
    with_input(
        &compiled,
        "lambda a lambda a probe",
        |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let prepared = keyed::prepare(
                "invalid-candidates",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let bound = prepared.execute(b, codec.source_admission()).map_err(err)?;
            assert!(matches!(
                bound.reply().outcome,
                BindingOutcome::Invalid { .. }
            ));
            let request = ScopeCandidateRequest {
                key: prepared.key(),
                occurrence: OccurrenceId(0),
                prefix: "",
            };
            assert_eq!(
                names(&bound, &request, &mut budget()).err(),
                Some(CandidateError::Access(BindingAccessError::Incomplete))
            );
            Ok(())
        },
    )
}

#[test]
fn custom_final_resolution_does_not_rewrite_captured_candidate_visibility() -> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let prepared = keyed::prepare(
            "candidate-history",
            tree.tree(),
            BindingOptions,
            b.limits(),
            profile,
            &mut codec,
            b,
        )
        .map_err(err)?;
        let mut host = custom::query_host(true, false);
        let bound = prepared
            .execute_with_host(&mut host, b, codec.source_admission())
            .map_err(err)?;
        let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
            return Err("complete history".into());
        };
        let first = analysis
            .facts()
            .occurrences
            .iter()
            .find(|o| o.role == OccurrenceRole::Reference && o.span.start() == 6)
            .ok_or("early x")?;
        assert_eq!(
            first.resolution,
            ReferenceResolution::Resolved(EntityId(100))
        );
        let request = ScopeCandidateRequest {
            key: prepared.key(),
            occurrence: first.id,
            prefix: "",
        };
        let result = names(&bound, &request, &mut budget()).map_err(err)?;
        assert!(result.candidates.is_empty()); // both z and Custom x were introduced later
        assert_eq!(
            first.resolution,
            ReferenceResolution::Resolved(EntityId(100))
        );
        Ok(())
    })
}
