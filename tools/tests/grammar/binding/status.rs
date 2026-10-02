use super::*;
use nepl3_engine::{analysis::BindingOptions, portable::analysis as keyed};
use nepl3_lsp::binding::{BindingState, ReferenceCounts, StatusError, inspect};

#[test]
fn lsp_binding_status_keeps_analysis_wide_resolution_classes() -> Result<(), String> {
    let compiled = execution()?;
    for (input, resolved, unresolved, ambiguous) in [
        ("1", 0, 0, 0),
        ("lambda x x", 1, 0, 0),
        ("apply x y", 0, 2, 0),
        ("lambda x apply x y", 1, 1, 0),
        ("recursive cons define a 1 cons define a 2 nil a", 0, 0, 1),
        ("lambda x apply guest x x", 1, 1, 0),
    ] {
        with_input(&compiled, input, |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let prepared = keyed::prepare(
                "status",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let reply = prepared.execute(b, codec.source_admission()).map_err(err)?;
            let mut projection = budget();
            let result = inspect(&reply, &prepared.key(), &mut projection).map_err(err)?;
            assert_eq!(result.key, prepared.key());
            assert!(core::ptr::eq(result.report, &reply.reply().report));
            let BindingState::Complete {
                analysis,
                references,
            } = result.state
            else {
                return Err("complete status".into());
            };
            assert_eq!(
                references,
                ReferenceCounts {
                    resolved,
                    unresolved,
                    ambiguous,
                    deferred: 0
                }
            );
            let BindingOutcome::Complete(original) = &reply.reply().outcome else {
                return Err("native complete".into());
            };
            assert!(core::ptr::eq(analysis, original));
            assert_eq!(
                projection.usage().work,
                128 + original.facts().occurrences.len() as u64
            );
            assert_eq!(projection.usage().allocation_units, 0);
            let mut exact = budget().limits();
            exact.work = projection.usage().work;
            assert!(inspect(&reply, &prepared.key(), &mut Budget::new(exact)).is_ok());

            // Every digest participates in the host's current-key gate.
            for index in 0..4 {
                let mut key = prepared.key();
                let wrong = Digest::of(b"stale");
                match index {
                    0 => key.tree_digest = wrong,
                    1 => key.profile_digest = wrong,
                    2 => key.execution_digest = wrong,
                    _ => key.request_digest = wrong,
                }
                assert!(matches!(
                    inspect(&reply, &key, &mut budget()),
                    Err(StatusError::StaleAnalysis)
                ));
            }
            // This stops the projection at the key gate or during its scan,
            // without changing the completed native analysis or its report.
            let mut limits = budget().limits();
            limits.work = projection.usage().work - 1;
            assert!(matches!(
                inspect(&reply, &prepared.key(), &mut Budget::new(limits)),
                Err(StatusError::Stopped(StopReason::WorkLimit))
            ));
            assert!(matches!(reply.reply().outcome, BindingOutcome::Complete(_)));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn lsp_binding_status_preserves_invalid_and_stopped_progress() -> Result<(), String> {
    let compiled = global::compiled()?;
    with_input(&compiled, "lambda x lambda x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
        let prepared = keyed::prepare(
            "invalid-status",
            tree.tree(),
            BindingOptions,
            b.limits(),
            profile,
            &mut codec,
            b,
        )
        .map_err(err)?;
        let reply = prepared.execute(b, codec.source_admission()).map_err(err)?;
        let result = inspect(&reply, &prepared.key(), &mut budget()).map_err(err)?;
        let BindingState::Invalid { error, progress } = result.state else {
            return Err("invalid status".into());
        };
        assert_eq!(*error, BindingError::DuplicateGlobal);
        let BindingOutcome::Invalid {
            error: original_error,
            progress: original_progress,
        } = &reply.reply().outcome
        else {
            return Err("native invalid".into());
        };
        assert!(core::ptr::eq(error, original_error));
        assert!(core::ptr::eq(progress, original_progress));
        assert!(core::ptr::eq(result.report, &reply.reply().report));
        let mut stopped = Budget::new(b.limits());
        stopped.cancel();
        let stopped_reply = prepared
            .execute(&mut stopped, codec.source_admission())
            .map_err(err)?;
        let result = inspect(&stopped_reply, &prepared.key(), &mut budget()).map_err(err)?;
        let BindingState::Stopped { reason, progress } = result.state else {
            return Err("stopped status".into());
        };
        assert_eq!(reason, StopReason::Cancelled);
        let BindingOutcome::Stopped {
            progress: original_progress,
            ..
        } = &stopped_reply.reply().outcome
        else {
            return Err("native stopped".into());
        };
        assert!(core::ptr::eq(progress, original_progress));
        assert!(core::ptr::eq(result.report, &stopped_reply.reply().report));
        let mut projection = budget();
        projection.cancel();
        assert!(matches!(
            inspect(&reply, &prepared.key(), &mut projection),
            Err(StatusError::Stopped(StopReason::Cancelled))
        ));
        Ok(())
    })
}

struct ProjectionHost {
    inner: Box<dyn BindingHost>,
    deferred: bool,
}
impl BindingHost for ProjectionHost {
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
        request: &nepl3_engine::facts::CheckedFactsView<'_, '_>,
        emit: &mut nepl3_engine::facts::FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let result = self.inner.facts(provider, request, emit)?;
        let CustomOutcome::Complete(mut delta) = result else {
            return Err(BindingError::ProviderInvalid);
        };
        if self.deferred {
            let occurrence = delta.occurrences.first_mut().ok_or(BindingError::Target)?;
            occurrence.role = OccurrenceRole::Reference;
            occurrence.resolution =
                ReferenceResolution::Deferred(vec![nepl3_core::value::TypedValue::Record(
                    nepl3_core::value::Record {
                        schema: request
                            .profile()
                            .registry()
                            .selected("nepl3.engine", 1)
                            .ok_or(BindingError::Target)?
                            .clone(),
                        kind: "BindingDiagnosticArguments".into(),
                        fields: vec![
                            NdfValue::Text("Value".into()),
                            NdfValue::Text(occurrence.name.clone()),
                        ],
                    },
                )]);
        }
        Ok(CustomOutcome::Complete(delta))
    }
}

#[test]
fn lsp_binding_status_handles_native_import_deferred_and_generated_sources() -> Result<(), String> {
    let compiled = custom::compiled()?;
    for mode in 0..3 {
        with_input(&compiled, "lambda x custom y x", |tree, profile, b, a| {
            let empty = SourceStore::default();
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            let prepared = keyed::prepare(
                "custom-status",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?;
            let mut host = ProjectionHost {
                inner: if mode == 2 {
                    Box::new(custom::query_map_host(false, false))
                } else {
                    Box::new(custom::query_host_import())
                },
                deferred: mode != 0,
            };
            let reply = prepared
                .execute_with_host(&mut host, b, codec.source_admission())
                .map_err(err)?;
            let result = inspect(&reply, &prepared.key(), &mut budget()).map_err(err)?;
            let BindingState::Complete {
                analysis,
                references,
            } = result.state
            else {
                return Err(format!("custom complete: {:?}", reply.reply().outcome));
            };
            assert_eq!(
                references,
                ReferenceCounts {
                    // The body x resolves to the enclosing lambda; the Custom
                    // occurrence is either an excluded Import or a Deferred reference.
                    resolved: 1,
                    deferred: u64::from(mode != 0),
                    ..ReferenceCounts::default()
                }
            );
            if mode == 0 {
                assert!(
                    analysis
                        .facts()
                        .occurrences
                        .iter()
                        .any(|o| o.role == OccurrenceRole::Import)
                );
            }
            if mode == 2 {
                assert!(analysis.result().sources.len() >= 2);
            }
            assert!(!result.report.diagnostics.is_empty());
            assert!(!result.report.events.is_empty());
            assert!(core::ptr::eq(result.report, &reply.reply().report));
            Ok(())
        })?;
    }
    Ok(())
}
