use super::*;
use nepl3_engine::facts::*;
#[path = "custom/recursive.rs"]
mod recursive;
pub(super) fn implementation_digest() -> Digest {
    Digest::of(
        &[
            include_bytes!("custom.rs").as_slice(),
            include_bytes!("custom/recursive.rs").as_slice(),
        ]
        .concat(),
    )
}
pub(super) fn compiled() -> Result<CompiledLanguage, String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../../conformance/fixtures/grammar/binding/custom.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding-custom",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
pub(super) fn query_host(updates: bool, source_less: bool) -> impl BindingHost {
    Host {
        registered: true,
        updates,
        source_less,
        ..Default::default()
    }
}
pub(super) fn query_host_import() -> impl BindingHost {
    Host {
        registered: true,
        import_role: true,
        ..Default::default()
    }
}
#[test]
fn custom_foreign_call_uses_its_own_namespace_and_root() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "lambda x guest custom y x",
        |tree, profile, _, _| {
            let mut host = Host {
                registered: true,
                ..Default::default()
            };
            let reply = analyze_with_host(
                "foreign-custom",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let BindingOutcome::Complete(analysis) = &reply.outcome else {
                return Err(format!("{reply:?}"));
            };
            assert_eq!(host.calls, 1);
            let facts = analysis.facts();
            let x = facts
                .entities
                .iter()
                .find(|v| v.name == "x")
                .ok_or("host x")?;
            let y = facts
                .entities
                .iter()
                .find(|v| v.id == EntityId(100))
                .ok_or("guest y")?;
            assert_ne!(x.namespace, y.namespace);
            let reference = facts
                .occurrences
                .iter()
                .find(|v| v.role == OccurrenceRole::Reference)
                .ok_or("guest ref")?;
            assert_eq!(reference.namespace, y.namespace);
            assert_eq!(
                reference.resolution,
                ReferenceResolution::Unresolved("x".into())
            );
            Ok(())
        },
    )
}
#[test]
fn custom_maximum_ids_stay_typed() -> Result<(), String> {
    let compiled = compiled()?;
    for input in ["custom x early x y x", "custom x early x y early x z x"] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut host = Host {
                registered: true,
                entity_start: Some(u64::MAX - 1),
                ..Default::default()
            };
            let reply = analyze_with_host(
                "id-limit",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            match &reply.outcome {
                BindingOutcome::Complete(analysis) if input == "custom x early x y x" => {
                    assert_eq!(
                        analysis
                            .facts()
                            .entities
                            .iter()
                            .map(|v| v.id.0)
                            .collect::<Vec<_>>(),
                        [u64::MAX - 1, u64::MAX]
                    );
                }
                BindingOutcome::Invalid {
                    error: BindingError::Fact(FactError::Reservation),
                    ..
                } if input.contains(" y early") => {
                    assert_eq!(reply.facts().ok_or("facts")?.entities.len(), 2);
                    assert_eq!(reply.report.events.len(), 1);
                }
                _ => return Err(format!("{input}: {reply:?}")),
            }
            reply
                .facts()
                .ok_or("facts")?
                .validate(
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
            Ok(())
        })?;
    }
    Ok(())
}
#[derive(Default)]
struct Host {
    calls: usize,
    registered: bool,
    cancel: bool,
    invalid_id: bool,
    updates: bool,
    artifacts: bool,
    source_less: bool,
    duplicate: bool,
    invalid_reply: bool,
    stopped_reply: Option<StopReason>,
    raw_artifacts: bool,
    entity_start: Option<u64>,
    import_role: bool,
}
impl BindingHost for Host {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        b.charge(Resource::Work, 1)?;
        if !self.registered {
            return Ok(None);
        }
        if call.provider.operation.name != "bindingFacts"
            || call.provider.revision != 1
            || call.provider.implementation_digest != implementation_digest()
        {
            return Err(BindingError::MissingProvider);
        }
        let mut root = call.scope;
        loop {
            b.charge(Resource::Work, call.existing.scopes.len() as u64)?;
            let scope = call
                .existing
                .scopes
                .iter()
                .find(|v| v.id == root)
                .ok_or(BindingError::Target)?;
            if let Some(parent) = scope.parent {
                root = parent;
            } else {
                break;
            }
        }
        let namespace = NamespaceRef(
            call.existing
                .namespaces
                .iter()
                .position(|v| v.root == root)
                .ok_or(BindingError::MissingNamespace)? as u64,
        );
        Ok(Some(FactAuthority {
            analysis_id: call.existing.analysis_id.clone(),
            current_scope: call.scope,
            namespaces: vec![namespace],
            writable_scopes: vec![call.scope],
            import_scopes: vec![],
            resolution_updates: if self.updates {
                call.existing
                    .occurrences
                    .iter()
                    .filter(|v| v.role == OccurrenceRole::Reference && v.name == "x")
                    .map(|v| v.id)
                    .collect()
            } else {
                vec![]
            },
            relation_sources: vec![],
            reservation: FactReservation {
                scopes: IdRange {
                    start: 100,
                    end: 110,
                },
                entities: IdRange {
                    start: self.entity_start.unwrap_or(100),
                    end: self.entity_start.unwrap_or(100).saturating_add(10),
                },
                occurrences: IdRange {
                    start: 200,
                    end: 210,
                },
                relations: IdRange {
                    start: 100,
                    end: 110,
                },
            },
        }))
    }
    fn facts(
        &mut self,
        _provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        self.calls += 1;
        let request = checked.request();
        let mut bundle = &request.tree.bundle;
        for step in request.path {
            // The explicit fixture Guest form has the single field `value`.
            if step.field != "value" {
                return Err(BindingError::Target);
            }
            let node = bundle
                .node(step.node)
                .map_err(nepl3_engine::tree::TreeError::from)?;
            let Some(nepl3_core::syntax::FieldValue::Foreign(child)) = node.fields.first() else {
                return Err(BindingError::Target);
            };
            bundle = &child.bundle;
        }
        let node = bundle
            .node(request.node)
            .map_err(nepl3_engine::tree::TreeError::from)?;
        let nepl3_core::syntax::FieldValue::Child(name) = node.fields[0] else {
            return Err(BindingError::Target);
        };
        let name = bundle
            .node(name)
            .map_err(nepl3_engine::tree::TreeError::from)?;
        let token = &bundle.tokens[name.token.ok_or(BindingError::Name)?.0 as usize];
        let NdfValue::Text(text) = &token.payload else {
            return Err(BindingError::Name);
        };
        let span = token.head.clone();
        let primary = if self.artifacts {
            let source = SourceSnapshot::new(
                SourceId("custom-generated".into()),
                0,
                "memory:custom-generated".into(),
                text.as_bytes().to_vec(),
                emit.budget(),
            )?;
            let generated = source.span_with_budget(0, text.len() as u64, emit.budget())?;
            emit.source(source)?;
            emit.source_map(nepl3_core::origin::Mapping {
                source: span.clone(),
                target: generated.clone(),
                kind: nepl3_core::origin::MappingKind::Exact,
            })?;
            generated
        } else {
            span.clone()
        };
        let schema = checked
            .profile()
            .registry()
            .selected("nepl3.engine", 1)
            .ok_or(BindingError::Target)?
            .clone();
        let args = nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
            schema: schema.clone(),
            kind: "BindingDiagnosticArguments".into(),
            fields: vec![NdfValue::Text("Value".into()), NdfValue::Text(text.clone())],
        });
        emit.diagnostic(nepl3_core::diagnostic::Diagnostic {
            schema: schema.clone(),
            code: "CustomNote".into(),
            severity: nepl3_core::diagnostic::Severity::Information,
            stage: "binding".into(),
            arguments: args.clone(),
            primary: Some(primary.clone()),
            related: vec![],
            fixes: vec![],
        })?;
        emit.event(nepl3_core::diagnostic::Event {
            schema,
            kind: "CustomVisit".into(),
            operation_path: vec![],
            span: Some(primary),
            payload: args,
        })?;
        let start = self.entity_start.unwrap_or(100);
        let entity = EntityId(if self.invalid_id {
            start.saturating_sub(1)
        } else {
            start
        });
        let scope = request.authority.current_scope;
        let mut delta = FactDelta {
            analysis_id: request.existing.analysis_id.clone(),
            origin_base: request.existing.origins.len() as u64,
            scopes: vec![],
            entities: vec![Entity {
                id: entity,
                scope,
                namespace: request.authority.namespaces[0],
                name: text.clone(),
                definition: (!self.source_less).then(|| span.clone()),
                selection: (!self.source_less).then(|| span.clone()),
                origin: None,
            }],
            occurrences: vec![Occurrence {
                id: OccurrenceId(200),
                scope,
                namespace: request.authority.namespaces[0],
                name: text.clone(),
                role: if self.import_role {
                    OccurrenceRole::Import
                } else {
                    OccurrenceRole::Definition
                },
                span,
                origin: None,
                resolution: ReferenceResolution::Resolved(entity),
            }],
            relations: vec![],
            edges: vec![],
            resolutions: if self.updates {
                request
                    .authority
                    .resolution_updates
                    .iter()
                    .map(|id| ResolutionUpdate {
                        occurrence: *id,
                        resolution: ReferenceResolution::Resolved(entity),
                    })
                    .collect()
            } else {
                vec![]
            },
            sources: vec![],
            origins: vec![],
            source_maps: vec![],
        };
        if self.raw_artifacts {
            let source = SourceSnapshot::new(
                SourceId("custom-delta-source".into()),
                0,
                "memory:custom-delta-source".into(),
                text.as_bytes().to_vec(),
                emit.budget(),
            )?;
            let generated = source.span_with_budget(0, text.len() as u64, emit.budget())?;
            delta.entities[0].definition = Some(generated.clone());
            delta.entities[0].selection = Some(generated.clone());
            delta.occurrences[0].span = generated.clone();
            delta.sources.push(source);
            delta.source_maps.push(nepl3_core::origin::Mapping {
                source: token.head.clone(),
                target: generated,
                kind: nepl3_core::origin::MappingKind::Exact,
            });
        }
        if self.duplicate {
            let mut other = delta.entities[0].clone();
            other.id = EntityId(101);
            delta.entities.push(other);
            let mut other = delta.occurrences[0].clone();
            other.id = OccurrenceId(201);
            other.resolution = ReferenceResolution::Resolved(EntityId(101));
            delta.occurrences.push(other);
        }
        if self.cancel {
            emit.budget().cancel();
        }
        Ok(if self.invalid_reply {
            CustomOutcome::Invalid(Some(delta))
        } else if let Some(reason) = self.stopped_reply {
            CustomOutcome::Stopped {
                reason,
                partial: Some(delta),
            }
        } else {
            CustomOutcome::Complete(delta)
        })
    }
}
#[test]
fn custom_global_diagnostic_retains_the_validated_delta_source() -> Result<(), String> {
    let mut compiled = compiled()?;
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Global;
    with_input(&compiled, "early x x custom x x", |tree, profile, _, _| {
        let mut host = Host {
            registered: true,
            raw_artifacts: true,
            ..Default::default()
        };
        let reply = analyze_with_host(
            "delta-source",
            tree,
            profile,
            &mut host,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(matches!(
            reply.outcome,
            BindingOutcome::Invalid {
                error: BindingError::DuplicateGlobal,
                ..
            }
        ));
        let diagnostic = reply.report.diagnostics.last().ok_or("diagnostic")?;
        assert_eq!(
            diagnostic
                .primary
                .as_ref()
                .ok_or("primary")?
                .snapshot_ref()
                .source
                .0,
            "custom-delta-source"
        );
        let mut sources = SourceStore::default();
        for source in reply.sources() {
            sources.insert(source.clone()).map_err(err)?;
        }
        reply
            .report
            .validate(&sources, &[], profile.registry(), &mut budget())
            .map_err(err)?;
        assert_eq!(reply.source_maps().len(), 1);
        nepl3_core::origin::SourceMap::validate_mappings(
            reply.source_maps(),
            &sources,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(reply.facts().ok_or("facts")?.entities.len(), 1);
        Ok(())
    })
}
#[test]
fn custom_partial_reply_and_budget_stop_keep_checked_prefixes() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "early missing z custom x x",
        |tree, profile, _, _| {
            for stopped in [None, Some(StopReason::DepthLimit)] {
                let mut host = Host {
                    registered: true,
                    invalid_reply: stopped.is_none(),
                    stopped_reply: stopped,
                    ..Default::default()
                };
                let reply = analyze_with_host(
                    "partial-custom",
                    tree,
                    profile,
                    &mut host,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                match &reply.outcome {
                    BindingOutcome::Invalid {
                        error: BindingError::ProviderInvalid,
                        ..
                    } if stopped.is_none() => {}
                    BindingOutcome::Stopped {
                        reason: StopReason::DepthLimit,
                        ..
                    } if stopped.is_some() => {}
                    _ => return Err(format!("{reply:?}")),
                }
                assert_eq!(
                    reply
                        .facts()
                        .ok_or("facts")?
                        .entities
                        .iter()
                        .map(|v| v.id.0)
                        .collect::<Vec<_>>(),
                    [0, 100]
                );
                assert_eq!(reply.report.events.len(), 1);
            }
            let mut full = budget();
            let mut host = Host {
                registered: true,
                artifacts: true,
                ..Default::default()
            };
            let reply = analyze_with_host(
                "sweep-custom",
                tree,
                profile,
                &mut host,
                &mut full,
                &mut SourceAdmission::default(),
            );
            assert!(matches!(reply.outcome, BindingOutcome::Complete(_)));
            let mut retained = 0;
            for work in [false, true] {
                let total = if work {
                    full.usage().work
                } else {
                    full.usage().allocation_units
                };
                for cap in (0..=total)
                    .step_by((total / 60).max(1) as usize)
                    .chain([total])
                {
                    let mut limits = budget().limits();
                    if work {
                        limits.work = cap;
                    } else {
                        limits.allocation_units = cap;
                    }
                    let mut b = Budget::new(limits);
                    let mut host = Host {
                        registered: true,
                        artifacts: true,
                        ..Default::default()
                    };
                    let reply = analyze_with_host(
                        "sweep-custom",
                        tree,
                        profile,
                        &mut host,
                        &mut b,
                        &mut SourceAdmission::default(),
                    );
                    match &reply.outcome {
                        BindingOutcome::Stopped { reason, .. } => {
                            assert_eq!(
                                *reason,
                                if work {
                                    StopReason::WorkLimit
                                } else {
                                    StopReason::AllocationLimit
                                }
                            );
                            if reply.report.events.len() == 1 {
                                retained += 1;
                            }
                        }
                        BindingOutcome::Complete(_) => {}
                        _ => return Err(format!("work={work} cap={cap}: {reply:?}")),
                    }
                    if let Some(facts) = reply.facts() {
                        facts
                            .validate(
                                profile.registry(),
                                &mut budget(),
                                &mut SourceAdmission::default(),
                            )
                            .map_err(err)?;
                    }
                    let mut store = SourceStore::default();
                    for source in reply.sources() {
                        store.insert(source.clone()).map_err(err)?;
                    }
                    reply
                        .report
                        .validate(&store, &[], profile.registry(), &mut budget())
                        .map_err(err)?;
                    nepl3_core::origin::SourceMap::validate_mappings(
                        reply.source_maps(),
                        &store,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    assert_eq!(reply.report.usage, b.usage());
                }
            }
            assert!(
                retained > 0,
                "sweep must stop after the formal callback event"
            );
            Ok(())
        },
    )
}
#[test]
fn custom_resolution_history_preserves_issuance_and_updates_open_inputs() -> Result<(), String> {
    let mut compiled = compiled()?;
    for policy in [
        nepl3_engine::package::NamespacePolicy::Lexical,
        nepl3_engine::package::NamespacePolicy::Open,
    ] {
        compiled.package.namespaces[0].policy = policy;
        with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
            let baseline = analyze(
                "history",
                tree,
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let BindingOutcome::Invalid {
                progress,
                error: BindingError::MissingProvider,
            } = baseline.outcome
            else {
                return Err(format!("{baseline:?}"));
            };
            let original = progress.occurrence_stages[0];
            let mut host = query_host(true, false);
            let reply = analyze_with_host(
                "history",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let BindingOutcome::Complete(analysis) = &reply.outcome else {
                return Err(format!("{reply:?}"));
            };
            assert_eq!(analysis.result().occurrence_stages[0], original);
            assert!(analysis.result().open_inputs.is_empty());
            let history = &analysis.result().resolution_history;
            assert_eq!(history.len(), 1);
            assert_eq!(history[0].updates.len(), 1);
            assert_eq!(
                history[0].updates[0].before,
                ReferenceResolution::Unresolved("x".into())
            );
            assert_eq!(
                history[0].updates[0].after,
                ReferenceResolution::Resolved(EntityId(100))
            );
            assert_eq!(
                analysis.facts().occurrences[0].resolution,
                history[0].updates[0].after
            );
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let value = nepl3_engine::portable::binding::reply_to_value(
                &reply,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let decoded = nepl3_engine::portable::binding::reply_from_value(
                &value,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let again = nepl3_engine::portable::binding::decoded_to_value(
                &decoded,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(value, again);
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn custom_declared_mapping_and_report_survive_cancel() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(
        &compiled,
        "early missing z custom x x",
        |tree, profile, _, _| {
            let mut host = Host {
                registered: true,
                cancel: true,
                artifacts: true,
                invalid_id: true,
                ..Default::default()
            };
            let reply = analyze_with_host(
                "artifacts",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(
                reply.outcome,
                BindingOutcome::Stopped {
                    reason: StopReason::Cancelled,
                    ..
                }
            ));
            assert_eq!(reply.report.diagnostics.len(), 2);
            assert_eq!(reply.report.events.len(), 1);
            assert_eq!(reply.source_maps().len(), 1);
            assert_eq!(reply.facts().ok_or("facts")?.entities.len(), 1);
            let mut store = SourceStore::default();
            for source in reply.sources() {
                store.insert(source.clone()).map_err(err)?;
            }
            reply
                .report
                .validate(&store, &[], profile.registry(), &mut budget())
                .map_err(err)?;
            nepl3_core::origin::SourceMap::validate_mappings(
                reply.source_maps(),
                &store,
                &mut budget(),
            )
            .map_err(err)?;
            Ok(())
        },
    )
}
#[test]
fn custom_global_checks_existing_batch_duplicates_and_lexical_authority() -> Result<(), String> {
    let mut compiled = compiled()?;
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Global;
    for (input, duplicate, source_less) in [
        ("early x x custom x x", false, true),
        ("custom x x", true, true),
        ("lambda x custom y y", false, false),
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut host = Host {
                registered: true,
                duplicate,
                source_less,
                ..Default::default()
            };
            let reply = analyze_with_host(
                "global-custom",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let BindingOutcome::Invalid { error, .. } = &reply.outcome else {
                return Err(format!("{reply:?}"));
            };
            if source_less {
                assert_eq!(*error, BindingError::DuplicateGlobal);
                let diagnostic = reply.report.diagnostics.last().ok_or("diagnostic")?;
                assert_eq!(diagnostic.code, "DuplicateGlobalName");
                assert!(diagnostic.primary.is_none());
            } else {
                assert_eq!(*error, BindingError::NamespaceBoundary);
            }
            reply
                .facts()
                .ok_or("facts")?
                .validate(
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
            assert!(
                reply
                    .facts()
                    .ok_or("facts")?
                    .entities
                    .iter()
                    .all(|v| v.id.0 < 100)
            );
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn custom_provider_sparse_ids_then_normal_binding_and_accepted_stop() -> Result<(), String> {
    let compiled = compiled()?;
    let input = "early missing z custom x early x y x";
    with_input(&compiled, input, |tree, profile, _, _| {
        for (registered, cancel, invalid_id) in [
            (true, false, false),
            (false, false, false),
            (true, true, true),
            (true, false, true),
        ] {
            let mut host = Host {
                calls: 0,
                registered,
                cancel,
                invalid_id,
                ..Default::default()
            };
            let mut b = budget();
            let reply = analyze_with_host(
                "custom",
                tree,
                profile,
                &mut host,
                &mut b,
                &mut SourceAdmission::default(),
            );
            match &reply.outcome {
                BindingOutcome::Complete(analysis) if registered && !cancel && !invalid_id => {
                    let facts = analysis.facts();
                    assert_eq!(
                        facts.entities.iter().map(|v| v.id.0).collect::<Vec<_>>(),
                        [0, 100, 101]
                    );
                    let x: Vec<_> = facts
                        .occurrences
                        .iter()
                        .filter(|v| v.role == OccurrenceRole::Reference && v.name == "x")
                        .collect();
                    assert_eq!(x.len(), 2);
                    assert!(
                        x.iter()
                            .all(|v| v.resolution == ReferenceResolution::Resolved(EntityId(100)))
                    );
                    assert_eq!(reply.report.diagnostics.len(), 2);
                }
                BindingOutcome::Invalid {
                    error: BindingError::MissingProvider,
                    ..
                } if !registered => assert_eq!(host.calls, 0),
                BindingOutcome::Stopped {
                    reason: StopReason::Cancelled,
                    ..
                } if cancel => {
                    assert_eq!(reply.facts().ok_or("facts")?.entities.len(), 1);
                    assert_eq!(reply.report.diagnostics.len(), 2);
                    assert_eq!(reply.report.events.len(), 1);
                }
                BindingOutcome::Invalid {
                    error: BindingError::Fact(FactError::Reservation),
                    ..
                } if invalid_id => {
                    assert_eq!(reply.facts().ok_or("facts")?.entities.len(), 1);
                    assert_eq!(reply.report.events.len(), 1);
                }
                _ => {
                    return Err(format!(
                        "registered={registered} cancel={cancel} invalid={invalid_id}: {reply:?}"
                    ));
                }
            }
            reply
                .facts()
                .ok_or("facts")?
                .validate(
                    profile.registry(),
                    &mut budget(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
        }
        Ok(())
    })
}
#[test]
fn borrowed_facts_boundary_shares_owned_validation_without_tree_or_fact_copies()
-> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, b, a| {
        let reply = analyze("borrowed-facts", tree, profile, b, a);
        let BindingOutcome::Complete(analysis) = reply.outcome else {
            return Err(format!("{reply:?}"));
        };
        let facts = analysis.facts();
        let root = facts.namespaces[0].root;
        let reservation = FactReservation {
            scopes: IdRange {
                start: facts.scopes.len() as u64,
                end: facts.scopes.len() as u64,
            },
            entities: IdRange {
                start: facts.entities.len() as u64,
                end: facts.entities.len() as u64,
            },
            occurrences: IdRange {
                start: facts.occurrences.len() as u64,
                end: facts.occurrences.len() as u64,
            },
            relations: IdRange { start: 0, end: 0 },
        };
        let authority = FactAuthority {
            analysis_id: facts.analysis_id.clone(),
            current_scope: root,
            namespaces: vec![NamespaceRef(0)],
            writable_scopes: vec![root],
            import_scopes: vec![],
            resolution_updates: vec![],
            relation_sources: vec![],
            reservation,
        };
        let view = FactsRequestView {
            tree: tree.tree(),
            path: &[],
            node: tree.tree().bundle.root,
            existing: facts,
            authority: &authority,
            phase: &nepl3_engine::facts::FactsPhase::Ordinary,
        };
        let owned = FactsRequest {
            tree: tree.tree().clone(),
            path: vec![],
            node: view.node,
            existing: facts.clone(),
            authority: authority.clone(),
            phase: nepl3_engine::facts::FactsPhase::Ordinary,
        };
        let mut owned_budget = budget();
        let mut borrowed_budget = budget();
        let checked_owned = owned
            .issue(profile, &mut owned_budget, &mut SourceAdmission::default())
            .map_err(err)?;
        let checked_view = view
            .issue(
                profile,
                &mut borrowed_budget,
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        assert!(core::ptr::eq(checked_view.request().tree, tree.tree()));
        assert!(core::ptr::eq(checked_view.request().existing, facts));
        assert_eq!(owned_budget.usage(), borrowed_budget.usage());
        let empty = SourceStore::default();
        let mut owned_admission = SourceAdmission::default();
        let mut borrowed_admission = SourceAdmission::default();
        let mut owned_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut owned_admission).map_err(err)?;
        let mut borrowed_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut borrowed_admission)
                .map_err(err)?;
        let mut owned_encoding = budget();
        let mut borrowed_encoding = budget();
        let owned_value = nepl3_engine::portable::facts::request_to_value(
            &checked_owned,
            &mut owned_codec,
            &mut owned_encoding,
        )
        .map_err(err)?;
        let borrowed_value = nepl3_engine::portable::facts::request_view_to_value(
            &checked_view,
            &mut borrowed_codec,
            &mut borrowed_encoding,
        )
        .map_err(err)?;
        assert_eq!(owned_value, borrowed_value);
        assert_eq!(owned_encoding.usage(), borrowed_encoding.usage());
        let response = FactsReply::Complete {
            delta: FactDelta {
                analysis_id: facts.analysis_id.clone(),
                origin_base: facts.origins.len() as u64,
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
            report: Default::default(),
            sources: vec![],
            source_maps: vec![],
        };
        let mut ob = budget();
        let mut vb = budget();
        response
            .validate(&checked_owned, &mut ob, &mut SourceAdmission::default())
            .map_err(err)?;
        response
            .validate_view(&checked_view, &mut vb, &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(ob.usage(), vb.usage());
        let mut foreign = authority.clone();
        foreign.analysis_id = "other-analysis".into();
        let rejected = FactsRequestView {
            authority: &foreign,
            ..view
        }
        .issue(profile, &mut budget(), &mut SourceAdmission::default());
        assert!(matches!(
            rejected,
            Err(FactsError::Fact(FactError::Analysis))
        ));
        response
            .validate_view(
                &checked_view,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        let mut limited = Budget::new(Limits {
            source_bytes: 0,
            ..budget().limits()
        });
        assert!(matches!(
            view.issue(profile, &mut limited, &mut SourceAdmission::default()),
            Err(FactsError::Stopped(StopReason::SourceLimit))
        ));
        Ok(())
    })
}
