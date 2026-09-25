use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    origin::{Origin, OriginId},
    schema::*,
    source::SourceAdmission,
    syntax::*,
    value::SchemaRef,
};
use nepl3_sentence_core::{lower, model::*};
fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        depth: 100_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        source_bytes: 2_000_000,
        output_bytes: 2_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

fn check_closure_entry(
    bundle: &SyntaxBundle,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    forms: &[lower::ForeignInlineForm<'_>],
    expected: &nepl3_sentence_core::syntax::SentenceSyntax,
) -> Result<(), String> {
    use lower::presentation::{ClosureLowerer, Error};
    use nepl3_core::source::SourceStore;
    use nepl3_wire::foundation::FoundationCodec;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(
        &environment,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or("foundation")?,
        registry,
        &mut b(),
    )
    .map_err(err)?;
    let closure = ForeignClosure {
        syntax: ForeignSyntax {
            schema: surface.clone(),
            category: match expected.value.root {
                Root::Sentence(_) => "Sentence",
                Root::Inline(_) => "Inline",
            }
            .into(),
            root: bundle.root,
            bundle: bundle.clone(),
            environment: EnvironmentRef { id: 0, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        provenance: OwnerProvenance::from_parts(vec![], vec![], vec![]),
    };
    let store = SourceStore::default();
    let run = |limits| {
        let mut budget = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let output =
            ClosureLowerer::new(registry).lower(&closure, surface, forms, &mut codec, &mut budget);
        Ok::<_, String>((output, budget))
    };
    let (output, measured) = run(b().limits())?;
    assert_eq!(output.map_err(err)?, *expected);
    let proof = bundle
        .validate_in_registry(registry, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let run_proof = |limits| {
        let mut budget = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let output = lower::presentation::sentence_in_registry(
            &proof,
            surface,
            forms,
            registry,
            &mut codec,
            &mut budget,
        );
        Ok::<_, String>((output, budget))
    };
    let (output, proof_usage) = run_proof(b().limits())?;
    assert_eq!(output.map_err(err)?, *expected);
    for resource in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::SourceLimit,
    ] {
        let used = match resource {
            StopReason::WorkLimit => proof_usage.usage().work,
            StopReason::AllocationLimit => proof_usage.usage().allocation_units,
            StopReason::DepthLimit => proof_usage.usage().depth,
            StopReason::SourceLimit => proof_usage.usage().source_bytes,
            _ => return Err("resource".into()),
        };
        for short in [false, true] {
            if short && used == 0 {
                continue;
            }
            let limit = used - u64::from(short);
            let mut limits = b().limits();
            match resource {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::DepthLimit => limits.depth = limit,
                StopReason::SourceLimit => limits.source_bytes = limit,
                _ => return Err("resource".into()),
            }
            let (output, budget) = run_proof(limits)?;
            if short {
                assert_eq!(output.err(), Some(Error::Stopped(resource)));
                assert_eq!(budget.poll(), Err(resource));
            } else {
                assert_eq!(output.map_err(err)?, *expected);
            }
        }
    }
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
    ] {
        for short in [false, true] {
            let mut limits = b().limits();
            match reason {
                StopReason::WorkLimit => {
                    limits.work = measured.usage().work - u64::from(short);
                }
                StopReason::AllocationLimit => {
                    limits.allocation_units = measured.usage().allocation_units - u64::from(short);
                }
                StopReason::DepthLimit => {
                    limits.depth = measured.usage().depth - u64::from(short);
                }
                _ => return Err("resource".into()),
            }
            let (output, budget) = run(limits)?;
            assert_eq!(budget.current_depth(), 0);
            if short {
                assert_eq!(output.err(), Some(Error::Stopped(reason)));
            } else {
                assert_eq!(output.map_err(err)?, *expected);
            }
        }
    }
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut lowerer = ClosureLowerer::new(registry);
    assert_eq!(
        lowerer
            .lower(&closure, surface, forms, &mut codec, &mut b())
            .map_err(err)?,
        *expected
    );
    let mut limited = b().limits();
    limited.depth = 0;
    assert_eq!(
        lowerer
            .lower(
                &closure,
                surface,
                forms,
                &mut codec,
                &mut Budget::new(limited)
            )
            .err(),
        Some(Error::Stopped(StopReason::DepthLimit))
    );
    let mut nested_limits = b().limits();
    nested_limits.depth = measured.usage().depth;
    let mut nested = Budget::new(nested_limits);
    let stopped = nested.with_depth_at_least(1, |budget| {
        lowerer.lower(&closure, surface, forms, &mut codec, budget)
    });
    assert_eq!(stopped.err(), Some(Error::Stopped(StopReason::DepthLimit)));
    assert_eq!(nested.current_depth(), 0);
    if !bundle.sources.is_empty() {
        // Reused owner proof does not grant sources to a fresh admission.
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
        let mut limits = b().limits();
        limits.source_bytes = 0;
        assert_eq!(
            lowerer
                .lower(
                    &closure,
                    surface,
                    forms,
                    &mut codec,
                    &mut Budget::new(limits)
                )
                .err(),
            Some(Error::Stopped(StopReason::SourceLimit))
        );
    }
    let mut wrong = closure.clone();
    let mut other_surface = surface.clone();
    other_surface.revision += 1;
    assert_eq!(
        ClosureLowerer::new(registry)
            .lower(&closure, &other_surface, forms, &mut codec, &mut b())
            .err(),
        Some(Error::Selection)
    );
    if !wrong.syntax.bundle.tokens.is_empty() {
        wrong.syntax.bundle.tokens[0].views.roots.clear();
        wrong.syntax.bundle.tokens[0].views.elements.clear();
        assert!(matches!(
            ClosureLowerer::new(registry).lower(&wrong, surface, forms, &mut codec, &mut b()),
            Err(Error::Literal(lower::literal::Error::TokenMismatch(_)))
        ));
        wrong = closure.clone();
    }
    wrong.syntax.category = match expected.value.root {
        Root::Sentence(_) => "Inline",
        Root::Inline(_) => "Sentence",
    }
    .into();
    assert_eq!(
        ClosureLowerer::new(registry)
            .lower(&wrong, surface, forms, &mut codec, &mut b())
            .err(),
        Some(Error::Category)
    );
    wrong.syntax.category = closure.syntax.category.clone();
    wrong.syntax.bundle.root = NodeRef(u64::MAX);
    assert!(matches!(
        ClosureLowerer::new(registry).lower(&wrong, surface, forms, &mut codec, &mut b()),
        Err(Error::Closure(_))
    ));
    let unfinalized = SchemaRegistry::default();
    assert!(matches!(
        ClosureLowerer::new(&unfinalized).lower(&closure, surface, forms, &mut codec, &mut b()),
        Err(Error::Closure(SyntaxError::Schema(
            SchemaError::Unfinalized
        )))
    ));
    Ok(())
}
fn fixture() -> Result<(SchemaRegistry, SchemaRef, SyntaxBundle), String> {
    // Hand-authored source-less subset, separate from the production Grammar
    // test. Descriptor identities are computed, not fabricated digests.
    let descriptor = SchemaDescriptor {
        package: "test.sentence-surface".into(),
        revision: 1,
        operations: vec![],
        types: vec![
            NamedType {
                name: "Form:Guest".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "syntax".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "ForeignSyntax".into(),
                        }),
                    }],
                },
            },
            NamedType {
                name: "Leaf:SentenceLiteral".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Form:Break".into(),
                constraints: vec![],
                shape: TypeShape::Record { fields: vec![] },
            },
            NamedType {
                name: "Form:Emphasis".into(),
                constraints: vec![],
                shape: TypeShape::Record {
                    fields: vec![FieldDescriptor {
                        name: "inline".into(),
                        ty: TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "NodeRef".into(),
                        }),
                    }],
                },
            },
        ],
    };
    let surface = descriptor.reference(&mut b()).map_err(err)?;
    let mut r = SchemaRegistry::default();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?;
    r.register(
        foundation.reference(&mut b()).map_err(err)?,
        foundation,
        &mut b(),
    )
    .map_err(err)?;
    r.register(surface.clone(), descriptor, &mut b())
        .map_err(err)?;
    let sentence = nepl3_sentence_core::schema::descriptor(&mut b()).map_err(err)?;
    r.register(
        sentence.reference(&mut b()).map_err(err)?,
        sentence,
        &mut b(),
    )
    .map_err(err)?;
    r.finalize(&mut b()).map_err(err)?;
    let mut nodes = vec![SyntaxNode {
        schema: surface.clone(),
        kind: "Form:Break".into(),
        fields: vec![],
        head: None,
        cover: None,
        token: None,
        origin: OriginId(0),
    }];
    for i in 0..1000 {
        nodes.push(SyntaxNode {
            schema: surface.clone(),
            kind: "Form:Emphasis".into(),
            fields: vec![FieldValue::Child(NodeRef(i))],
            head: None,
            cover: None,
            token: None,
            origin: OriginId(0),
        });
    }
    Ok((
        r,
        surface,
        SyntaxBundle {
            sources: vec![],
            nodes,
            origins: vec![Origin::Synthetic {
                reason: "test generated prefix".into(),
                anchor: None,
            }],
            root: NodeRef(1000),
            environments: vec![],
            tokens: vec![],
            source_maps: vec![],
        },
    ))
}
#[test]
fn generated_inline_projection_keeps_node_correspondence_and_revalidates_limits()
-> Result<(), String> {
    let (r, surface, bundle) = fixture()?;
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    let result = lower::prefix(
        &checked,
        &surface,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(result.value.root, Root::Inline(InlineRef(1000)));
    assert_eq!(result.value.nodes[0], Kind::Break);
    assert_eq!(
        result.value.nodes[1000],
        Kind::Emphasis {
            inline: InlineRef(999)
        }
    );
    assert_eq!(
        result.syntax_to_meaning,
        (0..=1000).map(Some).collect::<Vec<_>>()
    );
    assert!(
        bundle
            .nodes
            .iter()
            .all(|n| n.head.is_none() && n.cover.is_none())
    );
    let empty = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let syntax =
        lower::presentation::sentence(&checked, &surface, &r, &mut codec, &mut b()).map_err(err)?;
    assert_eq!(syntax.value, result.value);
    assert_eq!(syntax.origins, bundle.origins);
    assert!(
        syntax
            .locations
            .iter()
            .all(|l| l.origin == OriginId(0) && l.head.is_none() && l.cover.is_none())
    );
    let mut shallow = b().limits();
    shallow.depth = 10;
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut Budget::new(shallow),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Stopped(StopReason::DepthLimit))
    );
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut cancelled,
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Stopped(StopReason::Cancelled))
    );
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &SchemaRegistry::default(),
            &mut b(),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Syntax(SyntaxError::Schema(
            SchemaError::Unfinalized
        )))
    );
    Ok(())
}
#[test]
fn structurally_valid_bundle_is_not_a_proof_of_sentence_operands() -> Result<(), String> {
    let (r, surface, mut bundle) = fixture()?;
    // Foundation validates graph geometry, not the language constructor's
    // arity. Lowering must independently reject this missing inline operand.
    bundle.nodes[1000].fields.clear();
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(lower::Error::Unsupported(NodeRef(1000)))
    );
    let proof = bundle
        .validate_in_registry(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let sources = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &sources, &mut admission).map_err(err)?;
    assert_eq!(
        lower::presentation::sentence_in_registry(&proof, &surface, &[], &r, &mut codec, &mut b())
            .err(),
        Some(lower::presentation::Error::Prefix(
            lower::Error::Unsupported(NodeRef(1000))
        ))
    );
    Ok(())
}

#[test]
fn literal_consumer_matches_owner_token_and_returns_typed_stops() -> Result<(), String> {
    use nepl3_core::{
        source::{SourceId, SourceSnapshot, SourceStore},
        value::KindRef,
        view::Token,
    };
    use nepl3_sentence_core::{literal, portable};
    let (r, surface, _) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("literal-owner".into()),
        1,
        "memory:literal-owner".into(),
        "\"[漢/かん]{語/note}\" ".as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let read = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &r,
        &mut b(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let literal::SentenceOutcome::Matched(read) = read.outcome else {
        return Err("literal match".into());
    };
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let payload =
        portable::literal::to_value(&read.syntax, &r, &mut codec, &mut b()).map_err(err)?;
    let mut bundle = SyntaxBundle {
        sources: vec![source.clone()],
        nodes: vec![SyntaxNode {
            schema: surface.clone(),
            kind: "Leaf:SentenceLiteral".into(),
            fields: vec![],
            head: Some(read.head.clone()),
            cover: Some(read.head.clone()),
            origin: OriginId(0),
            token: Some(TokenRef(0)),
        }],
        origins: vec![Origin::Direct(read.head.clone())],
        root: NodeRef(0),
        environments: vec![],
        source_maps: vec![],
        tokens: vec![Token {
            kind: KindRef {
                schema: surface.clone(),
                local_kind: 1, // SentenceLiteral follows the foreign form in this fixture
            },
            head: read.head.clone(),
            payload,
            views: read.syntax.views[0].view.clone(),
            leading_trivia: vec![],
        }],
    };
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::presentation::sentence(&checked, &surface, &r, &mut codec, &mut b()).map_err(err)?,
        read.syntax
    );
    check_closure_entry(&bundle, &surface, &r, &[], &read.syntax)?;
    for (limits, cancel, expected) in [
        (
            {
                let mut l = b().limits();
                l.work = 0;
                l
            },
            false,
            StopReason::WorkLimit,
        ),
        (
            {
                let mut l = b().limits();
                l.depth = 1;
                l
            },
            false,
            StopReason::DepthLimit,
        ),
        (b().limits(), true, StopReason::Cancelled),
    ] {
        let mut budget = Budget::new(limits);
        if cancel {
            budget.cancel();
        }
        assert_eq!(
            lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut budget).err(),
            Some(lower::literal::Error::Stopped(expected))
        );
        assert_eq!(budget.current_depth(), 0);
    }
    // A structurally valid replacement presentation still has to match the
    // payload digest at the literal decoder boundary used by this entry.
    let mut changed_view = bundle.clone();
    changed_view.tokens[0].views.roots.clear();
    changed_view.tokens[0].views.elements.clear();
    let changed = changed_view.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::literal::sentence(&changed, &surface, &r, &mut codec, &mut b()).err(),
        Some(lower::literal::Error::TokenMismatch(NodeRef(0)))
    );
    let mut wrong = surface.clone();
    wrong.revision += 1;
    assert!(matches!(
        lower::literal::sentence(&checked, &wrong, &r, &mut codec, &mut b()),
        Err(lower::literal::Error::Syntax(lower::Error::Unsupported(_)))
    ));
    bundle.nodes[0].token = None;
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut b()).err(),
        Some(lower::literal::Error::TokenMismatch(NodeRef(0)))
    );
    bundle.nodes[0].token = Some(TokenRef(0));
    bundle.nodes[0].cover = Some(source.span(0, source.text().len() as u64).map_err(err)?);
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    assert_eq!(
        lower::literal::sentence(&checked, &surface, &r, &mut codec, &mut b()).err(),
        Some(lower::literal::Error::TokenMismatch(NodeRef(0)))
    );
    Ok(())
}

#[test]
fn selected_foreign_inline_retains_closure_and_checks_identity() -> Result<(), String> {
    let (r, surface, mut bundle) = fixture()?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(
        &environment,
        r.selected("nepl3.foundation", 1).ok_or("foundation")?,
        &r,
        &mut b(),
    )
    .map_err(err)?;
    let guest = ForeignSyntax {
        schema: surface.clone(),
        category: "Inline".into(),
        root: NodeRef(0),
        bundle: SyntaxBundle {
            nodes: vec![bundle.nodes[0].clone()],
            root: NodeRef(0),
            origins: bundle.origins.clone(),
            sources: vec![],
            tokens: vec![],
            environments: vec![],
            source_maps: vec![],
        },
        environment: EnvironmentRef { id: 7, digest },
    };
    bundle.nodes.truncate(1);
    bundle.root = NodeRef(0);
    bundle.nodes[0].kind = "Form:Guest".into();
    bundle.nodes[0].fields = vec![FieldValue::Foreign(Box::new(guest.clone()))];
    bundle.environments.push(EnvironmentEntry {
        id: 7,
        digest,
        value: environment,
    });
    let checked = bundle.validate(&r, &mut b()).map_err(err)?;
    let selection = |schema| lower::ForeignInlineForm {
        kind: "Form:Guest",
        guest_schema: schema,
        guest_category: "Inline",
    };
    assert!(matches!(
        lower::prefix(
            &checked,
            &surface,
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(lower::Error::Unsupported(NodeRef(0)))
    ));
    let mut measured = b();
    let result = lower::prefix_with_foreign(
        &checked,
        &surface,
        &[selection(&surface)],
        &r,
        &mut measured,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(result.value.root, Root::Inline(InlineRef(0)));
    assert_eq!(
        result.value.nodes,
        vec![Kind::ForeignInline {
            syntax: EmbedRef(0)
        }]
    );
    assert_eq!(result.syntax_to_meaning, vec![Some(0)]);
    assert_eq!(result.value.embeds.len(), 1);
    let closure = result.value.embeds[0].syntax().ok_or("syntax content")?;
    assert_eq!(closure.syntax, guest);
    assert_eq!(closure.owner_environment, bundle.environments[0]);
    let empty = nepl3_core::source::SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
    let syntax = lower::presentation::sentence_with_foreign(
        &checked,
        &surface,
        &[selection(&surface)],
        &r,
        &mut codec,
        &mut b(),
    )
    .map_err(err)?;
    assert_eq!(syntax.value, result.value);
    assert_eq!(syntax.locations[0].origin, OriginId(0));
    assert_eq!(syntax.origins, bundle.origins);
    check_closure_entry(&bundle, &surface, &r, &[selection(&surface)], &syntax)?;
    let mut wrong = surface.clone();
    wrong.revision += 1;
    assert!(matches!(
        lower::prefix_with_foreign(
            &checked,
            &surface,
            &[selection(&wrong)],
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(lower::Error::Operand {
            node: NodeRef(0),
            field: 0
        })
    ));
    assert!(matches!(
        lower::prefix_with_foreign(
            &checked,
            &surface,
            &[lower::ForeignInlineForm {
                kind: "Form:Guest",
                guest_schema: &surface,
                guest_category: "Other"
            }],
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(lower::Error::Operand {
            node: NodeRef(0),
            field: 0
        })
    ));
    assert!(matches!(
        lower::prefix_with_foreign(
            &checked,
            &surface,
            &[selection(&surface), selection(&surface)],
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(lower::Error::Unsupported(NodeRef(0)))
    ));
    for (resource, amount, reason) in [
        (Resource::Work, measured.usage().work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            measured.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        let mut limits = b().limits();
        match resource {
            Resource::Work => limits.work = amount - 1,
            _ => limits.allocation_units = amount - 1,
        }
        assert_eq!(
            lower::prefix_with_foreign(
                &checked,
                &surface,
                &[selection(&surface)],
                &r,
                &mut Budget::new(limits),
                &mut SourceAdmission::default()
            )
            .err(),
            Some(lower::Error::Stopped(reason))
        );
    }
    // A host selection cannot replace the meaning of a standard constructor.
    let mut standard = bundle.clone();
    standard.nodes[0].kind = "Form:Emphasis".into();
    let checked = standard.validate(&r, &mut b()).map_err(err)?;
    assert!(matches!(
        lower::prefix_with_foreign(
            &checked,
            &surface,
            &[lower::ForeignInlineForm {
                kind: "Form:Emphasis",
                guest_schema: &surface,
                guest_category: "Inline"
            }],
            &r,
            &mut b(),
            &mut SourceAdmission::default()
        ),
        Err(lower::Error::Unsupported(NodeRef(0)))
    ));
    Ok(())
}
