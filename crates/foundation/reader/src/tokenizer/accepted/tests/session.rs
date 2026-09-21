use super::*;

#[test]
fn session_issues_scope_only_for_host_free_completed_reads()
-> Result<(), crate::runtime::ReaderError> {
    use crate::{model::*, plan::*, tokenizer::*};
    use nepl3_core::{
        schema::*,
        source::SourceStore,
        syntax::{Environment, EnvironmentEntry},
        value::{KindRef, NdfValue},
    };
    let mut setup = Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        ..budget().limits()
    });
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut setup)?;
    let schema = descriptor.reference(&mut setup)?;
    registry.register(schema.clone(), descriptor, &mut setup)?;
    let reader = crate::schema::descriptor(&mut setup)?;
    registry.register(reader.reference(&mut setup)?, reader, &mut setup)?;
    let reader_type = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.reader".into(),
            revision: 1,
            name: name.into(),
        })
    };
    let external = SchemaDescriptor {
        package: "test.admission".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            constraints: vec![],
            shape: TypeShape::Record { fields: vec![] },
        }],
        operations: vec![
            OperationDescriptor {
                name: "read".into(),
                input: reader_type("ReadRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "transform".into(),
                input: reader_type("TransformRequest"),
                output: reader_type("TransformReply"),
                pure: true,
            },
        ],
    };
    let external_ref = external.reference(&mut setup)?;
    registry.register(external_ref.clone(), external, &mut setup)?;
    registry.finalize(&mut setup)?;
    let plan = ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions: vec![ReaderExpr::Literal("x".into())],
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(0),
            output: TypeDescriptor::Unit,
        }],
        providers: vec![],
    };
    let checked = plan.check(&registry, &mut setup)?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        0,
        "memory:input".into(),
        vec![b'x'],
        &mut setup,
    )?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest =
        nepl3_wire::environment::environment_digest(&environment, &schema, &registry, &mut setup)
            .map_err(|_| crate::runtime::ReaderError::Context)?;
    let raw = ReaderContext {
        schema: schema.clone(),
        category: "Token".into(),
        mode: "test".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        origins: vec![],
    };
    let mut declarations = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut declarations)
            .map_err(|_| crate::runtime::ReaderError::Context)?;
    let context = raw
        .check(&mut codec, &store, &registry, &mut setup)
        .map_err(|_| crate::runtime::ReaderError::Context)?;
    let modes = vec![ReaderMode {
        name: "test".into(),
        skip: vec![],
        take: vec![TakeRule {
            reader: TokenReader::Rule("entry".into()),
            kind: KindRef {
                schema,
                local_kind: 0,
            },
        }],
    }];
    let scope = TokenizationScope {
        operation_id: "operation".into(),
        profile_digest: Digest([0; 32]),
        snapshot: source.reference(),
    };
    struct NoCalls;
    impl TokenizationHost for NoCalls {
        fn provider(
            &mut self,
            _: &ProviderCall,
            _: &mut Budget,
            _: &mut SourceAdmission,
        ) -> Result<Option<crate::runtime::ProviderReply>, crate::runtime::ReaderError> {
            Err(crate::runtime::ReaderError::ProviderContract)
        }
        fn reservation(
            &mut self,
            _: &ReservationRequest,
            _: &mut Budget,
            _: &mut SourceAdmission,
        ) -> Result<Option<nepl3_core::source::SourceReservation>, crate::runtime::ReaderError>
        {
            Err(crate::runtime::ReaderError::ProviderContract)
        }
    }
    for (stopped, hosted) in [(false, false), (true, false), (false, true)] {
        let mut b = budget();
        let mut ledger = SourceAdmission::default();
        let mut accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
        // A nonempty private collector starts without an admission proof.
        // The actual read must check it before issuing the new scope.
        accepted.sources.push(source.clone());
        let mut session =
            TokenizationSession::new("session".into(), &modes, &checked, &registry, &mut b)?;
        if stopped {
            b.cancel();
        }
        let request = ScopedTokenizationRequest {
            scope: &scope,
            target: TokenTarget::Mode,
            input: TokenizationRequest {
                snapshot: &source,
                start: 0,
                limit: 1,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
        };
        let reply = if hosted {
            session
                .read_accepted_with_host(
                    request,
                    &store,
                    &mut b,
                    &mut ledger,
                    accepted,
                    &mut NoCalls,
                )?
                .reply
        } else {
            session.read_with_accepted(request, &store, &mut b, &mut ledger, accepted)?
        };
        if stopped {
            assert!(matches!(reply.outcome, TokenizationOutcome::Stopped { .. }));
            assert!(reply.accepted.admission_scope.is_none());
        } else {
            assert!(matches!(reply.outcome, TokenizationOutcome::Token(_)));
            assert_eq!(
                reply.accepted.admission_scope.is_some(),
                cfg!(target_has_atomic = "ptr") && !hosted
            );
            let work = b.usage().work;
            reply.accepted.admit_sources(&mut ledger, &mut b)?;
            if cfg!(target_has_atomic = "ptr") && !hosted {
                assert_eq!(b.usage().work, work);
            }
            let mut branch = reply.accepted.checkpoint(&mut b)?;
            let saved = b.usage();
            // A public append with another ledger invalidates the old proof,
            // even when no additional source or diagnostic is introduced.
            branch.append_report(
                Report {
                    usage: saved,
                    ..Report::default()
                },
                saved,
                &store,
                &registry,
                &mut b,
                &mut SourceAdmission::default(),
            )?;
            assert!(branch.admission_scope.is_none());
            let raw_reply = reply.into_raw();
            let restored =
                AcceptedTokenizationReply::from_native(raw_reply, Rc::new(scope.clone()), &b);
            assert!(restored.accepted.admission_scope.is_none());
        }
    }
    for decode in [false, true] {
        let operation = nepl3_core::value::OperationRef {
            schema: external_ref.clone(),
            name: if decode { "transform" } else { "read" }.into(),
        };
        let mut external_plan = plan.clone();
        external_plan.expressions = if decode {
            vec![
                ReaderExpr::Literal("x".into()),
                ReaderExpr::Decode {
                    provider: operation.clone(),
                    body: ReaderId(0),
                },
            ]
        } else {
            vec![ReaderExpr::Call(operation.clone())]
        };
        external_plan.rules[0].root = ReaderId(if decode { 1 } else { 0 });
        external_plan.rules[0].output = TypeDescriptor::Text;
        external_plan.providers = vec![ProviderSignature {
            operation,
            kind: if decode {
                ProviderKind::Transform
            } else {
                ProviderKind::Read
            },
            value_input: TypeDescriptor::Unit,
            value_output: TypeDescriptor::Text,
            pure: true,
            state_type: TypeDescriptor::Unit,
            continuation_type: reader_type("ReaderContinuation"),
        }];
        let checked = external_plan.check(&registry, &mut setup)?;
        let mut b = budget();
        let mut ledger = SourceAdmission::default();
        let mut session =
            TokenizationSession::new("external".into(), &modes, &checked, &registry, &mut b)?;
        let accepted = AcceptedTokenizationReport::empty(scope.clone(), &mut b)?;
        let reply = session.read_with_accepted(
            ScopedTokenizationRequest {
                scope: &scope,
                target: TokenTarget::Mode,
                input: TokenizationRequest {
                    snapshot: &source,
                    start: 0,
                    limit: 1,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
            },
            &store,
            &mut b,
            &mut ledger,
            accepted,
        )?;
        assert!(reply.accepted.admission_scope.is_none());
        match reply.outcome {
            TokenizationOutcome::Await { continuation, .. } if !decode => {
                let resumed = session.resume_accepted(
                    &continuation,
                    crate::runtime::ProviderReply::Read(alloc::boxed::Box::new(
                        crate::model::ReadReply::Matched {
                            value: NdfValue::Text("x".into()),
                            end: 1,
                            new_state: NdfValue::Unit,
                            view: nepl3_core::view::ViewBundle {
                                elements: vec![],
                                roots: vec![],
                            },
                            facts: vec![],
                            sources: vec![],
                            source_maps: vec![],
                            report: Report {
                                usage: b.usage(),
                                ..Report::default()
                            },
                        },
                    )),
                    &store,
                    &mut b,
                    &mut SourceAdmission::default(),
                )?;
                assert!(matches!(resumed.outcome, TokenizationOutcome::Token(_)));
                assert!(resumed.accepted.admission_scope.is_none());
            }
            TokenizationOutcome::Await { continuation, .. } if decode => {
                let resumed = session.resume_accepted(
                    &continuation,
                    crate::runtime::ProviderReply::Transform(alloc::boxed::Box::new(
                        TransformReply {
                            outcome: TransformOutcome::Complete {
                                value: NdfValue::Text("x".into()),
                                view: nepl3_core::view::ViewBundle {
                                    elements: vec![],
                                    roots: vec![],
                                },
                                facts: vec![],
                            },
                            sources: vec![],
                            source_maps: vec![],
                            report: Report {
                                usage: b.usage(),
                                ..Report::default()
                            },
                        },
                    )),
                    &store,
                    &mut b,
                    &mut SourceAdmission::default(),
                )?;
                assert!(matches!(resumed.outcome, TokenizationOutcome::Token(_)));
                assert!(resumed.accepted.admission_scope.is_none());
            }
            _ => return Err(crate::runtime::ReaderError::Context),
        }
    }
    let quoted = SourceSnapshot::new(
        SourceId("quoted".into()),
        0,
        "memory:quoted".into(),
        b"\"x\"".to_vec(),
        &mut setup,
    )?;
    let mut quoted_store = SourceStore::default();
    quoted_store.insert(quoted.clone())?;
    let quoted_scope = TokenizationScope {
        snapshot: quoted.reference(),
        ..scope.clone()
    };
    let mut text_modes = modes.clone();
    text_modes[0].take[0].reader = TokenReader::Builtin(crate::builtin::BuiltinReader::Text);
    let mut b = budget();
    let mut ledger = SourceAdmission::default();
    let accepted = AcceptedTokenizationReport::empty(quoted_scope.clone(), &mut b)?;
    let mut session =
        TokenizationSession::new("quoted".into(), &text_modes, &checked, &registry, &mut b)?;
    let reply = session.read_with_accepted(
        ScopedTokenizationRequest {
            scope: &quoted_scope,
            target: TokenTarget::Mode,
            input: TokenizationRequest {
                snapshot: &quoted,
                start: 0,
                limit: 3,
                final_input: true,
                context: &context,
                state: &NdfValue::Unit,
            },
        },
        &quoted_store,
        &mut b,
        &mut ledger,
        accepted,
    )?;
    assert!(reply.accepted.admission_scope.is_none());
    let TokenizationOutcome::Reserve { continuation, .. } = reply.outcome else {
        return Err(crate::runtime::ReaderError::Context);
    };
    let resumed = session.reserve_accepted(
        &continuation,
        &nepl3_core::source::SourceReservation {
            source_id: SourceId("decoded".into()),
            revision: 0,
            uri: "memory:decoded".into(),
        },
        &quoted_store,
        &mut b,
        &mut SourceAdmission::default(),
    )?;
    assert!(matches!(resumed.outcome, TokenizationOutcome::Token(_)));
    assert!(resumed.accepted.admission_scope.is_none());
    Ok(())
}
