use nepl3_core::{budget::*, schema::*, source::*, value::*, view::*};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 1000,
        depth: 100,
        source_bytes: 1000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
fn fixture(
    padding: usize,
) -> Result<(SchemaRegistry, SchemaRef, SchemaRef, SourceStore, Span), ViewError> {
    let mut registry = SchemaRegistry::default();
    let d = |package: String, typed: bool| SchemaDescriptor {
        package,
        revision: 1,
        types: if typed {
            vec![NamedType {
                name: "Node".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            }]
        } else {
            vec![]
        },
        operations: vec![],
    };
    let base = d("base".into(), true);
    let base_ref = base.reference(&mut budget())?;
    registry.register(base_ref.clone(), base, &mut budget())?;
    for i in 0..padding {
        let p = d(format!("padding.{i:02}"), false);
        registry.register(p.reference(&mut budget())?, p, &mut budget())?;
    }
    let owner = d("owner".into(), true);
    let owner_ref = owner.reference(&mut budget())?;
    registry.register(owner_ref.clone(), owner, &mut budget())?;
    registry.finalize(&mut budget())?;
    let source = SourceSnapshot::new(
        SourceId("source".into()),
        0,
        "memory:source".into(),
        b"x".to_vec(),
        &mut budget(),
    )?;
    let span = source.span(0, 1)?;
    let mut sources = SourceStore::default();
    sources.insert(source)?;
    Ok((registry, base_ref, owner_ref, sources, span))
}
fn validate(
    case: u8,
    base: &SchemaRef,
    owner: &SchemaRef,
    span: &Span,
    sources: &SourceStore,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), ViewError> {
    if case == 1 {
        return Token {
            kind: KindRef {
                schema: owner.clone(),
                local_kind: 0,
            },
            head: span.clone(),
            payload: NdfValue::Unit,
            views: ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            leading_trivia: vec![],
        }
        .validate(sources, registry, b);
    }
    let element = ViewElement {
        kind: KindRef {
            schema: if case == 0 {
                owner.clone()
            } else {
                base.clone()
            },
            local_kind: 0,
        },
        span: span.clone(),
        fields: vec![],
        roles: if case == 2 {
            vec![PresentationClass {
                schema: owner.clone(),
                name: "arbitrary-vocabulary".into(),
                fallback: FallbackRole::Content,
            }]
        } else {
            vec![]
        },
        relations: if case == 3 {
            vec![ViewRelation {
                schema: owner.clone(),
                kind: "arbitrary-vocabulary".into(),
                target: ViewRef(0),
            }]
        } else {
            vec![]
        },
    };
    ViewBundle {
        elements: vec![element],
        roots: vec![ViewRef(0)],
    }
    .validate(sources, registry, b)
}
#[test]
fn view_and_token_schema_lookups_meter_only_the_targeted_catalog_growth() -> Result<(), ViewError> {
    for case in 0..4 {
        let mut usages = vec![];
        for padding in [0, 32] {
            let (registry, base, owner, sources, span) = fixture(padding)?;
            let mut b = budget();
            validate(case, &base, &owner, &span, &sources, &registry, &mut b)?;
            usages.push(b.usage());
        }
        let mut expected = usages[0];
        expected.work += 32 * (10 + 5 + 9);
        assert_eq!(usages[1], expected, "case {case}");
    }
    Ok(())
}

#[test]
fn view_schema_errors_and_lookup_stops_keep_their_priority() -> Result<(), ViewError> {
    let (registry, base, owner, sources, span) = fixture(32)?;
    for case in 0..4 {
        for mutation in 0..3 {
            let mut invalid = owner.clone();
            match mutation {
                0 => invalid.package = "missing".into(),
                1 => invalid.revision += 1,
                _ => invalid.digest.0[0] ^= 1,
            }
            let mut b = budget();
            let expected = if case < 2 {
                ViewError::Schema(SchemaError::UnknownSchema)
            } else {
                ViewError::Presentation
            };
            assert_eq!(
                validate(case, &base, &invalid, &span, &sources, &registry, &mut b),
                Err(expected)
            );
            if mutation == 0 {
                let lookup_work = 1 + (4 + 7 + 9) + 32 * (10 + 7 + 9) + (5 + 7 + 9);
                let prefix = b.usage().work - lookup_work;
                let mut stopped = Budget::new(Limits {
                    work: prefix,
                    ..budget().limits()
                });
                assert_eq!(
                    validate(
                        case,
                        &base,
                        &invalid,
                        &span,
                        &sources,
                        &registry,
                        &mut stopped
                    ),
                    Err(ViewError::Stopped(StopReason::WorkLimit))
                );
                assert_eq!(stopped.usage().work, prefix);
                assert_eq!(stopped.usage().allocation_units, b.usage().allocation_units);
                let before = stopped.usage();
                assert_eq!(
                    validate(
                        case,
                        &base,
                        &invalid,
                        &span,
                        &sources,
                        &registry,
                        &mut stopped
                    ),
                    Err(ViewError::Stopped(StopReason::WorkLimit))
                );
                assert_eq!(stopped.usage(), before);
            }
        }
        let mut cancelled = budget();
        cancelled.charge(Resource::Work, 7)?;
        cancelled.cancel();
        let before = cancelled.usage();
        assert_eq!(
            validate(
                case,
                &base,
                &owner,
                &span,
                &sources,
                &registry,
                &mut cancelled
            ),
            Err(ViewError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(cancelled.usage(), before);
    }
    let kind = KindRef {
        schema: owner.clone(),
        local_kind: u64::MAX,
    };
    let view = ViewBundle {
        elements: vec![ViewElement {
            kind: kind.clone(),
            span: span.clone(),
            fields: vec![],
            roles: vec![],
            relations: vec![],
        }],
        roots: vec![ViewRef(0)],
    };
    assert_eq!(
        view.validate(&sources, &registry, &mut budget()),
        Err(ViewError::Schema(SchemaError::UnknownType))
    );
    let token = Token {
        kind,
        head: span.clone(),
        payload: NdfValue::Unit,
        views: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        leading_trivia: vec![],
    };
    assert_eq!(
        token.validate(&sources, &registry, &mut budget()),
        Err(ViewError::Schema(SchemaError::UnknownType))
    );
    let unfinalized = SchemaRegistry::default();
    for case in [0, 1] {
        assert_eq!(
            validate(
                case,
                &base,
                &owner,
                &span,
                &sources,
                &unfinalized,
                &mut budget()
            ),
            Err(ViewError::Schema(SchemaError::Unfinalized))
        );
    }
    // An empty view does not inspect schema kinds and gains no global
    // finalization requirement from the budgeted kind helper.
    ViewBundle {
        elements: vec![],
        roots: vec![],
    }
    .validate(&sources, &unfinalized, &mut budget())?;
    Ok(())
}

#[test]
fn empty_vocabulary_and_invalid_relation_target_precede_schema_lookup() -> Result<(), ViewError> {
    let (registry, base, mut missing, sources, span) = fixture(0)?;
    missing.package = "missing".into();
    for relation in [false, true] {
        let mut view = ViewBundle {
            elements: vec![ViewElement {
                kind: KindRef {
                    schema: base.clone(),
                    local_kind: 0,
                },
                span: span.clone(),
                fields: vec![],
                roles: vec![],
                relations: vec![],
            }],
            roots: vec![ViewRef(0)],
        };
        if relation {
            view.elements[0].relations.push(ViewRelation {
                schema: missing.clone(),
                kind: String::new(),
                target: ViewRef(0),
            });
        } else {
            view.elements[0].roles.push(PresentationClass {
                schema: missing.clone(),
                name: String::new(),
                fallback: FallbackRole::Content,
            });
        }
        let mut b = budget();
        assert_eq!(
            view.validate(&sources, &registry, &mut b),
            Err(ViewError::Presentation)
        );
        let prefix = b.usage().work;
        if relation {
            view.elements[0].relations[0].kind = "any".into();
        } else {
            view.elements[0].roles[0].name = "any".into();
        }
        let mut stopped = Budget::new(Limits {
            work: prefix,
            ..budget().limits()
        });
        assert_eq!(
            view.validate(&sources, &registry, &mut stopped),
            Err(ViewError::Stopped(StopReason::WorkLimit))
        );
        if relation {
            view.elements[0].relations[0].target = ViewRef(1);
            assert_eq!(
                view.validate(&sources, &registry, &mut budget()),
                Err(ViewError::Reference)
            );
        }
    }
    Ok(())
}
