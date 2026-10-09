use super::*;
use nepl3_reader::portable::PortableError;
use nepl3_wire::foundation::FoundationCodec;

#[derive(Clone, Copy)]
enum Projection {
    Continuation,
    Reply,
}
impl Projection {
    fn pending<C: nepl3_core::value_codec::FoundationValueCodec>(
        self,
        session: &ReaderSession<'_>,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<NdfValue, PortableError<C::Error>> {
        match self {
            Self::Continuation => session.pending_continuation_value(c, b),
            Self::Reply => session.pending_reply_value(c, b),
        }
    }
    fn resume<C: nepl3_core::value_codec::FoundationValueCodec>(
        self,
        session: &mut ReaderSession<'_>,
        echo: &NdfValue,
        reply: ProviderReply,
        store: &SourceStore,
        c: &mut C,
        b: &mut Budget,
    ) -> Result<ReadReply, PortableError<C::Error>> {
        match self {
            Self::Continuation => session.resume_continuation_value(echo, reply, store, c, b),
            Self::Reply => session.resume_reply_value(echo, reply, store, c, b),
        }
    }
    fn inner(self, value: &NdfValue) -> Result<&NdfValue, String> {
        match (self, value) {
            (Self::Continuation, v) => Ok(v),
            (Self::Reply, NdfValue::Variant(v)) if v.fields.len() == 3 => Ok(&v.fields[1]),
            _ => Err("Await envelope".into()),
        }
    }
    fn inner_mut(self, value: &mut NdfValue) -> Result<&mut NdfValue, String> {
        match (self, value) {
            (Self::Continuation, v) => Ok(v),
            (Self::Reply, NdfValue::Variant(v)) if v.fields.len() == 3 => Ok(&mut v.fields[1]),
            _ => Err("Await envelope".into()),
        }
    }
}

fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn complete_canonical_echo_keeps_saved_usage_and_rejects_every_changed_field() -> Result<(), String>
{
    for projection in [Projection::Continuation, Projection::Reply] {
        complete_canonical_echo_keeps_saved_usage_and_rejects_every_changed_field_case(projection)?;
    }
    Ok(())
}
fn complete_canonical_echo_keeps_saved_usage_and_rejects_every_changed_field_case(
    projection: Projection,
) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    let sig = signature(&schema, ProviderKind::Read);
    let mut p = plan(
        &schema,
        vec![ReaderExpr::Call(sig.operation.clone())],
        0,
        TypeDescriptor::Text,
    );
    p.providers.push(sig);
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let input = source("a").map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(input.clone()).map_err(err)?;
    let raw = context(&schema, &registry).map_err(err)?;
    let ctx = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut session =
        ReaderSession::new("canonical-echo".into(), &checked, &registry, &mut b).map_err(err)?;
    let reply = session
        .read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &ctx,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut a,
        )
        .map_err(err)?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err("Await".into());
    };
    let saved = continuation.usage;
    let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
    let original = projection.pending(&session, &mut c, &mut b).map_err(err)?;
    let value = nepl3_wire::decode(&nepl3_wire::encode(&original, &mut b).map_err(err)?, &mut b)
        .map_err(err)?;
    assert_eq!(value, original);
    b.charge(Resource::Work, 10).map_err(err)?;
    assert_eq!(
        projection.pending(&session, &mut c, &mut b).map_err(err)?,
        original
    );
    assert_eq!(continuation.usage, saved);
    assert_eq!(b.usage().diagnostics, saved.diagnostics);
    assert_eq!(b.usage().events, saved.events);
    if matches!(projection, Projection::Reply) {
        let NdfValue::Variant(v) = &value else {
            return Err("Await variant".into());
        };
        let NdfValue::Record(inner) = &v.fields[1] else {
            return Err("continuation record".into());
        };
        assert_eq!(v.variant, "Await");
        assert_eq!(v.type_name, "ReadReply");
        assert_eq!(v.fields[0], inner.fields[7]);
        assert_eq!(v.fields[2], inner.fields[9]);
        for field in 0..3 {
            let mut wrong = value.clone();
            let NdfValue::Variant(v) = &mut wrong else {
                return Err("Await variant".into());
            };
            v.fields[field] = NdfValue::Unit;
            let provider = terminal("a", 1, &mut b).map_err(err)?;
            assert!(matches!(
                projection.resume(&mut session, &wrong, provider, &store, &mut c, &mut b),
                Err(PortableError::Reader(ReaderError::Continuation))
            ));
            assert!(session.pending_read().is_ok());
        }
        // Valid outer call shape but a different correlation ID cannot override
        // the call nested in the saved continuation.
        let mut wrong = value.clone();
        let NdfValue::Variant(v) = &mut wrong else {
            return Err("Await variant".into());
        };
        let NdfValue::Variant(call) = &mut v.fields[0] else {
            return Err("call variant".into());
        };
        call.fields[1] = NdfValue::U64(u64::MAX);
        let provider = terminal("a", 1, &mut b).map_err(err)?;
        assert!(matches!(
            projection.resume(&mut session, &wrong, provider, &store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        assert!(session.pending_read().is_ok());
        for shape in 0..5 {
            let mut wrong = value.clone();
            let NdfValue::Variant(v) = &mut wrong else {
                return Err("Await variant".into());
            };
            match shape {
                0 => v.schema.package = "foreign-reader".into(),
                1 => v.type_name = "OtherReply".into(),
                2 => v.variant = "Stopped".into(),
                3 => {
                    v.fields.pop();
                }
                _ => v.fields[1] = NdfValue::Unit,
            }
            let mut cancelled = Budget::new(b.limits());
            cancelled.record_observed_usage(b.usage()).map_err(err)?;
            cancelled.cancel();
            let provider = terminal("a", 1, &mut budget()).map_err(err)?;
            assert!(matches!(
                projection.resume(
                    &mut session,
                    &wrong,
                    provider,
                    &store,
                    &mut c,
                    &mut cancelled
                ),
                Err(PortableError::Reader(ReaderError::Continuation))
            ));
            assert!(session.pending_read().is_ok());
        }
    }
    for field in 0..10 {
        let mut wrong = value.clone();
        let NdfValue::Record(r) = projection.inner_mut(&mut wrong)? else {
            return Err("record".into());
        };
        r.fields[field] = NdfValue::Unit;
        let provider = terminal("a", 1, &mut b).map_err(err)?;
        assert!(matches!(
            projection.resume(&mut session, &wrong, provider, &store, &mut c, &mut b),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        assert!(session.pending_read().is_ok());
    }
    // Preserve all observed history but deliberately change the operation's
    // original ceiling: a supplied replacement Budget cannot authorize resume.
    let mut changed = Budget::new(Limits {
        work: b.limits().work + 1,
        ..b.limits()
    });
    changed.record_observed_usage(b.usage()).map_err(err)?;
    let provider = terminal("a", 1, &mut b).map_err(err)?;
    assert!(matches!(
        projection.resume(&mut session, &value, provider, &store, &mut c, &mut changed),
        Err(PortableError::Reader(ReaderError::Continuation))
    ));
    assert!(session.pending_read().is_ok());
    let provider = terminal("a", 1, &mut b).map_err(err)?;
    let resumed = projection
        .resume(&mut session, &value, provider, &store, &mut c, &mut b)
        .map_err(err)?;
    assert!(matches!(resumed, ReadReply::Matched { end: 1, .. }));
    assert!(matches!(
        session.pending_read(),
        Err(ReaderError::NoPending)
    ));
    let provider = terminal("a", 1, &mut b).map_err(err)?;
    assert!(matches!(
        projection.resume(&mut session, &value, provider, &store, &mut c, &mut b),
        Err(PortableError::Reader(ReaderError::NoPending))
    ));
    session.close();
    assert!(matches!(
        projection.pending(&session, &mut c, &mut b),
        Err(PortableError::Reader(ReaderError::Closed))
    ));
    let provider = terminal("a", 1, &mut b).map_err(err)?;
    assert!(matches!(
        projection.resume(&mut session, &value, provider, &store, &mut c, &mut b),
        Err(PortableError::Reader(ReaderError::Closed))
    ));
    Ok(())
}

#[test]
fn echo_stops_preserve_collectors_and_foreign_session_cannot_consume_pending() -> Result<(), String>
{
    for projection in [Projection::Continuation, Projection::Reply] {
        echo_stops_preserve_collectors_and_foreign_session_cannot_consume_pending_case(projection)?;
    }
    Ok(())
}
fn echo_stops_preserve_collectors_and_foreign_session_cannot_consume_pending_case(
    projection: Projection,
) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    let read = signature(&schema, ProviderKind::Read);
    let mut p = plan(
        &schema,
        vec![
            ReaderExpr::Call(read.operation.clone()),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]),
        ],
        1,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    p.providers.push(read);
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let input = source("ab").map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(input.clone()).map_err(err)?;
    let raw = context(&schema, &registry).map_err(err)?;
    let ctx = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    #[derive(Clone, Copy)]
    enum StopCase {
        Cancel,
        Work(u64),
        ComparisonMetadata,
        Allocation,
        Nodes,
        Depth,
        WireOutput,
    }
    for case in [
        StopCase::Cancel,
        StopCase::Work(0),
        StopCase::Work(1),
        StopCase::Work(100),
        StopCase::ComparisonMetadata,
        StopCase::Allocation,
        StopCase::Nodes,
        StopCase::Depth,
        StopCase::WireOutput,
    ] {
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session = ReaderSession::new("kept-collector".into(), &checked, &registry, &mut b)
            .map_err(err)?;
        let wait = session
            .read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 2,
                    final_input: true,
                    context: &ctx,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut a,
            )
            .map_err(err)?;
        let ReadReply::Await { continuation, .. } = wait else {
            return Err("first Await".into());
        };
        let prefix =
            annotated_terminal(&schema, input.span(0, 1).map_err(err)?, 1, &mut b).map_err(err)?;
        let wait = session
            .resume(&continuation, prefix, &store, &mut b, &mut a)
            .map_err(err)?;
        let ReadReply::Await { report: saved, .. } = wait else {
            return Err("second Await".into());
        };
        assert_eq!(saved.diagnostics.len(), 1);
        assert_eq!(saved.events.len(), 1);
        let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
        let mut echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        // A failed export leaves the original slot intact, regardless of its
        // separate explicit encoding budget's exhaustion.
        let mut tiny = Budget::new(Limits {
            allocation_units: 0,
            ..budget().limits()
        });
        assert!(projection.pending(&session, &mut c, &mut tiny).is_err());
        assert!(session.pending_read().is_ok());
        let provider = terminal("b", 2, &mut b).map_err(err)?;
        let mut fresh = budget();
        assert!(matches!(
            projection.resume(&mut session, &echo, provider, &store, &mut c, &mut fresh),
            Err(PortableError::Reader(ReaderError::Continuation))
        ));
        assert!(session.pending_read().is_ok());
        let (preparation_work, comparison_work) = if matches!(case, StopCase::ComparisonMetadata) {
            let before = b.usage().work;
            let _ = projection.pending(&session, &mut c, &mut b).map_err(err)?;
            let work = b.usage().work - before;
            let before_compare = b.usage().work;
            assert!(echo.equal_with_budget(&echo, &mut b).map_err(err)?);
            let comparison = b.usage().work - before_compare;
            let NdfValue::Record(root) = projection.inner_mut(&mut echo)? else {
                return Err("echo".into());
            };
            let NdfValue::Record(report) = &mut root.fields[9] else {
                return Err("report".into());
            };
            report.schema.package = "adversarial-metadata".repeat(10_000);
            (work, comparison)
        } else {
            (0, 0)
        };
        let provider = terminal("b", 2, &mut b).map_err(err)?;
        let reason = match case {
            StopCase::Work(n) => {
                b.charge(Resource::Work, b.limits().work - b.usage().work - n)
                    .map_err(err)?;
                StopReason::WorkLimit
            }
            StopCase::ComparisonMetadata => {
                b.charge(
                    Resource::Work,
                    b.limits().work - b.usage().work - preparation_work - comparison_work - 200,
                )
                .map_err(err)?;
                StopReason::WorkLimit
            }
            StopCase::Cancel => {
                b.cancel();
                StopReason::Cancelled
            }
            StopCase::Allocation => {
                b.charge(
                    Resource::AllocationUnits,
                    b.limits().allocation_units - b.usage().allocation_units,
                )
                .map_err(err)?;
                StopReason::AllocationLimit
            }
            StopCase::Nodes => {
                b.charge(Resource::Nodes, b.limits().nodes - b.usage().nodes)
                    .map_err(err)?;
                StopReason::NodeLimit
            }
            StopCase::Depth => StopReason::DepthLimit,
            StopCase::WireOutput => {
                b.charge(
                    Resource::OutputBytes,
                    b.limits().output_bytes - b.usage().output_bytes,
                )
                .map_err(err)?;
                assert!(nepl3_wire::encode(&echo, &mut b).is_err());
                assert!(session.pending_read().is_ok());
                StopReason::OutputLimit
            }
        };
        if matches!(case, StopCase::Cancel) {
            let mut foreign = echo.clone();
            let NdfValue::Record(r) = projection.inner_mut(&mut foreign)? else {
                return Err("record".into());
            };
            r.fields[0] = NdfValue::Text("other-session".into());
            assert!(matches!(
                projection.resume(
                    &mut session,
                    &foreign,
                    terminal("foreign", 2, &mut budget()).map_err(err)?,
                    &store,
                    &mut c,
                    &mut b
                ),
                Err(PortableError::Reader(ReaderError::Continuation))
            ));
            assert!(session.pending_read().is_ok());
        }
        let before_resume = b.usage().work;
        let stopped = if matches!(case, StopCase::Depth) {
            b.with_depth_at_least(b.limits().depth, |b| {
                projection.resume(&mut session, &echo, provider, &store, &mut c, b)
            })
            .map_err(err)?
        } else {
            projection
                .resume(&mut session, &echo, provider, &store, &mut c, &mut b)
                .map_err(err)?
        };
        let ReadReply::Stopped {
            reason: actual,
            report,
            ..
        } = stopped
        else {
            return Err("Stopped".into());
        };
        assert_eq!(actual, reason);
        if matches!(case, StopCase::ComparisonMetadata) {
            assert!(b.usage().work - before_resume > preparation_work);
        }
        assert_eq!(report.diagnostics, saved.diagnostics);
        assert_eq!(report.events, saved.events);
        assert!(matches!(
            session.pending_read(),
            Err(ReaderError::NoPending)
        ));
        assert_eq!(b.poll(), Err(reason));
    }
    Ok(())
}

#[test]
fn canonical_echo_handles_read_transform_and_dependent_pending_calls() -> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        canonical_echo_handles_read_transform_and_dependent_pending_calls_case(projection)?;
    }
    Ok(())
}
fn canonical_echo_handles_read_transform_and_dependent_pending_calls_case(
    projection: Projection,
) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    for kind in [
        ProviderKind::Read,
        ProviderKind::Transform,
        ProviderKind::Dependent,
    ] {
        let sig = signature(&schema, kind);
        let root = match kind {
            ProviderKind::Read => ReaderExpr::Call(sig.operation.clone()),
            ProviderKind::Transform => ReaderExpr::Map {
                provider: sig.operation.clone(),
                body: ReaderId(0),
            },
            ProviderKind::Dependent => ReaderExpr::Then {
                provider: sig.operation.clone(),
                first: ReaderId(0),
            },
        };
        let mut p = plan(
            &schema,
            vec![ReaderExpr::Scalar(CharClass::Any), root],
            1,
            TypeDescriptor::Text,
        );
        p.providers.push(sig);
        let checked = p.check(&registry, &mut budget()).map_err(err)?;
        let input = source("ab").map_err(err)?;
        let mut store = SourceStore::default();
        store.insert(input.clone()).map_err(err)?;
        let raw = context(&schema, &registry).map_err(err)?;
        let ctx = check_context(
            &raw,
            &store,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default(),
        )
        .map_err(err)?;
        let mut b = budget();
        let mut a = SourceAdmission::default();
        let mut session =
            ReaderSession::new("three-calls".into(), &checked, &registry, &mut b).map_err(err)?;
        let wait = session
            .read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 2,
                    final_input: true,
                    context: &ctx,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut a,
            )
            .map_err(err)?;
        assert!(matches!(wait, ReadReply::Await { .. }));
        let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
        let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
        let bytes = nepl3_wire::encode(&echo, &mut b).map_err(err)?;
        let echo = nepl3_wire::decode(&bytes, &mut b).map_err(err)?;
        let provider = if kind == ProviderKind::Transform {
            ProviderReply::Transform(Box::new(TransformReply {
                outcome: TransformOutcome::Complete {
                    value: NdfValue::Text("mapped".into()),
                    view: ViewBundle {
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
            }))
        } else {
            terminal("read", 2, &mut b).map_err(err)?
        };
        let resumed = projection
            .resume(&mut session, &echo, provider, &store, &mut c, &mut b)
            .map_err(err)?;
        assert!(matches!(resumed, ReadReply::Matched { .. }));
    }
    Ok(())
}

#[test]
fn repeated_identical_generated_sources_have_one_canonical_declaration() -> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        source_declarations_case(true, projection)?;
    }
    Ok(())
}
#[test]
fn canonical_source_order_does_not_reorder_native_collectors() -> Result<(), String> {
    for projection in [Projection::Continuation, Projection::Reply] {
        source_declarations_case(false, projection)?;
    }
    Ok(())
}
fn source_declarations_case(repeated: bool, projection: Projection) -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    let sig = signature(&schema, ProviderKind::Read);
    let mut p = plan(
        &schema,
        vec![
            ReaderExpr::Call(sig.operation.clone()),
            ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0), ReaderId(0)]),
        ],
        1,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    p.providers.push(sig);
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let input = source("abc").map_err(err)?;
    let mut store = SourceStore::default();
    store.insert(input.clone()).map_err(err)?;
    let raw = context(&schema, &registry).map_err(err)?;
    let ctx = check_context(
        &raw,
        &store,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut b = budget();
    let mut a = SourceAdmission::default();
    let mut session =
        ReaderSession::new("duplicate-declaration".into(), &checked, &registry, &mut b)
            .map_err(err)?;
    let mut wait = session
        .read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 3,
                final_input: true,
                context: &ctx,
                state: &NdfValue::Unit,
            },
            &store,
            &mut b,
            &mut a,
        )
        .map_err(err)?;
    let generated = a
        .create(
            SourceId("z-generated".into()),
            0,
            "memory:z-generated".into(),
            b"x".to_vec(),
            &mut b,
        )
        .map_err(err)?;
    let second = if repeated {
        generated.clone()
    } else {
        a.create(
            SourceId("a-generated".into()),
            0,
            "memory:a-generated".into(),
            b"y".to_vec(),
            &mut b,
        )
        .map_err(err)?
    };
    for end in 1..=2 {
        let ReadReply::Await { continuation, .. } = wait else {
            return Err("prefix Await".into());
        };
        let mut provider = terminal("a", end, &mut b).map_err(err)?;
        let ProviderReply::Read(reply) = &mut provider else {
            return Err("Read".into());
        };
        let ReadReply::Matched { sources, .. } = reply.as_mut() else {
            return Err("Matched".into());
        };
        sources.push(if end == 1 {
            generated.clone()
        } else {
            second.clone()
        });
        wait = session
            .resume(&continuation, provider, &store, &mut b, &mut a)
            .map_err(err)?;
    }
    let ReadReply::Await { continuation, .. } = wait else {
        return Err("third Await".into());
    };
    assert_eq!(continuation.current.sources.len(), 2);
    assert_eq!(
        continuation.current.sources,
        vec![generated.clone(), second.clone()]
    );
    let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(err)?;
    let echo = projection.pending(&session, &mut c, &mut b).map_err(err)?;
    let NdfValue::Record(r) = projection.inner(&echo)? else {
        return Err("continuation record".into());
    };
    let NdfValue::Record(cp) = &r.fields[6] else {
        return Err("checkpoint record".into());
    };
    let NdfValue::List(declared) = &cp.fields[5] else {
        return Err("source list".into());
    };
    assert_eq!(declared.len(), if repeated { 1 } else { 2 });
    let mut wrong = echo.clone();
    let NdfValue::Record(r) = projection.inner_mut(&mut wrong)? else {
        return Err("record".into());
    };
    let NdfValue::Record(cp) = &mut r.fields[6] else {
        return Err("checkpoint".into());
    };
    let NdfValue::List(declared) = &mut cp.fields[5] else {
        return Err("declarations".into());
    };
    if repeated {
        declared.push(declared[0].clone());
    } else {
        declared.reverse();
    }
    let provider = terminal("c", 3, &mut b).map_err(err)?;
    assert!(matches!(
        projection.resume(&mut session, &wrong, provider, &store, &mut c, &mut b),
        Err(PortableError::Reader(ReaderError::Continuation))
    ));
    assert!(session.pending_read().is_ok());
    let provider = terminal("c", 3, &mut b).map_err(err)?;
    let resumed = projection
        .resume(&mut session, &echo, provider, &store, &mut c, &mut b)
        .map_err(err)?;
    let ReadReply::Matched { sources, .. } = resumed else {
        return Err("final Matched".into());
    };
    // Canonical transport does not rewrite the private native collector.
    assert_eq!(sources, vec![generated, second]);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resume_route(
    session: &mut ReaderSession<'_>,
    continuation: &ReaderContinuation,
    reply: ProviderReply,
    store: &SourceStore,
    registry: &SchemaRegistry,
    b: &mut Budget,
    a: &mut SourceAdmission,
    portable: u8,
) -> Result<ReadReply, String> {
    if portable == 0 {
        return session
            .resume(continuation, reply, store, b, a)
            .map_err(err);
    }
    let projection = if portable == 1 {
        Projection::Continuation
    } else {
        Projection::Reply
    };
    let mut c = FoundationCodec::new(registry, store, a).map_err(err)?;
    let echo = projection.pending(session, &mut c, b).map_err(err)?;
    let bytes = nepl3_wire::encode(&echo, b).map_err(err)?;
    let echo = nepl3_wire::decode(&bytes, b).map_err(err)?;
    projection
        .resume(session, &echo, reply, store, &mut c, b)
        .map_err(err)
}

#[test]
fn echo_preserves_repeat_look_choice_rollback_and_mapping_scopes() -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let (registry, schema) = registry().map_err(err)?;
    for portable in [0, 1, 2] {
        for mode in 0..3 {
            let sig = signature(&schema, ProviderKind::Read);
            let (expressions, root, output) = match mode {
                0 => (
                    vec![
                        ReaderExpr::Call(sig.operation.clone()),
                        ReaderExpr::Many(ReaderId(0)),
                    ],
                    1,
                    TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
                ),
                1 => (
                    vec![
                        ReaderExpr::Call(sig.operation.clone()),
                        ReaderExpr::Look(ReaderId(0)),
                    ],
                    1,
                    TypeDescriptor::Unit,
                ),
                _ => (
                    vec![
                        ReaderExpr::Call(sig.operation.clone()),
                        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]),
                        ReaderExpr::Scalar(CharClass::Any),
                        ReaderExpr::Seq(vec![ReaderId(2)]),
                        ReaderExpr::Choice(vec![ReaderId(1), ReaderId(3)]),
                    ],
                    4,
                    TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
                ),
            };
            let mut p = plan(&schema, expressions, root, output);
            p.providers.push(sig);
            let checked = p.check(&registry, &mut budget()).map_err(err)?;
            let input = source("ab").map_err(err)?;
            let mut store = SourceStore::default();
            store.insert(input.clone()).map_err(err)?;
            let raw = context(&schema, &registry).map_err(err)?;
            let ctx = check_context(
                &raw,
                &store,
                &registry,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
            let mut b = budget();
            let mut a = SourceAdmission::default();
            let mut session =
                ReaderSession::new("rollback-echo".into(), &checked, &registry, &mut b)
                    .map_err(err)?;
            let wait = session
                .read(
                    "entry",
                    ReadRequest {
                        snapshot: &input,
                        start: 0,
                        limit: 2,
                        final_input: true,
                        context: &ctx,
                        state: &NdfValue::Unit,
                    },
                    &store,
                    &mut b,
                    &mut a,
                )
                .map_err(err)?;
            let ReadReply::Await { continuation, .. } = wait else {
                return Err("first Await".into());
            };
            let generated = a
                .create(
                    SourceId("mapped-echo".into()),
                    0,
                    "memory:mapped-echo".into(),
                    b"x".to_vec(),
                    &mut b,
                )
                .map_err(err)?;
            let span = generated.span(0, 1).map_err(err)?;
            let mut provider = annotated_terminal(&schema, span.clone(), 1, &mut b).map_err(err)?;
            let ProviderReply::Read(reply) = &mut provider else {
                return Err("Read".into());
            };
            let ReadReply::Matched {
                sources,
                source_maps,
                view,
                ..
            } = reply.as_mut()
            else {
                return Err("Matched".into());
            };
            sources.push(generated);
            source_maps.push(Mapping {
                source: input.span(0, 1).map_err(err)?,
                target: span.clone(),
                kind: MappingKind::Transformed,
            });
            view.elements.push(ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span: input.span(0, 1).map_err(err)?,
                fields: vec![ViewField {
                    name: "mapped-child".into(),
                    children: vec![ViewRef(1)],
                }],
                roles: vec![],
                relations: vec![],
            });
            view.elements.push(ViewElement {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                span,
                fields: vec![],
                roles: vec![],
                relations: vec![],
            });
            view.roots.push(ViewRef(0));
            let mut result = resume_route(
                &mut session,
                &continuation,
                provider,
                &store,
                &registry,
                &mut b,
                &mut a,
                portable,
            )?;
            if mode != 1 {
                let ReadReply::Await { continuation, .. } = result else {
                    return Err("second Await".into());
                };
                assert_eq!(continuation.current.sources.len(), 1);
                assert_eq!(continuation.current.source_maps.len(), 1);
                assert_eq!(continuation.current.view.elements.len(), 2);
                assert!(
                    continuation
                        .frames
                        .iter()
                        .any(|f| f.checkpoint.sources.is_empty())
                );
                let provider = ProviderReply::Read(Box::new(ReadReply::NoMatch {
                    expected: vec![Expectation::EndOfInput],
                    furthest: 1,
                    sources: vec![],
                    source_maps: vec![],
                    report: Report {
                        usage: b.usage(),
                        ..Report::default()
                    },
                }));
                result = resume_route(
                    &mut session,
                    &continuation,
                    provider,
                    &store,
                    &registry,
                    &mut b,
                    &mut a,
                    portable,
                )?;
            }
            let ReadReply::Matched {
                value,
                end,
                view,
                sources,
                source_maps,
                report,
                ..
            } = &result
            else {
                return Err("final Matched".into());
            };
            assert_eq!(*end, if mode == 1 { 0 } else { 1 });
            assert_eq!(
                value,
                &if mode == 1 {
                    NdfValue::Unit
                } else {
                    NdfValue::List(vec![NdfValue::Text("a".into())])
                }
            );
            let retained = usize::from(mode == 0);
            assert_eq!(sources.len(), retained);
            assert_eq!(source_maps.len(), retained);
            assert_eq!(view.elements.len(), retained * 2);
            assert_eq!(report.diagnostics.len(), retained);
            assert_eq!(report.events.len(), retained);
            assert_eq!(b.usage().diagnostics, 1);
            assert_eq!(b.usage().events, 1);
        }
    }
    Ok(())
}
