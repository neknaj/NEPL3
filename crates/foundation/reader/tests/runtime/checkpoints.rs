use super::*;

#[test]
fn owned_resume_charges_frame_traversal_before_allocating_the_frame_vector()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for depth in [32u64, 128] {
        let read = signature(&schema, ProviderKind::Read);
        let mut expressions = vec![ReaderExpr::Call(read.operation.clone())];
        for id in 0..depth {
            expressions.push(ReaderExpr::Discard(ReaderId(id)));
        }
        let mut p = plan(&schema, expressions, depth, TypeDescriptor::Unit);
        p.providers.push(read);
        let checked = p.check(&registry, &mut budget())?;
        let input = source("a")?;
        let mut store = SourceStore::default();
        store.insert(input.clone())?;
        let raw = context(&schema, &registry)?;
        let context = check_context(
            &raw,
            &store,
            &registry,
            &mut budget(),
            &mut SourceAdmission::default(),
        )?;
        let execute = |remaining: Option<u64>| -> Result<_, ReaderError> {
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let mut session = ReaderSession::new("conversion".into(), &checked, &registry, &mut b)?;
            let reply = session.read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 1,
                    final_input: true,
                    context: &context,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut admission,
            )?;
            let ReadReply::Await { continuation, .. } = reply else {
                return Err(ReaderError::Context);
            };
            if let Some(remaining) = remaining {
                b.charge(Resource::Work, b.limits().work - b.usage().work - remaining)?;
            }
            let before = b.usage();
            let value = terminal("a", 1, &mut b)?;
            let reply = session.resume(&continuation, value, &store, &mut b, &mut admission)?;
            assert!(matches!(
                reply,
                ReadReply::Matched { .. }
                    | ReadReply::Stopped {
                        reason: StopReason::WorkLimit,
                        ..
                    }
            ));
            Ok((
                b.usage().work - before.work,
                b.usage().allocation_units - before.allocation_units,
                matches!(reply, ReadReply::Matched { .. }),
            ))
        };
        let full = execute(None)?;
        assert!(full.2);
        let mut previous = (0, 0);
        let mut largest_jump = (0, 0);
        // Sweep actual public resume boundaries, without a fixed implementation
        // work count or target-specific Frame size. The largest allocation step
        // is the saved frame vector: visiting it must consume at least depth Work.
        for remaining in 1..=full.0 {
            let (work, allocation, _) = execute(Some(remaining))?;
            let jump = allocation.saturating_sub(previous.1);
            if jump > largest_jump.0 {
                largest_jump = (jump, work.saturating_sub(previous.0));
            }
            assert!(work <= remaining);
            previous = (work, allocation);
        }
        assert!(
            largest_jump.1 >= depth,
            "depth={depth}, jump={largest_jump:?}"
        );
    }
    Ok(())
}

#[test]
fn lookahead_over_growing_view_keeps_prior_nodes_without_quadratic_rollback_copies()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("\u{6587}".into()),
            ReaderExpr::Node {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                body: ReaderId(0),
            },
            ReaderExpr::Look(ReaderId(1)),
            ReaderExpr::Seq(vec![ReaderId(2), ReaderId(1)]),
            ReaderExpr::Repeat {
                body: ReaderId(3),
                min: 0,
                max: 512,
            },
        ],
        4,
        TypeDescriptor::List(Box::new(TypeDescriptor::List(Box::new(
            TypeDescriptor::NdfValue,
        )))),
    );
    let mut allocations = Vec::new();
    for count in [128, 256] {
        let input = "\u{6587}".repeat(count);
        let (reply, usage) = run(&p, &registry, &input, true)?;
        let ReadReply::Matched {
            end,
            view,
            value,
            facts,
            report,
            ..
        } = reply
        else {
            return Err(ReaderError::Context);
        };
        assert_eq!(end, input.len() as u64);
        assert_eq!(view.roots.len(), count);
        assert_eq!(view.elements.len(), count);
        for (i, (root, element)) in view.roots.iter().zip(&view.elements).enumerate() {
            assert_eq!(root.0, i as u64);
            assert_eq!(
                (element.span.start(), element.span.end()),
                (i as u64 * 3, (i as u64 + 1) * 3)
            );
        }
        assert_eq!(
            value,
            NdfValue::List(vec![
                NdfValue::List(vec![NdfValue::Unit, NdfValue::Unit]);
                count
            ])
        );
        assert!(facts.is_empty() && report.diagnostics.is_empty());
        allocations.push(usage.allocation_units);
    }
    // Doubling an append-only view should not quadruple rollback storage.
    // Allow fixed overhead and allocator-independent logical accounting slack.
    eprintln!("rollback allocation units at 128/256: {allocations:?}");
    assert!(
        allocations[1] <= allocations[0] * 3,
        "allocation growth: {allocations:?}"
    );
    Ok(())
}

#[test]
fn suspended_lookahead_materializes_owned_prefixes_then_restores_the_visible_tree()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    let signature = signature(&schema, ProviderKind::Read);
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: 0,
    };
    let mut p = plan(
        &schema,
        vec![
            ReaderExpr::Literal("a".into()),
            ReaderExpr::Node {
                kind: kind.clone(),
                body: ReaderId(0),
            },
            ReaderExpr::Literal("b".into()),
            ReaderExpr::Node {
                kind,
                body: ReaderId(2),
            },
            ReaderExpr::Call(signature.operation.clone()),
            ReaderExpr::Seq(vec![ReaderId(3), ReaderId(4)]),
            ReaderExpr::Look(ReaderId(5)),
            ReaderExpr::Seq(vec![ReaderId(1), ReaderId(6), ReaderId(3)]),
        ],
        7,
        TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    );
    p.providers.push(signature);
    let checked = p.check(&registry, &mut budget())?;
    let mut b = budget();
    let mut session = ReaderSession::new("prefixes".into(), &checked, &registry, &mut b)?;
    let source = source("ab")?;
    let mut store = SourceStore::default();
    store.insert(source.clone())?;
    let mut admission = SourceAdmission::default();
    let raw = context(&schema, &registry)?;
    let context = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
    let reply = session.read(
        "entry",
        ReadRequest {
            snapshot: &source,
            start: 0,
            limit: 2,
            final_input: true,
            context: &context,
            state: &NdfValue::Unit,
        },
        &store,
        &mut b,
        &mut admission,
    )?;
    let ReadReply::Await { continuation, .. } = reply else {
        return Err(ReaderError::Context);
    };
    // Outer sequence starts empty; look and inner sequence retain a; provider retains a,b.
    assert_eq!(
        continuation
            .frames
            .iter()
            .map(|f| f.checkpoint.view.elements.len())
            .collect::<Vec<_>>(),
        vec![0, 1, 1, 2]
    );
    assert_eq!(continuation.current.view.elements.len(), 2);
    let mut forged = continuation.clone();
    forged.frames[1].checkpoint.view.elements.clear();
    let terminal = super::terminal("ok", 2, &mut b)?;
    assert!(matches!(
        session.resume(&forged, terminal, &store, &mut b, &mut admission),
        Err(ReaderError::Continuation)
    ));
    let terminal = super::terminal("ok", 2, &mut b)?;
    let reply = session.resume(&continuation, terminal, &store, &mut b, &mut admission)?;
    let ReadReply::Matched {
        view, end, value, ..
    } = reply
    else {
        return Err(ReaderError::Context);
    };
    assert_eq!(end, 2);
    assert_eq!(value, NdfValue::List(vec![NdfValue::Unit; 3]));
    assert_eq!(view.roots, vec![ViewRef(0), ViewRef(1)]);
    assert_eq!(
        view.elements
            .iter()
            .map(|v| (v.span.start(), v.span.end()))
            .collect::<Vec<_>>(),
        vec![(0, 1), (1, 2)]
    );
    Ok(())
}
