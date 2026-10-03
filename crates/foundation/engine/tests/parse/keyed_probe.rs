use super::*;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::SourceError,
};
use nepl3_engine::{
    analysis::{
        BindingAccessError, BindingOptions,
        probe::{ProbeAccessError, ProbePosition},
    },
    binding::probe::ProbeOutcome,
    package::{Binding, NameSelector, ReadSpecId},
    portable::analysis,
};

#[derive(Clone, Copy)]
enum Outcome {
    Hit,
    NoHit,
    Blocked,
    Stopped,
}

#[test]
fn keyed_probe_checks_identity_limits_and_positions_for_every_native_outcome() -> TestResult {
    for (input, expected) in [
        ("let あ", Outcome::Hit),
        ("let あ あ", Outcome::NoHit),
        ("let あ @", Outcome::Blocked),
        ("let あ", Outcome::Stopped),
    ] {
        retained::with_edited_package(
            input,
            |p| {
                p.forms[0].fields[1].read = ReadSpecId(0);
                p.bindings[1] = Binding::Reference {
                    namespace: "Value".into(),
                    name: NameSelector::Field("body".into()),
                };
            },
            |profile, environments, sources, source, entry| {
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Unit,
                }];
                let mut b = budget();
                let mut a = SourceAdmission::default();
                let mut session = RetainedParseSession::new(
                    "keyed".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: input.len() as u64,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let RetainedParseExecution::Continue(parsed) =
                    session.read(&mut b, &mut a).map_err(|e| format!("{e:?}"))?
                else {
                    return Err("parse".into());
                };
                let empty = SourceStore::default();
                let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a)
                    .map_err(|e| format!("{e:?}"))?;
                let limits = b.limits();
                let prepared = analysis::prepare(
                    "keyed",
                    parsed.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let other = analysis::prepare(
                    "other-id",
                    parsed.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                assert_ne!(prepared.key(), other.key());
                let mut different_limits = limits;
                different_limits.work -= 1;
                let changed_limits = analysis::prepare(
                    "keyed",
                    parsed.execution().tree(),
                    BindingOptions,
                    different_limits,
                    profile,
                    &mut codec,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                assert_ne!(prepared.key(), changed_limits.key());
                let mut mismatched = limits;
                mismatched.work = 0;
                mismatched.depth = 0;
                let mut wrong = Budget::new(mismatched);
                assert!(matches!(
                    prepared.probe_missing_reference(&mut wrong, &mut SourceAdmission::default()),
                    Err(BindingAccessError::LimitsMismatch)
                ));
                assert_eq!(wrong.usage().work, 0);
                let mut execution = Budget::new(limits);
                let mut execution_admission = SourceAdmission::default();
                if matches!(expected, Outcome::Stopped) {
                    execution.cancel();
                }
                let bound = prepared
                    .probe_missing_reference(&mut execution, &mut execution_admission)
                    .map_err(|e| format!("{e:?}"))?;
                assert_eq!(bound.key(), prepared.key());
                assert_eq!(bound.reply().report.usage, execution.usage());
                let original_report = bound.reply().report.clone();
                let key = bound.key();
                let source_ref = source.reference();
                let end = input.len() as u64;
                let bytes = execution.usage().source_bytes;
                let continued = bound.for_position(
                    &key,
                    &source_ref,
                    end,
                    &mut execution,
                    &mut execution_admission,
                );
                if matches!(expected, Outcome::Stopped) {
                    assert!(matches!(
                        continued,
                        Err(ProbeAccessError::Access(BindingAccessError::Stopped(
                            StopReason::Cancelled
                        )))
                    ));
                    assert!(matches!(
                        bound.reply().outcome,
                        ProbeOutcome::Stopped(StopReason::Cancelled)
                    ));
                    assert!(bound.reply().sources().is_empty());
                } else {
                    continued.map_err(|e| format!("{e:?}"))?;
                    assert_eq!(execution.usage().source_bytes, bytes);
                }
                let mut fresh = Budget::new(limits);
                let mut fresh_admission = SourceAdmission::default();
                let position = bound
                    .for_position(&key, &source_ref, end, &mut fresh, &mut fresh_admission)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    (expected, position),
                    (Outcome::Hit, ProbePosition::HitAtPosition(_))
                        | (Outcome::NoHit, ProbePosition::NoHit)
                        | (Outcome::Blocked, ProbePosition::Blocked(_))
                        | (
                            Outcome::Stopped,
                            ProbePosition::Stopped(StopReason::Cancelled)
                        )
                ));
                assert_eq!(fresh.usage().source_bytes, input.len() as u64);
                use nepl3_engine::analysis::{
                    expected::ExpectedReadRequest,
                    probe::read::{ReadOutcome, correlate},
                };
                let read_request = ExpectedReadRequest {
                    key,
                    source: source_ref.clone(),
                    offset: end,
                };
                let mut read_budget = Budget::new(limits);
                let read = correlate(
                    &bound,
                    &prepared,
                    &read_request,
                    &mut read_budget,
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    (expected, read.outcome()),
                    (Outcome::Hit, ReadOutcome::Hit(_))
                        | (Outcome::NoHit, ReadOutcome::NoHit)
                        | (Outcome::Blocked, ReadOutcome::Blocked(_))
                        | (
                            Outcome::Stopped,
                            ReadOutcome::Stopped(StopReason::Cancelled)
                        )
                ));
                if matches!(expected, Outcome::Hit) {
                    let measured = read_budget.usage();
                    let mut depth_exact = Budget::new(limits);
                    depth_exact
                        .with_depth_at_least(limits.depth - measured.depth, |b| {
                            correlate(
                                &bound,
                                &prepared,
                                &read_request,
                                b,
                                &mut SourceAdmission::default(),
                            )
                        })
                        .map_err(|e| format!("{e:?}"))?;
                    let mut depth_short = Budget::new(limits);
                    let depth_error = depth_short
                        .with_depth_at_least(limits.depth - measured.depth + 1, |b| {
                            correlate(
                                &bound,
                                &prepared,
                                &read_request,
                                b,
                                &mut SourceAdmission::default(),
                            )
                        })
                        .err()
                        .ok_or("read depth stop")?;
                    assert_eq!(depth_error.stop_reason(), Some(StopReason::DepthLimit));

                    for (resource, total, used, reason) in [
                        (
                            Resource::Work,
                            limits.work,
                            measured.work,
                            StopReason::WorkLimit,
                        ),
                        (
                            Resource::Nodes,
                            limits.nodes,
                            measured.nodes,
                            StopReason::NodeLimit,
                        ),
                        (
                            Resource::AllocationUnits,
                            limits.allocation_units,
                            measured.allocation_units,
                            StopReason::AllocationLimit,
                        ),
                        (
                            Resource::SourceBytes,
                            limits.source_bytes,
                            measured.source_bytes,
                            StopReason::SourceLimit,
                        ),
                    ] {
                        let mut limited = Budget::new(limits);
                        limited
                            .charge(resource, total - used + 1)
                            .map_err(|e| format!("{e:?}"))?;
                        let error = correlate(
                            &bound,
                            &prepared,
                            &read_request,
                            &mut limited,
                            &mut SourceAdmission::default(),
                        )
                        .err()
                        .ok_or("expected local read stop")?;
                        assert_eq!(error.stop_reason(), Some(reason));
                        assert_eq!(bound.reply().report, original_report);
                    }
                    let other_read = ExpectedReadRequest {
                        offset: 0,
                        ..read_request.clone()
                    };
                    let other = correlate(
                        &bound,
                        &prepared,
                        &other_read,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    assert!(matches!(other.outcome(), ReadOutcome::OtherHit(_)));
                }
                let mut cancelled_read = Budget::new(limits);
                cancelled_read.cancel();
                assert_eq!(
                    correlate(
                        &bound,
                        &prepared,
                        &read_request,
                        &mut cancelled_read,
                        &mut SourceAdmission::default()
                    )
                    .err()
                    .ok_or("read cancellation")?
                    .stop_reason(),
                    Some(StopReason::Cancelled)
                );

                use nepl3_engine::analysis::probe::candidates::{
                    ProbeCandidateOutcome, ProbeCandidateRequest, names,
                };
                let request = ProbeCandidateRequest {
                    key,
                    source: &source_ref,
                    offset: end,
                    prefix: "absent",
                };
                let mut candidate_budget = Budget::new(limits);
                let candidates = names(
                    &bound,
                    &request,
                    &mut candidate_budget,
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                assert!(
                    matches!((expected, candidates.outcome()),
                        (Outcome::Hit, ProbeCandidateOutcome::Hit { candidates, .. }) if candidates.is_empty()
                    ) || matches!(
                        (expected, candidates.outcome()),
                        (Outcome::NoHit, ProbeCandidateOutcome::NoHit)
                            | (Outcome::Blocked, ProbeCandidateOutcome::Blocked(_))
                            | (
                                Outcome::Stopped,
                                ProbeCandidateOutcome::Stopped(StopReason::Cancelled)
                            )
                    )
                );
                if !matches!(expected, Outcome::Hit) {
                    assert_eq!(candidate_budget.usage(), fresh.usage());
                }
                use nepl3_engine::analysis::probe::candidates::ProbeCandidateError;
                let mut stale_key = key;
                stale_key.tree_digest.0[0] ^= 1;
                let stale_request = ProbeCandidateRequest {
                    key: stale_key,
                    ..request
                };
                assert!(matches!(
                    names(
                        &bound,
                        &stale_request,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeCandidateError::Access(ProbeAccessError::Access(
                        BindingAccessError::StaleAnalysis
                    )))
                ));
                let mut stale_source = source_ref.clone();
                stale_source.revision += 1;
                let stale_request = ProbeCandidateRequest {
                    source: &stale_source,
                    ..request
                };
                assert!(matches!(
                    names(
                        &bound,
                        &stale_request,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeCandidateError::Access(ProbeAccessError::Access(
                        BindingAccessError::MissingSource
                    )))
                ));
                for offset in [5, end + 1] {
                    let invalid = ProbeCandidateRequest { offset, ..request };
                    assert!(matches!(
                        names(
                            &bound,
                            &invalid,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(ProbeCandidateError::Access(ProbeAccessError::Source(
                            SourceError::ScalarBoundary | SourceError::Bounds
                        )))
                    ));
                }
                let mut mismatch = Budget::new(mismatched);
                assert!(matches!(
                    names(
                        &bound,
                        &request,
                        &mut mismatch,
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeCandidateError::Access(ProbeAccessError::Access(
                        BindingAccessError::LimitsMismatch
                    )))
                ));
                assert_eq!(mismatch.usage().work, 0);
                let mut stopped_query = Budget::new(limits);
                stopped_query.cancel();
                assert!(matches!(
                    names(
                        &bound,
                        &request,
                        &mut stopped_query,
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeCandidateError::Access(ProbeAccessError::Access(
                        BindingAccessError::Stopped(StopReason::Cancelled)
                    )))
                ));
                if matches!(expected, Outcome::Stopped) {
                    assert!(matches!(
                        names(&bound, &request, &mut execution, &mut execution_admission),
                        Err(ProbeCandidateError::Access(ProbeAccessError::Access(
                            BindingAccessError::Stopped(StopReason::Cancelled)
                        )))
                    ));
                }
                let mut other_budget = Budget::new(limits);
                let mut gate_budget = Budget::new(limits);
                let other_request = ProbeCandidateRequest {
                    offset: 0,
                    ..request
                };
                let other_candidates = names(
                    &bound,
                    &other_request,
                    &mut other_budget,
                    &mut SourceAdmission::default(),
                )
                .map_err(|e| format!("{e:?}"))?;
                bound
                    .for_position(
                        &key,
                        &source_ref,
                        0,
                        &mut gate_budget,
                        &mut SourceAdmission::default(),
                    )
                    .map_err(|e| format!("{e:?}"))?;
                assert_eq!(other_budget.usage(), gate_budget.usage());
                if matches!(expected, Outcome::Hit) {
                    assert!(matches!(
                        other_candidates.outcome(),
                        ProbeCandidateOutcome::OtherHit(_)
                    ));
                }

                if matches!(expected, Outcome::Hit) {
                    assert!(matches!(
                        bound.for_position(&key, &source_ref, 0, &mut fresh, &mut fresh_admission),
                        Ok(ProbePosition::OtherHit(_))
                    ));
                }
                for at in [5, end + 1] {
                    let invalid_read = ExpectedReadRequest {
                        offset: at,
                        ..read_request.clone()
                    };
                    assert!(matches!(
                        correlate(
                            &bound,
                            &prepared,
                            &invalid_read,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                            ProbeAccessError::Source(
                                SourceError::ScalarBoundary | SourceError::Bounds
                            )
                        ))
                    ));

                    let error = bound
                        .for_position(
                            &key,
                            &source_ref,
                            at,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default(),
                        )
                        .err();
                    assert_eq!(
                        error,
                        Some(ProbeAccessError::Source(if at == 5 {
                            SourceError::ScalarBoundary
                        } else {
                            SourceError::Bounds
                        }))
                    );
                }
                let mut wrong = Budget::new(mismatched);
                assert!(matches!(
                    bound.for_position(
                        &key,
                        &source_ref,
                        end,
                        &mut wrong,
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeAccessError::Access(BindingAccessError::LimitsMismatch))
                ));
                assert_eq!(wrong.usage().work, 0);
                let mut wrong_read = Budget::new(mismatched);
                assert!(matches!(
                    correlate(
                        &bound,
                        &prepared,
                        &read_request,
                        &mut wrong_read,
                        &mut SourceAdmission::default()
                    ),
                    Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                        ProbeAccessError::Access(BindingAccessError::LimitsMismatch)
                    ))
                ));
                assert_eq!(wrong_read.usage().work, 0);
                assert!(matches!(
                    correlate(
                        &bound,
                        &other,
                        &read_request,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default()
                    ),
                    Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                        ProbeAccessError::Access(BindingAccessError::StaleAnalysis)
                    ))
                ));
                let mut prepared_limits_mismatch = Budget::new(limits);
                assert!(matches!(
                    correlate(
                        &bound,
                        &changed_limits,
                        &read_request,
                        &mut prepared_limits_mismatch,
                        &mut SourceAdmission::default()
                    ),
                    Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                        ProbeAccessError::Access(BindingAccessError::LimitsMismatch)
                    ))
                ));
                assert_eq!(prepared_limits_mismatch.usage().work, 0);

                let mut keys = vec![other.key(), changed_limits.key()];
                for index in 0..4 {
                    let mut changed = key;
                    match index {
                        0 => changed.tree_digest.0[0] ^= 1,
                        1 => changed.profile_digest.0[0] ^= 1,
                        2 => changed.execution_digest.0[0] ^= 1,
                        _ => changed.request_digest.0[0] ^= 1,
                    }
                    keys.push(changed);
                }
                // Local gate exhaustion precedes stale-key classification when
                // limits match. LimitsMismatch alone is checked before the budget.
                for cancelled in [false, true] {
                    let mut query = Budget::new(limits);
                    if cancelled {
                        query.cancel();
                    } else {
                        query
                            .charge(Resource::Work, limits.work)
                            .map_err(|e| format!("{e:?}"))?;
                    }
                    let mut stale = key;
                    stale.request_digest.0[0] ^= 1;
                    assert_eq!(
                        bound
                            .for_position(
                                &stale,
                                &source_ref,
                                end,
                                &mut query,
                                &mut SourceAdmission::default(),
                            )
                            .err(),
                        Some(ProbeAccessError::Access(BindingAccessError::Stopped(
                            if cancelled {
                                StopReason::Cancelled
                            } else {
                                StopReason::WorkLimit
                            }
                        )))
                    );
                    assert_eq!(bound.reply().report, original_report);
                }
                for changed in keys {
                    let stale_read = ExpectedReadRequest {
                        key: changed,
                        ..read_request.clone()
                    };
                    assert!(matches!(
                        correlate(
                            &bound,
                            &prepared,
                            &stale_read,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                            ProbeAccessError::Access(BindingAccessError::StaleAnalysis)
                        ))
                    ));

                    assert!(matches!(
                        bound.for_position(
                            &changed,
                            &source_ref,
                            end,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(ProbeAccessError::Access(BindingAccessError::StaleAnalysis))
                    ));
                }
                for index in 0..3 {
                    let mut changed = source_ref.clone();
                    match index {
                        0 => changed.source_id.0.push_str("-wrong"),
                        1 => changed.revision += 1,
                        _ => changed.digest.0[0] ^= 1,
                    }
                    let stale_read = ExpectedReadRequest {
                        source: changed.clone(),
                        ..read_request.clone()
                    };
                    assert!(matches!(
                        correlate(
                            &bound,
                            &prepared,
                            &stale_read,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(nepl3_engine::analysis::probe::read::ReadError::Access(
                            ProbeAccessError::Access(BindingAccessError::MissingSource)
                        ))
                    ));
                    assert!(matches!(
                        bound.for_position(
                            &key,
                            &changed,
                            end,
                            &mut Budget::new(limits),
                            &mut SourceAdmission::default()
                        ),
                        Err(ProbeAccessError::Access(BindingAccessError::MissingSource))
                    ));
                }
                for locator_conflict in [false, true] {
                    let mut query = Budget::new(limits);
                    let mut admission = SourceAdmission::default();
                    let conflict = SourceSnapshot::new(
                        source.identity().source.clone(),
                        source.identity().revision,
                        if locator_conflict {
                            "memory:other".into()
                        } else {
                            source.uri().into()
                        },
                        if locator_conflict {
                            input.as_bytes().to_vec()
                        } else {
                            b"other bytes".to_vec()
                        },
                        &mut query,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                    admission
                        .admit_existing(&conflict, &mut query)
                        .map_err(|e| format!("{e:?}"))?;
                    assert!(matches!(
                        bound.for_position(&key, &source_ref, end, &mut query, &mut admission),
                        Err(ProbeAccessError::Source(SourceError::IdentityConflict))
                    ));
                }
                let mut exhausted = Budget::new(limits);
                exhausted
                    .charge(Resource::SourceBytes, limits.source_bytes)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    bound.for_position(
                        &key,
                        &source_ref,
                        end,
                        &mut exhausted,
                        &mut SourceAdmission::default()
                    ),
                    Err(ProbeAccessError::Access(BindingAccessError::Stopped(
                        StopReason::SourceLimit
                    )))
                ));
                assert_eq!(bound.reply().report, original_report);
                Ok(())
            },
        )?;
    }
    Ok(())
}
