use super::*;
use alloc::format;
use nepl3_core::{
    origin::{Mapping, MappingKind},
    source::SourceId,
    value::{KindRef, OperationRef},
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 100,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(name: &str) -> Result<SourceSnapshot, ReaderError> {
    Ok(SourceSnapshot::new(
        SourceId(name.into()),
        0,
        format!("memory:{name}"),
        b"xy".to_vec(),
        &mut budget(),
    )?)
}

#[test]
fn direct_artifacts_do_not_revalidate_unrelated_accepted_maps() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor =
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let root = source("root").map_err(|e| format!("{e:?}"))?;
    let mut sources = SourceStore::default();
    sources.insert(root.clone()).map_err(|e| format!("{e:?}"))?;
    let span = root.span(0, 1).map_err(|e| format!("{e:?}"))?;
    let mut maps = Vec::new();
    for i in 0..1024 {
        let child = source(&format!("child-{i:04}")).map_err(|e| format!("{e:?}"))?;
        maps.push(Mapping {
            source: span.clone(),
            target: child.span(0, 1).map_err(|e| format!("{e:?}"))?,
            kind: MappingKind::Exact,
        });
        sources.insert(child).map_err(|e| format!("{e:?}"))?;
    }
    // Establish the same geometry/cycle invariant as the private checkpoint.
    SourceMap::validate_mappings(&maps, &sources, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let current = ReaderCheckpoint {
        cursor: 0,
        state: NdfValue::Unit,
        view: ViewBundle {
            elements: Vec::new(),
            roots: Vec::new(),
        },
        facts: Vec::new(),
        diagnostics: Vec::new(),
        events: Vec::new(),
        trace_overflow: None,
        sources: sources.snapshots().iter().skip(1).cloned().collect(),
        source_maps: maps,
    };
    let signature = crate::plan::ProviderSignature {
        operation: OperationRef {
            schema: schema.clone(),
            name: "test".into(),
        },
        kind: ProviderKind::Read,
        value_input: TypeDescriptor::Unit,
        value_output: TypeDescriptor::Unit,
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: TypeDescriptor::Unit,
    };
    let boundary = ProviderBoundary {
        signature: &signature,
        registry: &registry,
        snapshot: &root,
        declared: &[],
        current: &current,
    };
    let view = ViewBundle {
        elements: vec![ViewElement {
            kind: KindRef {
                schema,
                local_kind: 0,
            },
            span: span.clone(),
            fields: Vec::new(),
            roles: Vec::new(),
            relations: Vec::new(),
        }],
        roots: vec![ViewRef(0)],
    };
    let facts = vec![ReaderFact::Capture {
        name: "value".into(),
        span: span.clone(),
    }];
    let check = |view: &ViewBundle, facts: &[ReaderFact], maps: &[Mapping], b: &mut Budget| {
        artifacts(
            &boundary,
            &[],
            maps,
            view,
            facts,
            0,
            1,
            &sources,
            b,
            &mut SourceAdmission::default(),
        )
    };
    let mut small = Budget::new(Limits {
        work: 2000,
        ..budget().limits()
    });
    check(&view, &facts, &[], &mut small).map_err(|e| format!("{e:?}"))?;

    // Direct geometry is not proof of schema validity or valid fact metadata.
    let mut invalid_kind = view.clone();
    invalid_kind.elements[0].kind.local_kind = u64::MAX;
    assert_eq!(
        check(&invalid_kind, &[], &[], &mut budget()),
        Err(ReaderError::View(ViewError::Schema(
            SchemaError::UnknownType
        )))
    );
    let mut unknown_schema = view.elements[0].kind.schema.clone();
    unknown_schema.revision = u64::MAX;
    let invalid_facts = [
        ReaderFact::Capture {
            name: String::new(),
            span: span.clone(),
        },
        ReaderFact::Presentation {
            class: nepl3_core::view::PresentationClass {
                schema: unknown_schema.clone(),
                name: "value".into(),
                fallback: nepl3_core::view::FallbackRole::Content,
            },
            span: span.clone(),
        },
        ReaderFact::Relation {
            schema: unknown_schema,
            kind: "related".into(),
            from: span.clone(),
            to: span.clone(),
        },
    ];
    for fact in invalid_facts {
        assert_eq!(
            check(&view, &[fact], &[], &mut budget()),
            Err(ReaderError::ProviderContract)
        );
    }

    // Existing mappings remain available when any required containment is indirect.
    let mut indirect = view.clone();
    indirect.elements[0].span = current.source_maps[0].target.clone();
    check(&indirect, &[], &[], &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut nested = view.clone();
    nested.elements[0].fields.push(ViewField {
        name: "child".into(),
        children: vec![ViewRef(1)],
    });
    nested.elements.push(indirect.elements[0].clone());
    check(&nested, &[], &[], &mut budget()).map_err(|e| format!("{e:?}"))?;

    let mut cyclic = view.clone();
    cyclic.elements[0].fields.push(ViewField {
        name: "self".into(),
        children: vec![ViewRef(0)],
    });
    assert_eq!(
        check(&cyclic, &[], &[], &mut budget()),
        Err(ReaderError::View(ViewError::Cycle))
    );
    cyclic.elements[0].fields[0].children[0] = ViewRef(99);
    assert_eq!(
        check(&cyclic, &[], &[], &mut budget()),
        Err(ReaderError::View(ViewError::Reference))
    );
    let outside = vec![ReaderFact::Capture {
        name: "outside".into(),
        span: root.span(1, 2).map_err(|e| format!("{e:?}"))?,
    }];
    assert_eq!(
        check(&view, &outside, &[], &mut budget()),
        Err(ReaderError::ProviderContract)
    );
    let call = ProviderCall::Read {
        session_id: "local-artifact-test".into(),
        call_id: 0,
        depth_base: 0,
        operation: signature.operation.clone(),
        request: OwnedReadRequest {
            snapshot: root.reference(),
            sources: vec![root.clone()],
            start: 0,
            limit: 1,
            final_input: true,
            state: NdfValue::Unit,
            // Context preparation is checked by the request boundary, before this
            // response validator. This fixture exercises only response acceptance.
            context: ReaderContext {
                schema: signature.operation.schema.clone(),
                category: "Token".into(),
                mode: "test".into(),
                environment: nepl3_core::syntax::EnvironmentEntry {
                    id: 0,
                    digest: nepl3_core::source::Digest([0; 32]),
                    value: nepl3_core::syntax::Environment {
                        bindings: vec![],
                        resources: vec![],
                    },
                },
                origins: vec![],
            },
        },
    };
    let response = |facts: Vec<ReaderFact>, added: Vec<SourceSnapshot>| ReadReply::Matched {
        value: NdfValue::Unit,
        end: 1,
        new_state: NdfValue::Unit,
        view: view.clone(),
        facts,
        sources: added,
        source_maps: vec![],
        report: Report::default(),
    };
    let verify = |reply: &ReadReply, b: &mut Budget| {
        check_provider(
            &boundary,
            0,
            &call,
            ProviderReplyRef::Read(reply),
            Usage::default(),
            &sources,
            b,
            &mut SourceAdmission::default(),
        )
    };
    let mut local_budget = Budget::new(Limits {
        work: 3000,
        ..budget().limits()
    });
    verify(&response(facts.clone(), vec![]), &mut local_budget).map_err(|e| format!("{e:?}"))?;
    let presentation = ReaderFact::Presentation {
        class: nepl3_core::view::PresentationClass {
            schema: signature.operation.schema.clone(),
            name: "test".into(),
            fallback: nepl3_core::view::FallbackRole::Content,
        },
        span: root.span(1, 2).map_err(|e| format!("{e:?}"))?,
    };
    let relation = ReaderFact::Relation {
        schema: signature.operation.schema.clone(),
        kind: "test".into(),
        from: span.clone(),
        to: root.span(1, 2).map_err(|e| format!("{e:?}"))?,
    };
    // These facts are valid outside the consumed range: use the full resolver.
    verify(
        &response(vec![presentation, relation], vec![]),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let absent = source("absent").map_err(|e| format!("{e:?}"))?;
    let foreign = ReaderFact::Relation {
        schema: signature.operation.schema.clone(),
        kind: "test".into(),
        from: span.clone(),
        to: absent.span(0, 1).map_err(|e| format!("{e:?}"))?,
    };
    assert_eq!(
        verify(&response(vec![foreign], vec![]), &mut budget()),
        Err(ReaderError::Source(SourceError::MissingSnapshot))
    );
    let conflict = SourceSnapshot::new(
        SourceId("root".into()),
        0,
        "memory:root".into(),
        b"zz".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        verify(&response(facts.clone(), vec![conflict]), &mut budget()),
        Err(ReaderError::Source(SourceError::IdentityConflict))
    );
    let mut bad_report = response(facts.clone(), vec![]);
    if let ReadReply::Matched { report, .. } = &mut bad_report {
        report.usage.work = u64::MAX;
    }
    assert_eq!(
        verify(&bad_report, &mut budget()),
        Err(ReaderError::ProviderContract)
    );
    let mut bad_kind = response(facts.clone(), vec![]);
    if let ReadReply::Matched { view, .. } = &mut bad_kind {
        view.elements[0].kind.local_kind = u64::MAX;
    }
    assert_eq!(
        verify(&bad_kind, &mut budget()),
        Err(ReaderError::View(ViewError::Schema(
            SchemaError::UnknownType
        )))
    );

    // A newly supplied reverse edge must still detect a cycle with the prefix.
    let cycle = [Mapping {
        source: current.source_maps[0].target.clone(),
        target: span,
        kind: MappingKind::Exact,
    }];
    assert_eq!(
        check(&view, &facts, &cycle, &mut budget()),
        Err(ReaderError::Origin(OriginError::Cycle))
    );
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(
        check(&view, &facts, &[], &mut stopped),
        Err(ReaderError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    Ok(())
}
