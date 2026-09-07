use super::*;
use nepl3_core::{
    budget::{Budget, StopReason},
    schema::{TypeDescriptor, TypeRef},
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::EnvironmentRef,
    value::{KindRef, NdfValue},
};
use nepl3_engine::{
    head::*,
    portable::{PortableError, head::*},
    selection::HeadShape,
};
use nepl3_wire::foundation::FoundationCodec;
fn transport_reserve() -> HeadTransportReserve {
    HeadTransportReserve {
        work: 100_000,
        nodes: 10_000,
        allocation_units: 1_000_000,
        output_bytes: 100_000,
        diagnostics: 10,
        events: 10,
    }
}

#[test]
fn delegated_grant_and_real_framing_share_the_original_work_cap() -> TestResult {
    use nepl3_core::budget::Resource;
    with_profile(|profile| {
        let call = call(profile)?;
        let source = SourceSnapshot::new(
            SourceId("projection-source".into()),
            0,
            "memory:projection".into(),
            "head λ unread".as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(error)?;
        let source_table = [source];
        let mut cap = budget().limits();
        cap.work = 500_000;
        let mut parent = Budget::new(cap);
        let mut admission = SourceAdmission::default();
        let mut issued = IssuedHeadDelegation::issue(
            &call,
            profile,
            &[&source_table],
            HeadTransportReserve {
                work: 20_000,
                ..transport_reserve()
            },
            &mut parent,
            &mut admission,
        )
        .map_err(error)?;
        let empty = SourceStore::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
        let packet = issued.to_value(&mut codec).map_err(error)?;
        let bytes = issued
            .transport(|b| nepl3_wire::encode(&packet, b))
            .map_err(error)?;
        // This receiver cap is the authenticated host-issued grant. A raw
        // network packet's claimed Limits are not an execution authorization.
        let mut framing = Budget::new(issued.limits());
        let value = nepl3_wire::decode(&bytes, &mut framing).map_err(error)?;
        let mut receiver_admission = SourceAdmission::default();
        let mut receiver_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut receiver_admission)
                .map_err(error)?;
        let grant = delegation_limits(
            &value,
            profile.registry(),
            &mut receiver_codec,
            &mut framing,
        )
        .map_err(error)?;
        assert_eq!(grant, issued.limits());
        let mut remote = Budget::new(receiver_limits(grant, framing.usage()).map_err(error)?);
        let received =
            delegation_decode(&value, profile, &mut receiver_codec, &mut remote).map_err(error)?;
        let mut iterations = 0;
        loop {
            match received.call.validate_projection(profile, &mut remote) {
                Ok(()) => iterations += 1,
                Err(HeadError::Stopped(StopReason::WorkLimit)) => break,
                other => return Err(format!("unexpected repeated projection: {other:?}").into()),
            }
        }
        assert!(iterations > 100);
        let observed = metered_usage(framing.usage(), remote.usage()).map_err(error)?;
        let before = issued.parent_usage();
        assert!(before.work + observed.work <= cap.work);
        issued.settle(observed).map_err(error)?;
        assert_eq!(issued.parent_usage().work, before.work + observed.work);
        assert_eq!(issued.settle(observed), Err(HeadError::Report));
        // Only the unused child grant is available again after settlement.
        issued
            .transport(|b| b.charge(Resource::Work, 1))
            .map_err(error)?;
        drop(issued);
        assert_eq!(parent.poll(), Ok(()));
        assert!(parent.usage().work <= cap.work);
        Ok(())
    })
}

#[test]
fn stopped_transport_records_child_cost_and_unsettled_drop_blocks_parent_reuse() -> TestResult {
    use nepl3_core::budget::{Resource, Usage};
    with_profile(|profile| {
        let call = call(profile)?;
        let source = SourceSnapshot::new(
            SourceId("projection-source".into()),
            0,
            "memory:projection".into(),
            "head λ unread".as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(error)?;
        let source_table = [source];
        for (resource, reason) in [
            (Resource::Work, StopReason::WorkLimit),
            (Resource::Nodes, StopReason::NodeLimit),
            (Resource::AllocationUnits, StopReason::AllocationLimit),
            (Resource::OutputBytes, StopReason::OutputLimit),
            (Resource::Diagnostics, StopReason::DiagnosticLimit),
            (Resource::Events, StopReason::EventLimit),
        ] {
            let mut parent = budget();
            let original_limits = parent.limits();
            let mut admission = SourceAdmission::default();
            let mut issued = IssuedHeadDelegation::issue(
                &call,
                profile,
                &[&source_table],
                transport_reserve(),
                &mut parent,
                &mut admission,
            )
            .map_err(error)?;
            let mut child = Budget::new(issued.limits());
            child.charge(resource, 1).map_err(error)?;
            let observed = child.usage();
            let before = issued.parent_usage();
            assert_eq!(
                issued.transport(|b| b.charge(resource, u64::MAX)),
                Err(reason)
            );
            assert_eq!(issued.parent_usage(), before);
            assert_eq!(issued.settle(observed), Err(HeadError::Stopped(reason)));
            let expected = metered_usage(before, observed).map_err(error)?;
            assert_eq!(issued.parent_usage(), expected);
            assert_eq!(issued.settle(Usage::default()), Err(HeadError::Report));
            drop(issued);
            assert_eq!(parent.limits(), original_limits);
            assert_eq!(parent.poll(), Err(reason));
        }
        let mut parent = budget();
        let mut admission = SourceAdmission::default();
        let issued = IssuedHeadDelegation::issue(
            &call,
            profile,
            &[&source_table],
            transport_reserve(),
            &mut parent,
            &mut admission,
        )
        .map_err(error)?;
        let before = issued.parent_usage();
        drop(issued); // Child termination/actual usage was never confirmed.
        assert_eq!(parent.poll(), Err(StopReason::Cancelled));
        assert_eq!(parent.usage(), before); // Reserved capacity is not fake Usage.
        assert_eq!(parent.charge(Resource::Work, 1), Err(StopReason::Cancelled));
        Ok(())
    })
}

fn report(call: &HeadCall) -> ProjectedReport {
    use nepl3_core::{
        budget::Usage,
        diagnostic::Severity,
        value::{Record, TypedValue},
    };
    let arguments = TypedValue::Record(Record {
        schema: call.entry.package.schema.clone(),
        kind: "Leaf:Name".into(),
        fields: vec![],
    });
    ProjectedReport {
        diagnostics: vec![ProjectedDiagnostic {
            schema: call.entry.package.schema.clone(),
            code: "head-note".into(),
            severity: Severity::Error,
            stage: "head".into(),
            arguments: arguments.clone(),
            primary: Some(call.head.window.span.clone()),
            related: vec![ProjectedRelated {
                span: Some(call.head.window.span.clone()),
                code: "related".into(),
                arguments: arguments.clone(),
            }],
            fixes: vec![ProjectedFix {
                id: "replace-head".into(),
                edits: vec![ProjectedEdit {
                    span: call.head.window.span.clone(),
                    expected_digest: Digest::of(&call.head.window.bytes),
                    replacement: "corrected".into(),
                }],
            }],
        }],
        events: vec![ProjectedEvent {
            schema: call.entry.package.schema.clone(),
            kind: "head-event".into(),
            operation_path: vec![0],
            span: Some(call.head.window.span.clone()),
            payload: arguments,
        }],
        trace_overflow: None,
        usage: Usage {
            diagnostics: 1,
            events: 1,
            ..Usage::default()
        },
    }
}

#[test]
fn head_all_reply_branches_preserve_projected_report_and_reject_invalid_fixes_and_graphs()
-> TestResult {
    with_profile(|profile| {
        let child = call(profile)?;
        let HeadRequest::ChildContext { shape, .. } = &child.request else {
            return Err("child".into());
        };
        let shape = shape.clone();
        let mut shape_call = child.clone();
        shape_call.request = HeadRequest::Shape;
        shape_call.identity.operation = profile
            .head_provider("A", "Expr", &mut budget())
            .map_err(error)?
            .ok_or("provider")?
            .shape
            .clone();
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
        for branch in 0..5 {
            let call = if branch < 2 { &shape_call } else { &child };
            let report = report(call);
            let outcome = match branch {
                0 => HeadOutcome::Shape { shape: None },
                1 => HeadOutcome::Shape {
                    shape: Some(shape.clone()),
                },
                2 => HeadOutcome::ChildContext {
                    context: profile.entry("A", None, &mut budget()).map_err(error)?,
                },
                3 => HeadOutcome::Failed {
                    diagnostic: Box::new(report.diagnostics[0].clone()),
                },
                _ => HeadOutcome::Stopped {
                    reason: StopReason::Cancelled,
                },
            };
            let reply = HeadReply {
                identity: call.identity.clone(),
                outcome,
                report,
            };
            let value =
                reply_to_value(&reply, call, profile, &mut codec, &mut budget()).map_err(error)?;
            assert_eq!(
                reply_decode(&value, call, profile, &mut codec, &mut budget()).map_err(error)?,
                reply
            );
            let mut overflow = reply.clone();
            overflow.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert_eq!(
                call.validate_reply(&overflow, profile, &mut budget())
                    .is_ok(),
                branch == 4
            );
            let mut raw_overflow = value.clone();
            *field(field(&mut raw_overflow, 2)?, 2)? =
                NdfValue::Some(Box::new(NdfValue::Record(nepl3_core::value::Record {
                    schema: profile
                        .registry()
                        .selected("nepl3.foundation", 1)
                        .ok_or("foundation")?
                        .clone(),
                    kind: "TraceOverflow".into(),
                    fields: vec![NdfValue::U64(1)],
                })));
            profile
                .registry()
                .validate(
                    &TypeDescriptor::Named(TypeRef {
                        package: "nepl3.engine".into(),
                        revision: 1,
                        name: "HeadReply".into(),
                    }),
                    &raw_overflow,
                    &mut budget(),
                )
                .map_err(error)?;
            assert_eq!(
                reply_decode(&raw_overflow, call, profile, &mut codec, &mut budget()).is_ok(),
                branch == 4
            );
            for stop in 0..4 {
                let mut limits = budget().limits();
                let reason = match stop {
                    0 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    2 => {
                        limits.depth = 0;
                        StopReason::DepthLimit
                    }
                    _ => StopReason::Cancelled,
                };
                let mut b = Budget::new(limits);
                if stop == 3 {
                    b.cancel();
                }
                assert!(
                    matches!(reply_decode(&value,call,profile,&mut codec,&mut b),Err(PortableError::Stopped(v)) if v==reason)
                );
            }
        }
        let reply = HeadReply {
            identity: child.identity.clone(),
            outcome: HeadOutcome::Stopped {
                reason: StopReason::Cancelled,
            },
            report: report(&child),
        };
        for mutation in 0..5 {
            let mut bad = reply.clone();
            match mutation {
                0 => {
                    bad.report.diagnostics[0].fixes[0].edits[0].expected_digest =
                        Digest::of(b"wrong")
                }
                1 => {
                    let duplicate = bad.report.diagnostics[0].fixes[0].edits[0].clone();
                    bad.report.diagnostics[0].fixes[0].edits.push(duplicate);
                }
                2 => {
                    bad.report.diagnostics[0]
                        .primary
                        .as_mut()
                        .ok_or("primary")?
                        .end = 100
                }
                3 => bad.identity.call_id += 1,
                _ => bad.report.usage.diagnostics = 0,
            }
            assert!(
                child.validate_reply(&bad, profile, &mut budget()).is_err(),
                "native {mutation}"
            );
        }
        // Receiver-only range checks neither repair malformed graph references
        // nor allow an extra unreachable node to disclose unrelated syntax.
        for mutation in 0..3 {
            let mut bad = child.clone();
            let HeadRequest::ChildContext { completed, .. } = &mut bad.request else {
                return Err("child".into());
            };
            match mutation {
                0 => completed.nodes.push(completed.nodes[0].clone()),
                1 => {
                    completed.nodes[0].kind = "Form:Let".into();
                    completed.nodes[0].fields = vec![
                        ProjectedFieldValue::Child(ProjectedNodeRef(0)),
                        ProjectedFieldValue::Child(ProjectedNodeRef(0)),
                    ];
                }
                _ => completed.roots.push(ProjectedNodeRef(0)),
            }
            assert!(
                bad.validate_projection(profile, &mut budget()).is_err(),
                "graph {mutation}"
            );
        }
        Ok(())
    })
}

fn with_profile(test: impl FnOnce(&ResolvedParseProfile<'_>) -> TestResult) -> TestResult {
    let (package, registry) = fixture()?;
    let identity = package
        .check(&registry, &mut budget())
        .map_err(error)?
        .semantic_identity(&mut budget())
        .map_err(error)?;
    let engine = registry.selected("nepl3.engine", 1).ok_or("engine")?;
    let operation = |name: &str| OperationRef {
        schema: engine.clone(),
        name: name.into(),
    };
    let provider = HeadProviderRef {
        shape: operation("headShape"),
        child_context: operation("headChildContext"),
    };
    let operations = vec![provider.shape.clone(), provider.child_context.clone()];
    let host = ProviderImplementation {
        provider: "projection-test".into(),
        revision: 1,
        implementation_digest: Digest::of(b"local test implementation"),
        operations: operations.clone(),
    };
    let profile = ParseProfile {
        id: "projection".into(),
        languages: vec![LanguageRegistration {
            alias: "A".into(),
            package: identity,
            default_category: "Expr".into(),
        }],
        schemas: vec![
            package.schema.clone(),
            engine.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
            registry
                .selected("nepl3.reader", 1)
                .ok_or("reader")?
                .clone(),
        ],
        category_modes: vec![],
        head_providers: vec![HeadRegistration {
            alias: "A".into(),
            category: "Expr".into(),
            provider,
        }],
        providers: operations
            .iter()
            .map(|operation| ProviderRequirement {
                provider: host.provider.clone(),
                revision: 1,
                implementation_digest: host.implementation_digest,
                operation: operation.clone(),
            })
            .collect(),
        allowlist: operations,
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&package];
    let hosts = [host];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &hosts,
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(error)?;
    test(&resolved)
}
fn call(profile: &ResolvedParseProfile<'_>) -> Result<HeadCall, String> {
    let package = profile.language("A", &mut budget()).map_err(error)?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let source = SourceSnapshot::new(
        SourceId("projection-source".into()),
        0,
        "memory:projection".into(),
        "head λ unread".as_bytes().to_vec(),
        &mut b,
    )
    .map_err(error)?;
    let head_window = SourceWindow::capture(&source, 0, 4, &mut b, &mut a).map_err(error)?;
    let child_window = SourceWindow::capture(&source, 5, 7, &mut b, &mut a).map_err(error)?;
    let kind = KindRef {
        schema: package.schema.clone(),
        local_kind: profile
            .registry()
            .kind_id(&package.schema, "Token:Word")
            .map_err(error)?,
    };
    let entry = profile.entry("A", None, &mut b).map_err(error)?;
    let form = &package.forms[0];
    Ok(HeadCall {
        identity: HeadCallIdentity {
            session_id: "projection-operation".into(),
            call_id: 3,
            operation: profile
                .head_provider("A", "Expr", &mut b)
                .map_err(error)?
                .ok_or("provider")?
                .child_context
                .clone(),
            profile_digest: profile.digest(),
            execution_digest: profile.execution_digest("A", &mut b).map_err(error)?,
        },
        depth_base: 2,
        entry,
        environment: EnvironmentRef {
            id: 42,
            digest: Digest::of(b"opaque local environment identity"),
        },
        head: ProjectedHead {
            token: ProjectedToken {
                kind: kind.clone(),
                head: head_window.span.clone(),
                payload: NdfValue::Text("head".into()),
            },
            window: head_window,
        },
        request: HeadRequest::ChildContext {
            shape: Box::new(HeadShape {
                kind: form.kind.clone(),
                fields: form.fields.clone(),
                binding: form.binding,
                selection_rules: vec![],
                styles: form.styles.clone(),
            }),
            index: 1,
            completed: ProjectedSyntax {
                nodes: vec![ProjectedNode {
                    schema: package.schema.clone(),
                    kind: "Builtin:Name".into(),
                    head: Some(child_window.span.clone()),
                    cover: Some(child_window.span.clone()),
                    token: Some(ProjectedToken {
                        kind,
                        head: child_window.span.clone(),
                        payload: NdfValue::List(vec![
                            NdfValue::Text("body".into()),
                            NdfValue::U64(7),
                        ]),
                    }),
                    fields: vec![],
                }],
                roots: vec![ProjectedNodeRef(0)],
                windows: vec![child_window.clone(), child_window],
            },
        },
    })
}
fn field(v: &mut NdfValue, index: usize) -> Result<&mut NdfValue, String> {
    match v {
        NdfValue::Record(v) => v.fields.get_mut(index),
        NdfValue::Variant(v) => v.fields.get_mut(index),
        _ => None,
    }
    .ok_or("field".into())
}
fn item(v: &mut NdfValue, index: usize) -> Result<&mut NdfValue, String> {
    match v {
        NdfValue::List(v) => v.get_mut(index),
        _ => None,
    }
    .ok_or("item".into())
}

#[test]
fn head_first_receiver_preserves_compound_projection_and_rejects_schema_valid_corruption()
-> TestResult {
    with_profile(|profile| {
        // Only NDF survives the sender block: no original call, SourceStore,
        // complete SourceSnapshot or sender admission is needed by the receiver.
        let value = {
            let sender = call(profile)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
            call_to_value(&sender, profile, &mut codec, &mut budget()).map_err(error)?
        };
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(error)?;
        let mut b = budget();
        let mut windows = WindowAdmission::default();
        let received =
            call_decode(&value, profile, &mut codec, &mut windows, &mut b).map_err(error)?;
        assert_eq!(b.usage().source_bytes, 6); // 4 head bytes + 2 lambda bytes, duplicate windows once.
        let HeadRequest::ChildContext { completed, .. } = &received.request else {
            return Err("child request".into());
        };
        assert_eq!(
            completed.nodes[0].token.as_ref().ok_or("token")?.payload,
            NdfValue::List(vec![NdfValue::Text("body".into()), NdfValue::U64(7)])
        );
        assert_eq!(
            received
                .slice(&completed.windows[0].span, &mut b)
                .map_err(error)?,
            "λ"
        );
        let again =
            call_decode(&value, profile, &mut codec, &mut windows, &mut b).map_err(error)?;
        assert_eq!(again, received);
        assert_eq!(b.usage().source_bytes, 6);
        assert_eq!(
            call_to_value(&received, profile, &mut codec, &mut b).map_err(error)?,
            value
        );
        let expected = TypeDescriptor::Named(TypeRef {
            package: "nepl3.engine".into(),
            revision: 1,
            name: "HeadCall".into(),
        });
        for mutation in 0..9 {
            let mut bad = value.clone();
            match mutation {
                0 => *field(field(&mut bad, 0)?, 3)? = NdfValue::Bytes(vec![0; 32]),
                1 => *field(field(&mut bad, 0)?, 4)? = NdfValue::Bytes(vec![0; 32]),
                2 => {
                    *field(field(field(&mut bad, 0)?, 2)?, 1)? =
                        NdfValue::Text("bindingFacts".into())
                }
                3 => *field(field(&mut bad, 2)?, 3)? = NdfValue::Text("MissingMode".into()),
                4 => {
                    *field(item(field(field(field(&mut bad, 5)?, 2)?, 1)?, 0)?, 0)? =
                        NdfValue::U64(9)
                }
                5 => {
                    *field(item(field(field(field(&mut bad, 5)?, 2)?, 2)?, 0)?, 1)? =
                        NdfValue::Bytes(vec![0xff, 0xff])
                }
                6 => {
                    *field(
                        field(item(field(field(field(&mut bad, 5)?, 2)?, 2)?, 0)?, 0)?,
                        1,
                    )? = NdfValue::U64(8)
                }
                7 => {
                    *field(item(field(field(field(&mut bad, 5)?, 2)?, 2)?, 1)?, 1)? =
                        NdfValue::Bytes(b"ab".to_vec())
                }
                _ => {
                    // Well-formed UTF-8 window whose start cuts the prior lambda.
                    let w = item(field(field(field(&mut bad, 5)?, 2)?, 2)?, 1)?;
                    *field(field(w, 0)?, 1)? = NdfValue::U64(6);
                    *field(w, 1)? = NdfValue::Bytes(b"x".to_vec());
                }
            }
            profile
                .registry()
                .validate(&expected, &bad, &mut budget())
                .map_err(error)?;
            assert!(
                call_decode(
                    &bad,
                    profile,
                    &mut codec,
                    &mut WindowAdmission::default(),
                    &mut budget()
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
        for resource in 0..5 {
            let mut limits = budget().limits();
            let reason = match resource {
                0 => {
                    limits.source_bytes = 0;
                    StopReason::SourceLimit
                }
                1 => {
                    limits.work = 0;
                    StopReason::WorkLimit
                }
                2 => {
                    limits.depth = 0;
                    StopReason::DepthLimit
                }
                3 => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut stopped = Budget::new(limits);
            if resource == 4 {
                stopped.cancel();
            }
            assert!(
                matches!(call_decode(&value, profile, &mut codec, &mut WindowAdmission::default(), &mut stopped), Err(PortableError::Stopped(v)) if v==reason)
            );
        }
        let reply = HeadReply {
            identity: received.identity.clone(),
            outcome: HeadOutcome::ChildContext {
                context: profile.entry("A", None, &mut b).map_err(error)?,
            },
            report: ProjectedReport {
                diagnostics: vec![],
                events: vec![],
                trace_overflow: None,
                usage: b.usage(),
            },
        };
        let encoded =
            reply_to_value(&reply, &received, profile, &mut codec, &mut b).map_err(error)?;
        assert_eq!(
            reply_decode(&encoded, &received, profile, &mut codec, &mut b).map_err(error)?,
            reply
        );
        Ok(())
    })
}

#[test]
fn head_delegation_accounts_window_reception_once_and_merges_relative_costs_with_absolute_depth()
-> TestResult {
    with_profile(|profile| {
        let call = call(profile)?;
        let text = "head λ unread";
        let source = SourceSnapshot::new(
            SourceId("projection-source".into()),
            0,
            "memory:projection".into(),
            text.as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(error)?;
        let mut limits = budget().limits();
        limits.source_bytes = text.len() as u64;
        let mut parent = Budget::new(limits);
        parent.observe_depth(400).map_err(error)?;
        let mut parent_admission = SourceAdmission::default();
        let source_table = [source];
        let mut issued = IssuedHeadDelegation::issue(
            &call,
            profile,
            &[&source_table],
            transport_reserve(),
            &mut parent,
            &mut parent_admission,
        )
        .map_err(error)?;
        assert_eq!(issued.parent_usage().source_bytes, limits.source_bytes);
        assert_eq!(issued.limits().source_bytes, 6); // Explicit window union, not parent remaining 0.
        assert_eq!(issued.limits().depth, limits.depth); // Peak 400 is not subtracted.
        let empty = SourceStore::default();
        let mut host_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut parent_admission)
                .map_err(error)?;
        let packet = issued.to_value(&mut host_codec).map_err(error)?;
        // Independent transport framing has its own host-selected cap. Its
        // cost is not claimed as part of the child operation's Usage.
        let mut remote_admission = SourceAdmission::default();
        let mut remote_codec =
            FoundationCodec::new(profile.registry(), &empty, &mut remote_admission)
                .map_err(error)?;
        let mut framing = budget();
        let remote_limits =
            delegation_limits(&packet, profile.registry(), &mut remote_codec, &mut framing)
                .map_err(error)?;
        let framing_usage = framing.usage();
        let mut remote = Budget::new(receiver_limits(remote_limits, framing_usage).map_err(error)?);
        let received =
            delegation_decode(&packet, profile, &mut remote_codec, &mut remote).map_err(error)?;
        assert_eq!(remote.usage().source_bytes, 6);
        let reply = HeadReply {
            identity: received.call.identity.clone(),
            outcome: HeadOutcome::ChildContext {
                context: profile.entry("A", None, &mut remote).map_err(error)?,
            },
            report: ProjectedReport {
                diagnostics: vec![],
                events: vec![],
                trace_overflow: None,
                usage: remote.usage(),
            },
        };
        let delivery_packet = delivery_to_value(
            &reply,
            &received.call,
            profile,
            &mut remote_codec,
            &mut remote,
            framing_usage,
        )
        .map_err(error)?;
        let bytes = nepl3_wire::encode(&delivery_packet, &mut remote).map_err(error)?;
        // Actual codec/framing costs are metered outside the serialized cutoff;
        // no counter is recursively embedded in its own final serialization.
        let observed = metered_usage(framing_usage, remote.usage()).map_err(error)?;
        let received_value = issued
            .transport(|b| nepl3_wire::decode(&bytes, b))
            .map_err(error)?;
        let delivery =
            delivery_decode(&received_value, &mut issued, &mut host_codec).map_err(error)?;
        assert!(observed.work > delivery.usage.work);
        assert!(observed.allocation_units > delivery.usage.allocation_units);
        let saved_delivery = delivery.clone();
        let before = issued.parent_usage();
        let accepted = issued.accept(delivery, observed).map_err(error)?;
        assert_eq!(parent.usage().source_bytes, text.len() as u64);
        assert_eq!(accepted.report.usage.source_bytes, text.len() as u64);
        assert!(parent.usage().work >= before.work + observed.work);
        assert!(
            parent.usage().allocation_units >= before.allocation_units + observed.allocation_units
        );
        assert_eq!(parent.usage().depth, before.depth.max(observed.depth));
        assert_eq!(parent.usage().depth, 400);
        for mutation in 0..6 {
            let mut parent = budget();
            let mut admission = SourceAdmission::default();
            let mut issued = IssuedHeadDelegation::issue(
                &call,
                profile,
                &[&source_table],
                transport_reserve(),
                &mut parent,
                &mut admission,
            )
            .map_err(error)?;
            let mut delivery = saved_delivery.clone();
            let mut measured = observed;
            match mutation {
                0 => delivery.usage.work = measured.work + 1,
                1 => delivery.reply.identity.call_id += 1,
                2 => measured.depth = issued.limits().depth + 1,
                3 => measured.source_bytes = 7,
                4 => {
                    delivery.reply.report.trace_overflow =
                        Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 })
                }
                _ => {
                    issued
                        .transport(|b| -> Result<(), StopReason> {
                            b.cancel();
                            Ok(())
                        })
                        .map_err(error)?;
                }
            }
            let result = issued.accept(delivery, measured);
            assert!(result.is_err(), "delegation mutation {mutation}");
            if mutation == 5 {
                assert_eq!(result, Err(HeadError::Stopped(StopReason::Cancelled)));
            }
        }
        // A received assertion alone cannot mint the exemption: wrong or absent
        // actual host bytes fail before an IssuedHeadDelegation can be obtained.
        assert!(
            IssuedHeadDelegation::issue(
                &call,
                profile,
                &[],
                transport_reserve(),
                &mut budget(),
                &mut SourceAdmission::default()
            )
            .is_err()
        );
        Ok(())
    })
}

#[test]
fn projected_shared_payload_depth_includes_the_deeper_graph_path() -> TestResult {
    with_profile(|profile| {
        let mut call = call(profile)?;
        let HeadRequest::ChildContext { completed, .. } = &mut call.request else {
            return Err("child".into());
        };
        let mut leaf = completed.nodes[0].clone();
        let mut payload = NdfValue::Unit;
        for _ in 0..16 {
            payload = NdfValue::Some(Box::new(payload));
        }
        leaf.token.as_mut().ok_or("token")?.payload = payload;
        completed.nodes.clear();
        // Every form first visits the same leaf through a shallow edge, then
        // descends to the next form. The deepest revisit must validate payload
        // at that path's depth, even though the node was checked earlier.
        for index in 0..8 {
            let mut node = leaf.clone();
            node.kind = "Form:Let".into();
            node.token = None;
            node.head = None;
            node.cover = None;
            node.fields = vec![
                ProjectedFieldValue::Child(ProjectedNodeRef(8)),
                ProjectedFieldValue::Child(ProjectedNodeRef(index + 1)),
            ];
            completed.nodes.push(node);
        }
        completed.nodes.push(leaf);
        let mut full = budget();
        call.validate_projection(profile, &mut full)
            .map_err(error)?;
        // Absolute call base 2 + graph nodes 9 + 16 Some wrappers and Unit 17.
        assert_eq!(full.usage().depth, 28);
        let mut limits = budget().limits();
        limits.depth = 27;
        assert_eq!(
            call.validate_projection(profile, &mut Budget::new(limits)),
            Err(HeadError::Stopped(StopReason::DepthLimit))
        );
        limits.depth = 28;
        call.validate_projection(profile, &mut Budget::new(limits))
            .map_err(error)?;
        Ok(())
    })
}
