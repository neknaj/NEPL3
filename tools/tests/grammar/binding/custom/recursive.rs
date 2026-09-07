//! Header IDs, lexical references, and phase order are expected from the
//! original source, independently of serialization or runtime arena order.
use super::*;

fn compiled_recursive() -> Result<CompiledLanguage, String> {
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../../../conformance/fixtures/grammar/binding/recursive.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.binding-recursive",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
fn empty(request: FactsRequestView<'_>) -> FactDelta {
    FactDelta {
        analysis_id: request.existing.analysis_id.clone(),
        origin_base: request.existing.origins.len() as u64,
        scopes: vec![],
        entities: vec![],
        occurrences: vec![],
        relations: vec![],
        edges: vec![],
        resolutions: vec![],
        sources: vec![],
        origins: vec![],
        source_maps: vec![],
    }
}
#[derive(Default)]
struct RecursiveHost {
    inner: Host,
    phases: Vec<&'static str>,
    headers: Vec<EntityId>,
    bodies: Vec<EntityId>,
    bad_header: bool,
    redeclare: bool,
    distinct: bool,
    stop_body: bool,
    portable: bool,
    check_claims: bool,
}
impl BindingHost for RecursiveHost {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        self.inner.registered = true;
        self.inner.entity_start = Some(
            call.existing
                .entities
                .iter()
                .map(|v| v.id.0)
                .max()
                .map_or(100, |v| v.saturating_add(1)),
        );
        let mut authority = self.inner.authorize(call, b)?;
        if let Some(authority) = &mut authority {
            let start = call
                .existing
                .occurrences
                .iter()
                .map(|v| v.id.0)
                .max()
                .map_or(200, |v| v.saturating_add(1));
            authority.reservation.occurrences = IdRange {
                start,
                end: start.saturating_add(10),
            };
        }
        Ok(authority)
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        if self.portable {
            self.portable_answer(provider, checked, emit)
        } else {
            self.answer(provider, checked, emit)
        }
    }
}
impl RecursiveHost {
    /// This is synchronous transport work in the same shared parent Budget and
    /// Admission, not an unmetered remote worker or a delegated-quota adapter.
    fn portable_answer(
        &mut self,
        provider: &ProviderRequirement,
        original: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        use nepl3_engine::portable::facts as portable;
        let profile = original.profile();
        let empty_store = SourceStore::default();
        let received = {
            let (b, a) = emit.budget_and_admission();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty_store, a).map_err(wire_error)?;
            let value =
                portable::request_view_to_value(original, &mut codec, b).map_err(portable_error)?;
            let bytes = nepl3_wire::encode(&value, b).map_err(wire_error)?;
            let raw = nepl3_wire::decode(&bytes, b).map_err(wire_error)?;
            if self.check_claims {
                check_claims(&raw, profile)?;
            }
            portable::request_decode(&raw, profile, &mut codec, b).map_err(portable_error)?
        };
        // This host knows it sent this exact packet. Raw decoding alone would
        // not authorize a request arriving from an unauthenticated process.
        let proof = {
            let (b, a) = emit.budget_and_admission();
            received.issue(profile, b, a)?
        };
        let outcome = self.answer(provider, &proof.view(), emit)?;
        let report = nepl3_core::diagnostic::Report {
            usage: emit.budget().usage(),
            ..Default::default()
        };
        // Formal emits already used the shared sink. The wire reply owns only
        // its delta here, so its empty Report cannot duplicate those emissions.
        let reply = match outcome {
            CustomOutcome::Complete(delta) => FactsReply::Complete {
                delta,
                report,
                sources: vec![],
                source_maps: vec![],
            },
            CustomOutcome::Invalid(partial) => FactsReply::Invalid {
                partial,
                report,
                sources: vec![],
                source_maps: vec![],
            },
            CustomOutcome::Stopped { reason, partial } => FactsReply::Stopped {
                reason,
                partial,
                report,
                sources: vec![],
                source_maps: vec![],
            },
        };
        let (b, a) = emit.budget_and_admission();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty_store, a).map_err(wire_error)?;
        let value =
            portable::reply_to_value(&reply, &proof, &mut codec, b).map_err(portable_error)?;
        let bytes = nepl3_wire::encode(&value, b).map_err(wire_error)?;
        let raw = nepl3_wire::decode(&bytes, b).map_err(wire_error)?;
        let reply =
            portable::reply_from_value(&raw, &proof, &mut codec, b).map_err(portable_error)?;
        Ok(match reply {
            FactsReply::Complete { delta, .. } => CustomOutcome::Complete(delta),
            FactsReply::Invalid { partial, .. } => CustomOutcome::Invalid(partial),
            FactsReply::Stopped {
                reason, partial, ..
            } => CustomOutcome::Stopped { reason, partial },
        })
    }
    fn answer(
        &mut self,
        provider: &ProviderRequirement,
        checked: &CheckedFactsView<'_, '_>,
        emit: &mut FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let request = checked.request();
        match request.phase {
            FactsPhase::Body { header } => {
                self.phases.push("body");
                self.bodies.extend_from_slice(&header.entities);
                if self.stop_body {
                    emit.budget().cancel();
                }
                if self.redeclare {
                    let mut delta = empty(request);
                    let id = *header.entities.first().ok_or(BindingError::Target)?;
                    delta.entities.push(
                        request
                            .existing
                            .entities
                            .iter()
                            .find(|e| e.id == id)
                            .ok_or(BindingError::Target)?
                            .clone(),
                    );
                    return Ok(CustomOutcome::Complete(delta));
                }
                if !self.distinct {
                    return Ok(CustomOutcome::Complete(empty(request)));
                }
            }
            FactsPhase::Header { .. } => self.phases.push("header"),
            FactsPhase::Ordinary => self.phases.push("ordinary"),
        }
        let CustomOutcome::Complete(mut delta) = self.inner.facts(provider, checked, emit)? else {
            return Err(BindingError::ProviderInvalid);
        };
        for occurrence in &mut delta.occurrences {
            occurrence.id = OccurrenceId(request.authority.reservation.occurrences.start);
            if matches!(request.phase, FactsPhase::Header { .. }) {
                occurrence.role = if self.bad_header {
                    OccurrenceRole::Reference
                } else {
                    OccurrenceRole::Export
                };
            }
        }
        if matches!(request.phase, FactsPhase::Header { .. }) {
            self.headers.extend(delta.entities.iter().map(|e| e.id));
        }
        Ok(CustomOutcome::Complete(delta))
    }
}
fn wire_error(error: nepl3_wire::WireError) -> BindingError {
    use nepl3_core::value_codec::FoundationCodecError;
    error
        .stop_reason()
        .map_or(BindingError::ProviderInvalid, BindingError::Stopped)
}
fn portable_error(
    error: nepl3_engine::portable::PortableError<nepl3_wire::WireError>,
) -> BindingError {
    use nepl3_engine::portable::PortableError;
    match error {
        PortableError::Stopped(reason) => BindingError::Stopped(reason),
        PortableError::Facts(error) => BindingError::Facts(error),
        PortableError::Boundary(error) => wire_error(error),
        _ => BindingError::ProviderInvalid,
    }
}
fn check_claims(raw: &NdfValue, profile: &ResolvedParseProfile<'_>) -> Result<(), BindingError> {
    use nepl3_engine::portable::facts::request_decode;
    let NdfValue::Record(request) = raw else {
        return Err(BindingError::Target);
    };
    let NdfValue::Variant(phase) = &request.fields[5] else {
        return Err(BindingError::Target);
    };
    if phase.variant == "Ordinary" {
        return Ok(());
    }
    let empty = SourceStore::default();
    let mut changes = Vec::new();
    let mut bad = raw.clone();
    if let NdfValue::Record(r) = &mut bad
        && let NdfValue::Variant(p) = &mut r.fields[5]
    {
        let group = if p.variant == "Body" {
            let NdfValue::Record(h) = &mut p.fields[0] else {
                return Err(BindingError::Target);
            };
            &mut h.fields[0]
        } else {
            &mut p.fields[0]
        };
        let NdfValue::Record(group) = group else {
            return Err(BindingError::Target);
        };
        group.fields[0] = NdfValue::U64(u64::MAX);
    }
    changes.push(bad);
    if phase.variant == "Body" {
        for index in [1, 2, 3, 4] {
            let mut bad = raw.clone();
            let NdfValue::Record(r) = &mut bad else {
                return Err(BindingError::Target);
            };
            let NdfValue::Variant(p) = &mut r.fields[5] else {
                return Err(BindingError::Target);
            };
            let NdfValue::Record(h) = &mut p.fields[0] else {
                return Err(BindingError::Target);
            };
            match index {
                1 => {
                    let NdfValue::Record(provider) = &mut h.fields[1] else {
                        return Err(BindingError::Target);
                    };
                    provider.fields[2] = NdfValue::Bytes(vec![0; 32]);
                }
                2 => {
                    let NdfValue::Record(target) = &mut h.fields[2] else {
                        return Err(BindingError::Target);
                    };
                    let NdfValue::Record(node) = &mut target.fields[1] else {
                        return Err(BindingError::Target);
                    };
                    node.fields[0] = NdfValue::U64(u64::MAX);
                }
                _ => {
                    let NdfValue::List(ids) = &mut h.fields[index] else {
                        return Err(BindingError::Target);
                    };
                    let first = ids.first().ok_or(BindingError::Target)?.clone();
                    ids.push(first);
                }
            }
            changes.push(bad);
        }
    }
    for changed in changes {
        let mut b = budget();
        // All mutations are structurally well typed before semantic receipt checks.
        profile.registry().validate(
            &TypeDescriptor::Named(TypeRef {
                package: "nepl3.engine".into(),
                revision: 1,
                name: "FactsRequest".into(),
            }),
            &changed,
            &mut b,
        )?;
        let mut a = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(wire_error)?;
        assert!(request_decode(&changed, profile, &mut codec, &mut b).is_err());
    }
    Ok(())
}
#[test]
fn recursive_custom_phase_receipts_reject_schema_valid_claims_and_preserve_stop_prefix()
-> Result<(), String> {
    let compiled = compiled_recursive()?;
    let input = "lambda z guest recursive cons customdecl x x nil x";
    with_input(&compiled, input, |tree, profile, _, _| {
        let mut host = RecursiveHost {
            portable: true,
            check_claims: true,
            ..Default::default()
        };
        let mut full = budget();
        let reply = analyze_with_host(
            "receipt",
            tree,
            profile,
            &mut host,
            &mut full,
            &mut SourceAdmission::default(),
        );
        assert!(
            matches!(reply.outcome, BindingOutcome::Complete(_)),
            "{reply:?}"
        );
        for portable in [false, true] {
            for resource in 0..7 {
                let maximum = match resource {
                    0 => full.usage().work,
                    1 => full.usage().allocation_units,
                    2 => full.usage().nodes,
                    3 => full.usage().source_bytes,
                    4 => full.usage().depth,
                    5 => full.usage().diagnostics,
                    _ => full.usage().events,
                };
                for cap in [
                    0,
                    maximum / 4,
                    maximum / 2,
                    maximum.saturating_sub(1),
                    maximum,
                ] {
                    let mut limits = budget().limits();
                    let reason = match resource {
                        0 => {
                            limits.work = cap;
                            StopReason::WorkLimit
                        }
                        1 => {
                            limits.allocation_units = cap;
                            StopReason::AllocationLimit
                        }
                        2 => {
                            limits.nodes = cap;
                            StopReason::NodeLimit
                        }
                        3 => {
                            limits.source_bytes = cap;
                            StopReason::SourceLimit
                        }
                        4 => {
                            limits.depth = cap;
                            StopReason::DepthLimit
                        }
                        5 => {
                            limits.diagnostics = cap;
                            StopReason::DiagnosticLimit
                        }
                        _ => {
                            limits.events = cap;
                            StopReason::EventLimit
                        }
                    };
                    let mut host = RecursiveHost {
                        portable,
                        ..Default::default()
                    };
                    let mut b = Budget::new(limits);
                    let reply = analyze_with_host(
                        "receipt",
                        tree,
                        profile,
                        &mut host,
                        &mut b,
                        &mut SourceAdmission::default(),
                    );
                    match &reply.outcome {
                        BindingOutcome::Complete(_) => {}
                        BindingOutcome::Stopped { reason: actual, .. } => {
                            assert_eq!(*actual, reason)
                        }
                        _ => return Err(format!("resource {resource} cap {cap}: {reply:?}")),
                    }
                    let mut cb = budget();
                    let mut a = SourceAdmission::default();
                    let empty = SourceStore::default();
                    let mut codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                    let value = nepl3_engine::portable::binding::reply_to_value(
                        &reply,
                        profile.registry(),
                        &mut codec,
                        &mut cb,
                    )
                    .map_err(err)?;
                    nepl3_engine::portable::binding::reply_from_value(
                        &value,
                        profile.registry(),
                        &mut codec,
                        &mut cb,
                    )
                    .map_err(err)?;
                }
            }
        }
        Ok(())
    })
}
#[test]
fn recursive_custom_actual_cbor_requests_use_same_phase_callback_and_fact_ids() -> Result<(), String>
{
    let compiled = compiled_recursive()?;
    for input in [
        "recursive cons customdecl x y cons customdecl y x nil apply x y",
        "guest recursive cons customdecl x x nil x",
        "repeat cons customdecl x x nil x",
        "recursive cons customdecl x custom y y nil x",
        "sequence cons customdecl x y cons customdecl y x nil y",
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut expected = None;
            for portable in [false, true] {
                let mut host = RecursiveHost {
                    portable,
                    ..Default::default()
                };
                let mut b = budget();
                let reply = analyze_with_host(
                    "portable-recursive",
                    tree,
                    profile,
                    &mut host,
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                let BindingOutcome::Complete(analysis) = reply.outcome else {
                    return Err(format!("{input} portable={portable}: {reply:?}"));
                };
                let actual = (
                    (
                        analysis.facts().clone(),
                        analysis.result().stages.clone(),
                        analysis.result().occurrence_stages.clone(),
                        analysis.result().exports.clone(),
                        analysis.result().open_inputs.clone(),
                        analysis.result().resolution_history.clone(),
                    ),
                    reply.report.diagnostics,
                    reply.report.events,
                    host.headers,
                    host.bodies,
                    host.phases,
                );
                assert_eq!(b.usage().source_bytes, input.len() as u64);
                if let Some(expected) = &expected {
                    assert_eq!(expected, &actual, "{input}");
                } else {
                    expected = Some(actual);
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn recursive_custom_reuses_header_ids_and_runs_initializers_after_all_headers() -> Result<(), String>
{
    let compiled = compiled_recursive()?;
    for (input, phases, names) in [
        (
            "recursive cons customdef x nil x",
            vec!["header", "body"],
            vec!["x"],
        ),
        (
            "recursive cons customdecl x y cons customdecl y x nil apply x y",
            vec!["header", "header", "body", "body"],
            vec!["x", "y"],
        ),
        (
            "recursive cons define x y cons customdecl y x nil apply x y",
            vec!["header", "body"],
            vec!["y"],
        ),
        (
            "recursive cons wrapped customdecl x x nil x",
            vec!["header", "body"],
            vec!["x"],
        ),
        (
            "repeat cons customdecl x x nil x",
            vec!["header", "body", "header", "body"],
            vec!["x", "x"],
        ),
        (
            "guest recursive cons customdecl λ λ nil λ",
            vec!["header", "body"],
            vec!["λ"],
        ),
        (
            "recursive cons customdecl x custom y y nil x",
            vec!["header", "body", "ordinary"],
            vec!["x"],
        ),
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let mut host = RecursiveHost::default();
            let reply = analyze_with_host(
                "recursive",
                tree,
                profile,
                &mut host,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let BindingOutcome::Complete(analysis) = &reply.outcome else {
                return Err(format!("{input}: {reply:?}"));
            };
            assert_eq!(host.phases, phases, "{input}");
            assert_eq!(host.headers, host.bodies, "one accepted ID per declaration");
            let facts = analysis.facts();
            assert_eq!(
                host.headers
                    .iter()
                    .map(|id| facts
                        .entities
                        .iter()
                        .find(|e| e.id == *id)
                        .map(|e| e.name.as_str()))
                    .collect::<Vec<_>>(),
                names.into_iter().map(Some).collect::<Vec<_>>()
            );
            for occurrence in facts
                .occurrences
                .iter()
                .filter(|o| o.role == OccurrenceRole::Reference)
            {
                let ReferenceResolution::Resolved(id) = occurrence.resolution else {
                    return Err(format!("{input}: {occurrence:?}"));
                };
                assert_eq!(
                    facts.entities.iter().find(|e| e.id == id).map(|e| &e.name),
                    Some(&occurrence.name)
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn recursive_custom_rejects_header_references_and_id_reissue_but_preserves_distinct_names()
-> Result<(), String> {
    let compiled = compiled_recursive()?;
    with_input(
        &compiled,
        "recursive cons customdecl x x nil x",
        |tree, profile, _, _| {
            for mode in 0..4 {
                let mut host = RecursiveHost {
                    bad_header: mode == 0,
                    redeclare: mode == 1,
                    distinct: mode == 2,
                    stop_body: mode == 3,
                    ..Default::default()
                };
                let reply = analyze_with_host(
                    "boundaries",
                    tree,
                    profile,
                    &mut host,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                match (&reply.outcome, mode) {
                    (
                        BindingOutcome::Invalid {
                            error: BindingError::Facts(FactsError::Phase),
                            ..
                        },
                        0,
                    ) => {}
                    (
                        BindingOutcome::Invalid {
                            error: BindingError::Fact(FactError::DuplicateId),
                            ..
                        },
                        1,
                    ) => {}
                    (BindingOutcome::Complete(analysis), 2) => {
                        assert_eq!(analysis.facts().entities.len(), 2);
                        assert!(analysis.facts().occurrences.iter().any(|o|matches!(&o.resolution,ReferenceResolution::Ambiguous(ids) if ids.len()==2)));
                    }
                    (
                        BindingOutcome::Stopped {
                            reason: StopReason::Cancelled,
                            ..
                        },
                        3,
                    ) => {}
                    _ => return Err(format!("mode {mode}: {reply:?}")),
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
                assert_eq!(reply.report.events.len(), if mode == 2 { 2 } else { 1 });
            }
            Ok(())
        },
    )
}
