use super::*;
use nepl3_core::budget::{Budget, StopReason};
use nepl3_engine::analysis::insertion::draft;

#[derive(Clone, Copy, Debug)]
enum Gate {
    Normal,
    Cancelled,
    StaleKey,
    WrongPrepared,
    LimitsMismatch,
    WrongSource,
    NewerRevision,
    ExtraSource,
    PartialInput,
    Work,
    Allocation,
    Depth,
    Source,
}

fn run_draft(
    old_text: &str,
    at: u64,
    text: &str,
    gate: Gate,
    expected: Option<&str>,
) -> TestResult {
    super::super::retained::with_context(
        old_text,
        |profile, environments, sources, source, entry| {
            let mut newer_sources = SourceStore::default();
            let sources = if matches!(gate, Gate::NewerRevision | Gate::ExtraSource) {
                newer_sources
                    .insert(source.clone())
                    .map_err(|e| format!("{e:?}"))?;
                let newer = SourceSnapshot::new(
                    SourceId(
                        if matches!(gate, Gate::ExtraSource) {
                            "extra"
                        } else {
                            "input"
                        }
                        .into(),
                    ),
                    1,
                    if matches!(gate, Gate::ExtraSource) {
                        "memory:extra"
                    } else {
                        "memory:input"
                    }
                    .into(),
                    b"existing next revision".to_vec(),
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                newer_sources.insert(newer).map_err(|e| format!("{e:?}"))?;
                &newer_sources
            } else {
                sources
            };
            let source_count = sources.snapshots().len();
            let (start, limit) = if matches!(gate, Gate::PartialInput) {
                (6, 11)
            } else {
                (0, old_text.len() as u64)
            };
            let states = [LanguageReaderState {
                alias: "Host".into(),
                state: NdfValue::Unit,
            }];
            let mut session = RetainedParseSession::new(
                "draft-old".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: source,
                    start,
                    limit,
                    final_input: true,
                    entry,
                    states: &states,
                },
                sources,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Continue(parsed) = session
                .read(&mut budget(), &mut SourceAdmission::default())
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("retained initial parse".into());
            };
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            let mut limits = budget().limits();
            match gate {
                Gate::Work => limits.work = 0,
                Gate::Allocation => limits.allocation_units = 0,
                Gate::Depth => limits.depth = 0,
                Gate::Source => limits.source_bytes = 0,
                _ => {}
            }
            let alternate_tree = parsed.execution().tree().clone();
            let prepared = analysis::prepare(
                "draft-old",
                if matches!(gate, Gate::WrongPrepared) {
                    &alternate_tree
                } else {
                    parsed.execution().tree()
                },
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
            let mut query = ExpectedReadRequest {
                key: prepared.key(),
                source: source.reference(),
                offset: at,
            };
            if matches!(gate, Gate::StaleKey) {
                query.key.request_digest = Digest::of(b"stale");
            }
            if matches!(gate, Gate::WrongSource) {
                query.source.revision += 1;
            }
            let mut operation_limits = limits;
            if matches!(gate, Gate::LimitsMismatch) {
                operation_limits.work -= 1;
            }
            let mut operation = Budget::new(operation_limits);
            if matches!(gate, Gate::Cancelled) {
                operation.cancel();
            }
            let mut ledger = SourceAdmission::default();
            let proposal = draft::prepare(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &query,
                text,
                &mut operation,
                &mut ledger,
            );
            assert_eq!(sources.snapshots().len(), source_count);
            assert_eq!(source.text(), old_text);
            let Some(expected) = expected else {
                use nepl3_core::source::SourceError;
                use nepl3_engine::analysis::BindingAccessError;
                let error = match gate {
                    Gate::Cancelled => InsertionError::Stopped(StopReason::Cancelled),
                    Gate::StaleKey => InsertionError::Access(BindingAccessError::StaleAnalysis),
                    Gate::WrongPrepared => InsertionError::ProofMismatch,
                    Gate::LimitsMismatch => {
                        InsertionError::Access(BindingAccessError::LimitsMismatch)
                    }
                    Gate::WrongSource => InsertionError::EditMismatch,
                    Gate::NewerRevision => InsertionError::Source(SourceError::SnapshotMismatch),
                    Gate::Work => InsertionError::Stopped(StopReason::WorkLimit),
                    Gate::Allocation => InsertionError::Stopped(StopReason::AllocationLimit),
                    Gate::Depth => InsertionError::Stopped(StopReason::DepthLimit),
                    Gate::Source => InsertionError::Stopped(StopReason::SourceLimit),
                    _ if text.is_empty() || at > limit => InsertionError::EditMismatch,
                    _ if !old_text.is_char_boundary(at as usize) => {
                        InsertionError::Source(SourceError::ScalarBoundary)
                    }
                    _ => InsertionError::NoMissing,
                };
                assert_eq!(proposal.err(), Some(error), "{gate:?}");
                return Ok(());
            };
            let proposal = proposal.map_err(|e| format!("{e:?}"))?;
            assert_eq!(proposal.original_key(), prepared.key());
            assert_eq!(proposal.snapshot().text(), expected);
            assert_eq!(
                proposal.snapshot().identity().revision,
                source.identity().revision + 1
            );
            assert_eq!(proposal.snapshot().uri(), source.uri());
            assert_eq!(proposal.limit(), limit + text.len() as u64);
            assert_eq!(proposal.edit().replacement, text);
            assert_eq!(proposal.edit().span.start(), at);
            assert_eq!(proposal.edit().span.end(), at);
            assert_eq!(proposal.sources().snapshots().len(), source_count + 1);
            for original in sources.snapshots() {
                let preserved = proposal
                    .sources()
                    .get_ref(original.identity())
                    .ok_or("preserved source")?;
                assert_eq!(preserved.text(), original.text());
                assert_eq!(preserved.uri(), original.uri());
            }
            assert_eq!(
                operation.usage().source_bytes,
                sources
                    .snapshots()
                    .iter()
                    .map(|s| s.text().len() as u64)
                    .sum::<u64>()
                    + proposal.snapshot().text().len() as u64
            );
            assert_eq!(proposal.report().usage, operation.usage());
            // Stop just before the successful operation's final charge. The
            // generated source is already admitted, but no draft may escape.
            for allocation in [false, true] {
                let mut strict = limits;
                if allocation {
                    strict.allocation_units = operation.usage().allocation_units - 1;
                } else {
                    strict.work = operation.usage().work - 1;
                }
                let prepared = analysis::prepare(
                    "draft-late",
                    parsed.execution().tree(),
                    BindingOptions,
                    strict,
                    profile,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?;
                let query = ExpectedReadRequest {
                    key: prepared.key(),
                    source: source.reference(),
                    offset: at,
                };
                let mut stopped = Budget::new(strict);
                let outcome = draft::prepare(
                    InsertionInput {
                        parsed: &parsed,
                        prepared: &prepared,
                    },
                    &query,
                    text,
                    &mut stopped,
                    &mut SourceAdmission::default(),
                );
                let reason = if allocation {
                    StopReason::AllocationLimit
                } else {
                    StopReason::WorkLimit
                };
                assert_eq!(outcome.err(), Some(InsertionError::Stopped(reason)));
                assert_eq!(stopped.usage().source_bytes, operation.usage().source_bytes);
                assert_eq!(sources.snapshots().len(), source_count);
                assert_eq!(source.text(), old_text);
            }

            let alternative = format!("{text}\n");
            let previous_work = operation.usage().work;
            let conflicting = draft::prepare(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &query,
                &alternative,
                &mut operation,
                &mut ledger,
            );
            assert_eq!(
                conflicting.err(),
                Some(InsertionError::Source(
                    nepl3_core::source::SourceError::IdentityConflict
                ))
            );
            let separate = draft::prepare(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &query,
                &alternative,
                &mut operation,
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("separate candidate: {e:?}"))?;
            assert_eq!(
                separate.snapshot().identity().revision,
                proposal.snapshot().identity().revision
            );
            assert_ne!(
                separate.snapshot().identity().digest,
                proposal.snapshot().identity().digest
            );
            assert!(operation.usage().work > previous_work);
            assert_eq!(sources.snapshots().len(), source_count);
            let mut candidate = RetainedParseSession::new(
                "draft-new".into(),
                profile,
                environments,
                ParseRequest {
                    snapshot: proposal.snapshot(),
                    start,
                    limit: proposal.limit(),
                    final_input: true,
                    entry,
                    states: &states,
                },
                proposal.sources(),
                &mut operation,
            )
            .map_err(|e| format!("{e:?}"))?;
            let RetainedParseExecution::Continue(candidate) = candidate
                .read(&mut operation, &mut ledger)
                .map_err(|e| format!("{e:?}"))?
            else {
                return Err("retained candidate parse".into());
            };
            let candidate_prepared = {
                let mut candidate_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                        .map_err(|e| format!("{e:?}"))?;
                analysis::prepare(
                    "draft-new",
                    candidate.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut candidate_codec,
                    &mut operation,
                )
                .map_err(|e| format!("{e:?}"))?
            };
            let observed = observe(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                InsertionInput {
                    parsed: &candidate,
                    prepared: &candidate_prepared,
                },
                &query,
                proposal.edit(),
                &mut operation,
                &mut ledger,
            );
            if text == "@" {
                assert!(observed.is_err());
            } else {
                observed.map_err(|e| format!("{e:?}"))?;
            }
            use nepl3_engine::analysis::insertion::checked::{self, CheckError};
            let equivalent = draft::prepare(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &query,
                text,
                &mut operation,
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
            assert_eq!(
                equivalent.snapshot().identity(),
                proposal.snapshot().identity()
            );
            let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
                .map_err(|e| format!("{e:?}"))?;
            let wrong_store = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &equivalent,
                &query,
                "checked",
                &mut codec,
                &mut operation,
            );
            assert!(matches!(wrong_store, Err(CheckError::DraftMismatch)));
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &query,
                "checked",
                &mut codec,
                &mut operation,
            );
            if text == "@" {
                assert!(matches!(
                    result,
                    Err(CheckError::Insertion(InsertionError::RecoveryTarget))
                ));
            } else {
                let checked = result.map_err(|e| format!("{e:?}"))?;
                assert!(core::ptr::eq(checked.original(), &parsed));
                assert!(core::ptr::eq(checked.candidate(), &candidate));
                assert!(core::ptr::eq(checked.edit(), proposal.edit()));
                assert_eq!(checked.keys().0, prepared.key());
                assert_eq!(checked.report().usage, operation.usage());
                // Measure this verification operation separately, then retain
                // the same effective Limits while leaving exactly one unit too
                // little for the end of preparation or the end of observation.
                use nepl3_core::budget::Resource;
                let mut measured = Budget::new(limits);
                let mut measured_ledger = SourceAdmission::default();
                let mut measured_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut measured_ledger)
                        .map_err(|e| format!("{e:?}"))?;
                checked::check(
                    InsertionInput {
                        parsed: &parsed,
                        prepared: &prepared,
                    },
                    &candidate,
                    &proposal,
                    &query,
                    "checked",
                    &mut measured_codec,
                    &mut measured,
                )
                .map_err(|e| format!("{e:?}"))?;
                let whole_work = measured.usage().work;
                let mut preparation = Budget::new(limits);
                preparation
                    .charge(Resource::Work, 1)
                    .map_err(|e| format!("{e:?}"))?;
                candidate
                    .seed()
                    .request()
                    .snapshot
                    .identity()
                    .compare_with_budget(proposal.snapshot().identity(), &mut preparation)
                    .map_err(|e| format!("{e:?}"))?;
                preparation
                    .charge(
                        Resource::Work,
                        (candidate.seed().request().snapshot.uri().len()
                            + proposal.snapshot().uri().len()) as u64
                            + 1,
                    )
                    .map_err(|e| format!("{e:?}"))?;
                let mut preparation_ledger = SourceAdmission::default();
                let mut preparation_codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut preparation_ledger)
                        .map_err(|e| format!("{e:?}"))?;
                analysis::prepare(
                    "checked",
                    candidate.execution().tree(),
                    BindingOptions,
                    limits,
                    profile,
                    &mut preparation_codec,
                    &mut preparation,
                )
                .map_err(|e| format!("{e:?}"))?;
                for (available, in_preparation) in [
                    (preparation.usage().work - 1, true),
                    (whole_work - 1, false),
                ] {
                    let mut stopped = Budget::new(limits);
                    stopped
                        .charge(Resource::Work, limits.work - available)
                        .map_err(|e| format!("{e:?}"))?;
                    let mut stopped_ledger = SourceAdmission::default();
                    let mut stopped_codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut stopped_ledger)
                            .map_err(|e| format!("{e:?}"))?;
                    let result = checked::check(
                        InsertionInput {
                            parsed: &parsed,
                            prepared: &prepared,
                        },
                        &candidate,
                        &proposal,
                        &query,
                        "checked",
                        &mut stopped_codec,
                        &mut stopped,
                    );
                    if in_preparation {
                        assert!(matches!(
                            result,
                            Err(CheckError::Preparation(
                                nepl3_engine::portable::PortableError::Stopped(
                                    StopReason::WorkLimit
                                )
                            ))
                        ));
                    } else {
                        assert!(matches!(
                            result,
                            Err(CheckError::Insertion(InsertionError::Stopped(
                                StopReason::WorkLimit
                            )))
                        ));
                    }
                    assert_eq!(sources.snapshots().len(), source_count);
                    assert_eq!(source.text(), old_text);
                }
            }
            let mut stopped = Budget::new(limits);
            stopped.cancel();
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &query,
                "checked",
                &mut codec,
                &mut stopped,
            );
            assert!(matches!(
                result,
                Err(CheckError::Stopped(StopReason::Cancelled))
            ));
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &query,
                "",
                &mut codec,
                &mut operation,
            );
            assert!(matches!(
                result,
                Err(CheckError::Preparation(
                    nepl3_engine::portable::PortableError::Shape
                ))
            ));
            let mut stale = query.clone();
            stale.key.request_digest = Digest::of(b"wrong checked key");
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &stale,
                "checked",
                &mut codec,
                &mut operation,
            );
            assert!(matches!(
                result,
                Err(CheckError::Insertion(InsertionError::Access(
                    nepl3_engine::analysis::BindingAccessError::StaleAnalysis
                )))
            ));
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &candidate_prepared,
                },
                &candidate,
                &proposal,
                &query,
                "checked",
                &mut codec,
                &mut operation,
            );
            assert!(matches!(
                result,
                Err(CheckError::Insertion(InsertionError::ProofMismatch))
            ));
            let mut wrong_limits = limits;
            wrong_limits.work -= 1;
            let mut wrong_budget = Budget::new(wrong_limits);
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &query,
                "checked",
                &mut codec,
                &mut wrong_budget,
            );
            assert!(matches!(
                result,
                Err(CheckError::Insertion(InsertionError::Access(
                    nepl3_engine::analysis::BindingAccessError::LimitsMismatch
                )))
            ));
            assert_eq!(wrong_budget.usage().work, 0);
            assert_eq!(wrong_budget.usage().source_bytes, 0);
            let mut wrong_offset = query.clone();
            wrong_offset.offset += 1;
            let result = checked::check(
                InsertionInput {
                    parsed: &parsed,
                    prepared: &prepared,
                },
                &candidate,
                &proposal,
                &wrong_offset,
                "checked",
                &mut codec,
                &mut operation,
            );
            assert!(matches!(
                result,
                Err(CheckError::Insertion(InsertionError::EditMismatch))
            ));
            Ok(())
        },
    )
}

#[test]
fn explicit_private_proposals_are_reparsed_before_acceptance() -> TestResult {
    run_draft("", 0, "let x x", Gate::Normal, Some("let x x"))?;
    run_draft("let x", 5, " x", Gate::Normal, Some("let x x"))?;
    run_draft("", 0, "let x x", Gate::ExtraSource, Some("let x x"))?;
    run_draft(
        "prefixlet xsuffix",
        11,
        " x",
        Gate::PartialInput,
        Some("prefixlet x xsuffix"),
    )?;
    // A draft can contain rejected syntax; only the later observation checks it.
    run_draft("", 0, "@", Gate::Normal, Some("@"))
}

#[test]
fn failed_proposals_leave_original_sources_unchanged() -> TestResult {
    run_draft("", 0, "let", Gate::Cancelled, None)?;
    run_draft("", 0, "", Gate::Normal, None)?;
    run_draft("x", 0, "let", Gate::Normal, None)?;
    run_draft("", 1, "let", Gate::Normal, None)?;
    run_draft("é", 1, "let", Gate::Normal, None)
}

#[test]
fn private_draft_identity_and_finite_budget_gates_are_atomic() -> TestResult {
    for gate in [
        Gate::StaleKey,
        Gate::WrongPrepared,
        Gate::LimitsMismatch,
        Gate::WrongSource,
        Gate::NewerRevision,
        Gate::Work,
        Gate::Allocation,
        Gate::Depth,
        Gate::Source,
    ] {
        run_draft("", 0, "let", gate, None)?;
    }
    Ok(())
}
