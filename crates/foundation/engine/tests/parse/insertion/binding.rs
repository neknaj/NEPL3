use super::*;
use nepl3_engine::analysis::insertion::checked;

pub(super) fn check_binding<C: nepl3_core::value_codec::FoundationValueCodec>(
    checked: &checked::CheckedInsertion<'_, '_>,
    codec: &mut C,
    b: &mut nepl3_core::budget::Budget,
) -> TestResult
where
    C::Error: core::fmt::Debug,
{
    use nepl3_core::{budget::Budget, facts::ReferenceResolution};
    use nepl3_engine::{
        analysis::{BindingAccessError, insertion::binding as candidate_binding},
        binding::{BindingError, BindingOutcome},
    };
    let source_bytes = b.usage().source_bytes;
    let reply =
        candidate_binding::execute(checked, "declared", codec, b).map_err(|e| format!("{e:?}"))?;
    assert_eq!(reply.key(), checked.keys().1);
    assert_eq!(b.usage().source_bytes, source_bytes);
    assert_eq!(reply.reply().report.usage, b.usage());
    let source = checked.candidate().seed().request().snapshot.reference();
    match checked.candidate().execution().kind() {
        ExecutionKind::Recovered => {
            assert!(matches!(
                reply.reply().outcome,
                BindingOutcome::Invalid {
                    error: BindingError::RecoveredTree,
                    ..
                }
            ));
            assert_eq!(
                reply.for_source(&reply.key(), &source, b).err(),
                Some(BindingAccessError::Incomplete)
            );
        }
        ExecutionKind::Complete => {
            let analysis = reply
                .for_source(&reply.key(), &source, b)
                .map_err(|e| format!("{e:?}"))?;
            let unresolved = checked
                .candidate()
                .seed()
                .request()
                .snapshot
                .text()
                .ends_with(" x y");
            assert_eq!(
                analysis
                    .facts()
                    .occurrences
                    .iter()
                    .any(|o| matches!(o.resolution, ReferenceResolution::Unresolved(_))),
                unresolved
            );
        }
    }
    assert_eq!(
        reply.for_source(&checked.keys().0, &source, b).err(),
        Some(BindingAccessError::StaleAnalysis)
    );
    let wrong = candidate_binding::execute(checked, "different", codec, b);
    assert!(matches!(
        wrong,
        Err(candidate_binding::BindingError::Access(
            BindingAccessError::StaleAnalysis
        ))
    ));
    let mut limits = b.limits();
    limits.work -= 1;
    let mut changed = Budget::new(limits);
    assert!(matches!(
        candidate_binding::execute(checked, "declared", codec, &mut changed),
        Err(candidate_binding::BindingError::Access(
            BindingAccessError::LimitsMismatch
        ))
    ));
    assert_eq!(changed.usage().work, 0);
    stopped_stages(checked)?;
    Ok(())
}

fn stopped_stages(checked: &checked::CheckedInsertion<'_, '_>) -> TestResult {
    use nepl3_core::budget::{Budget, Resource, StopReason};
    use nepl3_engine::{
        analysis::insertion::binding as candidate_binding, binding::BindingOutcome,
        portable::PortableError,
    };
    let limits = checked.limits();
    let empty = SourceStore::default();
    let profile = checked.candidate().seed().profile();
    let mut prepared_cost = Budget::new(limits);
    let mut ledger = SourceAdmission::default();
    let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
        .map_err(|e| format!("{e:?}"))?;
    analysis::prepare(
        "declared",
        checked.candidate().execution().tree(),
        BindingOptions,
        limits,
        profile,
        &mut codec,
        &mut prepared_cost,
    )
    .map_err(|e| format!("{e:?}"))?;
    let preparation = prepared_cost.usage().work;
    for (available, stage) in [(0, 0), (preparation, 1), (preparation + 128, 2)] {
        let mut b = Budget::new(limits);
        b.charge(Resource::Work, limits.work - available)
            .map_err(|e| format!("{e:?}"))?;
        let mut ledger = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut ledger)
            .map_err(|e| format!("{e:?}"))?;
        let result = candidate_binding::execute(checked, "declared", &mut codec, &mut b);
        match stage {
            0 => assert!(matches!(
                result,
                Err(candidate_binding::BindingError::Preparation(
                    PortableError::Stopped(StopReason::WorkLimit)
                ))
            )),
            1 => assert!(matches!(
                result,
                Err(candidate_binding::BindingError::Access(
                    nepl3_engine::analysis::BindingAccessError::Stopped(StopReason::WorkLimit)
                ))
            )),
            _ => {
                let result = result.map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    result.reply().outcome,
                    BindingOutcome::Stopped {
                        reason: StopReason::WorkLimit,
                        ..
                    }
                ));
                assert_eq!(result.key(), checked.keys().1);
                assert_eq!(result.reply().report.usage, b.usage());
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Host {
    authorize_calls: u64,
    facts_calls: u64,
}
impl nepl3_engine::binding::BindingHost for Host {
    fn authorize(
        &mut self,
        call: &nepl3_engine::binding::BindingCall<'_>,
        b: &mut nepl3_core::budget::Budget,
    ) -> Result<Option<nepl3_core::facts::FactAuthority>, nepl3_engine::binding::BindingError> {
        use nepl3_core::facts::{FactAuthority, FactReservation, IdRange};
        b.charge(nepl3_core::budget::Resource::Work, 1)?;
        self.authorize_calls += 1;
        assert_eq!(call.provider.provider, "facts-provider");
        let empty = IdRange {
            start: 100,
            end: 100,
        };
        Ok(Some(FactAuthority {
            analysis_id: call.existing.analysis_id.clone(),
            current_scope: call.scope,
            namespaces: vec![],
            writable_scopes: vec![],
            import_scopes: vec![],
            resolution_updates: vec![],
            relation_sources: vec![],
            reservation: FactReservation {
                scopes: empty,
                entities: empty,
                occurrences: empty,
                relations: empty,
            },
        }))
    }
    fn facts(
        &mut self,
        provider: &nepl3_engine::profile::ProviderRequirement,
        request: &nepl3_engine::facts::CheckedFactsView<'_, '_>,
        _: &mut nepl3_engine::facts::FactsEmitter<'_>,
    ) -> Result<nepl3_engine::binding::CustomOutcome, nepl3_engine::binding::BindingError> {
        self.facts_calls += 1;
        assert_eq!(provider.provider, "facts-provider");
        let existing = request.request().existing;
        Ok(nepl3_engine::binding::CustomOutcome::Complete(
            nepl3_core::facts::FactDelta {
                analysis_id: existing.analysis_id.clone(),
                origin_base: existing.origins.len() as u64,
                scopes: vec![],
                entities: vec![],
                occurrences: vec![],
                relations: vec![],
                edges: vec![],
                resolutions: vec![],
                sources: vec![],
                origins: vec![],
                source_maps: vec![],
            },
        ))
    }
}

pub(super) fn check_custom<C: nepl3_core::value_codec::FoundationValueCodec>(
    checked: &checked::CheckedInsertion<'_, '_>,
    codec: &mut C,
    b: &mut nepl3_core::budget::Budget,
) -> TestResult
where
    C::Error: core::fmt::Debug,
{
    use nepl3_core::budget::{Budget, Resource, StopReason};
    use nepl3_engine::{
        analysis::{BindingAccessError, insertion::binding as candidate_binding},
        binding::{BindingError, BindingOutcome},
    };
    let absent =
        candidate_binding::execute(checked, "declared", codec, b).map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        absent.reply().outcome,
        BindingOutcome::Invalid {
            error: BindingError::MissingProvider,
            ..
        }
    ));
    let mut host = Host::default();
    let source_bytes = b.usage().source_bytes;
    let reply = candidate_binding::execute_with_host(checked, "declared", codec, &mut host, b)
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(reply.reply().outcome, BindingOutcome::Complete(_)));
    assert_eq!((host.authorize_calls, host.facts_calls), (1, 1));
    assert_eq!(b.usage().source_bytes, source_bytes);
    let mut inactive = Host::default();
    let wrong = candidate_binding::execute_with_host(checked, "different", codec, &mut inactive, b);
    assert!(matches!(
        wrong,
        Err(candidate_binding::BindingError::Access(
            BindingAccessError::StaleAnalysis
        ))
    ));
    let mut limits = checked.limits();
    limits.work -= 1;
    let mut wrong_limits = Budget::new(limits);
    assert!(matches!(
        candidate_binding::execute_with_host(
            checked,
            "declared",
            codec,
            &mut inactive,
            &mut wrong_limits
        ),
        Err(candidate_binding::BindingError::Access(
            BindingAccessError::LimitsMismatch
        ))
    ));
    let mut stopped = Budget::new(checked.limits());
    stopped
        .charge(Resource::Work, checked.limits().work)
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        candidate_binding::execute_with_host(
            checked,
            "declared",
            codec,
            &mut inactive,
            &mut stopped
        ),
        Err(candidate_binding::BindingError::Preparation(
            nepl3_engine::portable::PortableError::Stopped(StopReason::WorkLimit)
        ))
    ));
    assert_eq!((inactive.authorize_calls, inactive.facts_calls), (0, 0));
    Ok(())
}
