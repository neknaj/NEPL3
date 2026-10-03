use super::*;
use nepl3_engine::binding::trace;

#[test]
fn native_reference_trace_retains_actual_final_facts_and_issuance_order() -> Result<(), String> {
    let compiled = execution()?;
    for input in [
        "1",
        "free",
        "let x 1 x",
        "let x x x",
        "let outer 1 guest lambda inner inner",
        "recursive cons define a 1 cons define a 2 nil a",
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut ordinary_budget = budget();
            let ordinary = analyze(
                "trace",
                tree,
                profile,
                &mut ordinary_budget,
                &mut SourceAdmission::default(),
            );
            let mut observed_budget = budget();
            let observed = trace::analyze(
                "trace",
                tree,
                profile,
                None,
                &mut observed_budget,
                &mut SourceAdmission::default(),
            );
            assert_eq!(
                format!("{:?}", observed.reply().outcome),
                format!("{:?}", ordinary.outcome),
                "{input}"
            );
            let (analysis, rows) = observed.complete().ok_or("complete trace")?;
            let references: Vec<_> = analysis
                .facts()
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
                .collect();
            assert_eq!(rows.len(), references.len(), "{input}");
            assert_eq!(observed.reply().report.usage, observed_budget.usage());
            assert!(observed_budget.usage().work >= ordinary_budget.usage().work);
            assert!(core::ptr::eq(observed.tree(), tree.tree()));
            assert!(core::ptr::eq(observed.profile(), profile));
            for (index, (row, occurrence)) in rows.iter().zip(references).enumerate() {
                let final_value = observed
                    .final_reference(index, &mut observed_budget)
                    .map_err(err)?;
                assert!(core::ptr::eq(final_value.occurrence, occurrence));
                assert_eq!(row.occurrence, occurrence.id);
                assert_eq!(row.namespace, occurrence.namespace);
                let stage = analysis
                    .result()
                    .occurrence_stages
                    .iter()
                    .find(|s| s.occurrence == row.occurrence)
                    .ok_or("stage")?;
                assert_eq!(row.stage, stage.stage);
                assert_eq!(row.namespace_stage, stage.namespace_stage);
                assert_eq!(row.package.schema, compiled.package.schema);
            }
            assert!(
                rows.windows(2)
                    .all(|w| w[0].execution_step < w[1].execution_step)
            );
            if input.contains("guest") {
                assert!(rows.iter().any(|r| !r.name_target.path.is_empty()));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn native_reference_trace_partial_runs_never_offer_complete_evidence() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "let x x x", |tree, profile, _, _| {
        let mut measured = budget();
        let complete = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut measured,
            &mut SourceAdmission::default(),
        );
        assert!(complete.complete().is_some());
        let work = measured.usage().work;
        for available in [0, work / 4, work / 2, work * 3 / 4, work - 1] {
            let mut b = budget();
            b.charge(Resource::Work, b.limits().work - available)
                .map_err(err)?;
            let observed = trace::analyze(
                "trace",
                tree,
                profile,
                None,
                &mut b,
                &mut SourceAdmission::default(),
            );
            assert!(observed.complete().is_none());
            assert!(matches!(
                observed.reply().outcome,
                BindingOutcome::Stopped { .. }
            ));
            assert_eq!(observed.reply().report.usage, b.usage());
            if let BindingOutcome::Stopped { progress, .. } = &observed.reply().outcome {
                for row in observed.rows() {
                    let facts = progress.facts.as_ref().ok_or("trace without facts")?;
                    assert!(
                        facts
                            .occurrences
                            .iter()
                            .any(|o| o.id == row.occurrence && o.role == OccurrenceRole::Reference)
                    );
                    assert!(
                        progress
                            .occurrence_stages
                            .iter()
                            .any(|s| s.occurrence == row.occurrence)
                    );
                }
            }
        }
        let mut b = budget();
        b.cancel();
        let cancelled = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut b,
            &mut SourceAdmission::default(),
        );
        assert!(cancelled.complete().is_none());
        assert!(cancelled.rows().is_empty());
        Ok(())
    })
}

#[test]
fn native_reference_trace_reads_latest_custom_resolution_with_sparse_ids() -> Result<(), String> {
    let mut compiled = custom::compiled()?;
    for policy in [
        nepl3_engine::package::NamespacePolicy::Lexical,
        nepl3_engine::package::NamespacePolicy::Open,
    ] {
        compiled.package.namespaces[0].policy = policy;
        for updates in [false, true] {
            with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
                let mut b = budget();
                let mut host = custom::query_host(updates, false);
                let observed = trace::analyze(
                    "trace",
                    tree,
                    profile,
                    Some(&mut host),
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                let (analysis, rows) = observed.complete().ok_or("custom complete")?;
                // The provider issues 200; ordinary execution must retain its actual next ID.
                assert!(
                    analysis
                        .facts()
                        .occurrences
                        .iter()
                        .any(|o| o.id == OccurrenceId(200))
                );
                assert!(rows.iter().all(|r| r.occurrence != OccurrenceId(200)));
                assert!(rows.iter().any(|r| r.occurrence.0 > 200));
                let first = observed.final_reference(0, &mut b).map_err(err)?;
                assert_eq!(first.occurrence.name, "x");
                assert_eq!(
                    first.occurrence.resolution,
                    if updates {
                        ReferenceResolution::Resolved(EntityId(100))
                    } else {
                        ReferenceResolution::Unresolved("x".into())
                    }
                );
                assert_eq!(
                    first.open_input,
                    !updates && policy == nepl3_engine::package::NamespacePolicy::Open
                );
                let mut changed = b.limits();
                changed.work -= 1;
                let mut changed = Budget::new(changed);
                changed.cancel();
                assert_eq!(
                    observed.final_reference(0, &mut changed).err(),
                    Some(trace::TraceAccessError::LimitsMismatch)
                );
                assert_eq!(changed.usage().work, 0);
                assert_eq!(
                    observed.final_reference(rows.len(), &mut b).err(),
                    Some(trace::TraceAccessError::MissingRecord)
                );
                let mut cancelled = budget();
                cancelled.cancel();
                assert_eq!(
                    observed.final_reference(0, &mut cancelled).err(),
                    Some(trace::TraceAccessError::Stopped(StopReason::Cancelled))
                );
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[test]
fn native_reference_trace_keeps_repeated_actions_and_incomplete_prefixes_distinct()
-> Result<(), String> {
    let mut compiled = missing_probe::named_lambda()?;
    let lambda = compiled
        .package
        .forms
        .iter()
        .find(|f| f.spelling == "lambda")
        .ok_or("lambda")?;
    let nepl3_engine::package::Binding::Scope(actions) =
        &mut compiled.package.bindings[lambda.binding.0 as usize]
    else {
        return Err("scope".into());
    };
    let reference = *actions.last().ok_or("reference")?;
    actions.push(reference);
    actions.push(reference);
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        let observed = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let (_, rows) = observed.complete().ok_or("complete")?;
        assert_eq!(rows.len(), 3);
        for pair in rows.windows(2) {
            assert_eq!(pair[0].owner, pair[1].owner);
            assert_eq!(pair[0].name_target, pair[1].name_target);
            assert_eq!(pair[0].binding, pair[1].binding);
            assert_ne!(pair[0].occurrence, pair[1].occurrence);
            assert!(pair[0].execution_step < pair[1].execution_step);
        }
        let mut measured = budget();
        observed.final_reference(0, &mut measured).map_err(err)?;
        for (resource, used, limit, reason) in [
            (
                Resource::Work,
                measured.usage().work,
                measured.limits().work,
                StopReason::WorkLimit,
            ),
            (
                Resource::Nodes,
                measured.usage().nodes,
                measured.limits().nodes,
                StopReason::NodeLimit,
            ),
        ] {
            assert!(used > 0);
            let mut b = budget();
            b.charge(resource, limit - used + 1).map_err(err)?;
            assert_eq!(
                observed.final_reference(0, &mut b).err(),
                Some(trace::TraceAccessError::Stopped(reason))
            );
        }
        Ok(())
    })?;
    let custom = custom::compiled()?;
    with_input(&custom, "early x z custom x x", |tree, profile, _, _| {
        let observed = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            observed.reply().outcome,
            BindingOutcome::Invalid {
                error: BindingError::MissingProvider,
                ..
            }
        ));
        assert!(!observed.rows().is_empty());
        assert!(observed.complete().is_none());
        assert_eq!(
            observed.final_reference(0, &mut budget()).err(),
            Some(trace::TraceAccessError::Incomplete)
        );
        Ok(())
    })
}

#[test]
fn native_reference_trace_issuance_is_atomic_at_all_allocation_boundaries() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "free", |tree, profile, _, _| {
        let mut measured = budget();
        let reference = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut measured,
            &mut SourceAdmission::default(),
        );
        assert!(reference.complete().is_some());
        let total = measured.usage().allocation_units;
        let mut before_diagnostic = 0;
        for available in 0..=total {
            let mut b = budget();
            b.charge(
                Resource::AllocationUnits,
                b.limits().allocation_units - available,
            )
            .map_err(err)?;
            let observed = trace::analyze(
                "trace",
                tree,
                profile,
                None,
                &mut b,
                &mut SourceAdmission::default(),
            );
            if let Some(facts) = observed.reply().facts() {
                let references: Vec<_> = facts
                    .occurrences
                    .iter()
                    .filter(|o| o.role == OccurrenceRole::Reference)
                    .collect();
                // Both directions are necessary: a missing row after issuance is also a bug.
                assert_eq!(references.len(), observed.rows().len());
                for occurrence in references {
                    assert_eq!(
                        observed
                            .rows()
                            .iter()
                            .filter(|r| r.occurrence == occurrence.id)
                            .count(),
                        1
                    );
                }
            } else {
                assert!(observed.rows().is_empty());
            }
            if !observed.rows().is_empty()
                && observed.reply().report.diagnostics.is_empty()
                && observed.complete().is_none()
            {
                before_diagnostic += 1;
            }
            if available < total {
                assert!(matches!(
                    observed.reply().outcome,
                    BindingOutcome::Stopped {
                        reason: StopReason::AllocationLimit,
                        ..
                    }
                ));
            } else {
                assert!(observed.complete().is_some());
            }
        }
        assert!(
            before_diagnostic > 0,
            "exercise a stop after issuance, before diagnostic publication"
        );
        Ok(())
    })
}

struct UpdateHost {
    inner: Box<dyn BindingHost>,
    terminal: u8,
}
impl BindingHost for UpdateHost {
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
        let CustomOutcome::Complete(mut delta) = self.inner.facts(provider, request, emit)? else {
            return Err(BindingError::ProviderInvalid);
        };
        for update in &mut delta.resolutions {
            update.resolution =
                ReferenceResolution::Deferred(vec![nepl3_core::value::TypedValue::Record(
                    nepl3_core::value::Record {
                        schema: request
                            .profile()
                            .registry()
                            .selected("nepl3.engine", 1)
                            .ok_or(BindingError::Target)?
                            .clone(),
                        kind: "BindingDiagnosticArguments".into(),
                        fields: vec![NdfValue::Text("Value".into()), NdfValue::Text("x".into())],
                    },
                )]);
        }
        Ok(match self.terminal {
            0 => CustomOutcome::Complete(delta),
            1 => CustomOutcome::Invalid(Some(delta)),
            _ => CustomOutcome::Stopped {
                reason: StopReason::Cancelled,
                partial: Some(delta),
            },
        })
    }
}

#[test]
fn native_reference_trace_preserves_deferred_updates_and_rejects_partial_proof()
-> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
        for terminal in 0..3 {
            let mut host = UpdateHost {
                inner: Box::new(custom::query_host(true, false)),
                terminal,
            };
            let observed = trace::analyze(
                "trace",
                tree,
                profile,
                Some(&mut host),
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(!observed.rows().is_empty());
            let facts = observed.reply().facts().ok_or("updated facts")?;
            let original_id = observed.rows()[0].occurrence;
            let actual = facts
                .occurrences
                .iter()
                .find(|o| o.id == original_id)
                .ok_or("original reference")?;
            assert!(matches!(
                actual.resolution,
                ReferenceResolution::Deferred(_)
            ));
            if terminal == 0 {
                assert!(matches!(
                    observed
                        .final_reference(0, &mut budget())
                        .map_err(err)?
                        .occurrence
                        .resolution,
                    ReferenceResolution::Deferred(_)
                ));
            } else {
                assert!(observed.complete().is_none());
                assert_eq!(
                    observed.final_reference(0, &mut budget()).err(),
                    Some(trace::TraceAccessError::Incomplete)
                );
            }
        }
        Ok(())
    })?;
    with_input(&compiled, "custom x 1", |tree, profile, _, _| {
        let mut host = custom::query_host(false, false);
        let observed = trace::analyze(
            "trace",
            tree,
            profile,
            Some(&mut host),
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(observed.complete().is_some());
        assert!(observed.rows().is_empty());
        assert!(
            !observed
                .reply()
                .facts()
                .ok_or("custom facts")?
                .occurrences
                .is_empty()
        );
        Ok(())
    })
}

#[test]
fn native_reference_trace_retains_global_namespace_stage_and_depth_limits() -> Result<(), String> {
    let mut compiled = execution()?;
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Global;
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        let observed = trace::analyze(
            "trace",
            tree,
            profile,
            None,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let (_, rows) = observed.complete().ok_or("global complete")?;
        assert_eq!(rows.len(), 1);
        assert_ne!(rows[0].stage, rows[0].namespace_stage);
        let mut b = budget();
        let limit = b.limits().depth;
        let result = b.with_depth_at_least(limit, |b| observed.final_reference(0, b));
        assert_eq!(
            result.err(),
            Some(trace::TraceAccessError::Stopped(StopReason::DepthLimit))
        );
        assert!(matches!(
            observed
                .final_reference(0, &mut budget())
                .map_err(err)?
                .occurrence
                .resolution,
            ReferenceResolution::Resolved(_)
        ));
        Ok(())
    })
}

#[test]
fn native_reference_trace_canonical_coordinates_survive_a_new_receiver() -> Result<(), String> {
    let compiled = execution()?;
    with_input(
        &compiled,
        "lambda x apply guest x x",
        |tree, profile, _, _| {
            let original = trace::analyze(
                "trace",
                tree,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let encoded = nepl3_engine::portable::tree::to_value(
                tree.tree(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let received = nepl3_engine::portable::tree::from_value(
                &value,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let mut received = received;
            // Reverse the root arena without changing semantic child order.
            let count = received.bundle.nodes.len() as u64;
            let remap = |node: &mut nepl3_core::syntax::NodeRef| {
                node.0 = count - 1 - node.0;
            };
            received.bundle.nodes.reverse();
            remap(&mut received.bundle.root);
            for node in &mut received.bundle.nodes {
                for field in &mut node.fields {
                    match field {
                        nepl3_core::syntax::FieldValue::Child(child) => remap(child),
                        nepl3_core::syntax::FieldValue::Children(children) => {
                            for child in children {
                                remap(child);
                            }
                        }
                        _ => {}
                    }
                }
            }
            for context in &mut received.contexts {
                if let Some(step) = context.path.first_mut() {
                    remap(&mut step.node);
                } else {
                    for selected in &mut context.nodes {
                        remap(&mut selected.node);
                    }
                }
            }
            assert!(received.recovery.is_empty());
            let received = received
                .validate(profile, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            assert_ne!(tree.tree().bundle.root, received.tree().bundle.root);
            let decoded = trace::analyze(
                "trace",
                &received,
                profile,
                None,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(!core::ptr::eq(original.tree(), decoded.tree()));
            assert_eq!(original.rows().len(), decoded.rows().len());
            for (left, right) in original.rows().iter().zip(decoded.rows()) {
                assert_eq!(left.owner, right.owner);
                assert_eq!(left.name_target, right.name_target);
                assert_eq!(left.package, right.package);
                assert_eq!(left.execution_digest, right.execution_digest);
                assert_eq!(left.binding, right.binding);
                assert_eq!(left.execution_step, right.execution_step);
            }
            // Equal canonical coordinates authenticate only their own retained execution.
            assert!(original.complete().is_some());
            assert!(decoded.complete().is_some());
            Ok(())
        },
    )
}
