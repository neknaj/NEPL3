use nepl3_core::{
    budget::{Budget, Limits},
    schema::*,
    value::KindRef,
};
use nepl3_engine::package::*;
use nepl3_reader::{
    builtin::BuiltinReader,
    plan::ReaderPlan,
    tokenizer::{ReaderMode, TakeRule, TokenReader},
};
#[path = "package/head.rs"]
mod head;
#[path = "package/portable.rs"]
mod portable;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn field(name: &str, ty: TypeDescriptor) -> FieldDescriptor {
    FieldDescriptor {
        name: name.into(),
        ty,
    }
}
fn node_ref() -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.foundation".into(),
        revision: 1,
        name: "NodeRef".into(),
    })
}
fn record(name: &str, fields: Vec<FieldDescriptor>) -> NamedType {
    NamedType {
        name: name.into(),
        shape: TypeShape::Record { fields },
        constraints: vec![],
    }
}
fn fixture() -> Result<(LanguagePackage, SchemaRegistry), String> {
    let mut budget = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget),
        nepl3_reader::schema::descriptor(&mut budget),
        nepl3_engine::schema::descriptor(&mut budget),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let reference = descriptor
            .reference(&mut budget)
            .map_err(|e| format!("{e:?}"))?;
        registry
            .register(reference, descriptor, &mut budget)
            .map_err(|e| format!("{e:?}"))?;
    }
    let descriptor = SchemaDescriptor {
        package: "fixture.syntax".into(),
        revision: 1,
        operations: vec![OperationDescriptor {
            name: "facts".into(),
            input: TypeDescriptor::Text,
            output: TypeDescriptor::Unit,
            pure: true,
        }],
        types: vec![
            record(
                "Form:Let",
                vec![field("name", node_ref()), field("body", node_ref())],
            ),
            record("Builtin:Name", vec![]),
            record("Leaf:Name", vec![]),
            record("Token:Word", vec![field("payload", TypeDescriptor::Text)]),
            record(
                "List:Cons",
                vec![
                    field(
                        "head",
                        TypeDescriptor::Named(TypeRef {
                            package: "nepl3.foundation".into(),
                            revision: 1,
                            name: "ForeignSyntax".into(),
                        }),
                    ),
                    field("tail", node_ref()),
                ],
            ),
            record("List:Nil", vec![]),
        ],
    };
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: schema.clone(),
            local_kind: registry
                .kind_id(&schema, name)
                .map_err(|e| format!("{e:?}"))?,
        })
    };
    let package = LanguagePackage {
        schema: schema.clone(),
        payload_schemas: vec![],
        root: "Expr".into(),
        reader: ReaderPlan {
            schema: schema.clone(),
            state_type: TypeDescriptor::Unit,
            expressions: vec![],
            rules: vec![],
            providers: vec![],
        },
        modes: vec![ReaderMode {
            name: "Code".into(),
            skip: vec![],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: kind("Token:Word")?,
            }],
        }],
        categories: vec![Category {
            name: "Expr".into(),
            mode: "Code".into(),
        }],
        reads: vec![
            ReadSpec::Builtin {
                reader: BuiltinReader::Name,
                kind: kind("Builtin:Name")?,
                token_kind: kind("Token:Word")?,
            },
            ReadSpec::Local {
                category: "Expr".into(),
            },
        ],
        forms: vec![Form {
            category: "Expr".into(),
            kind: kind("Form:Let")?,
            spelling: "let".into(),
            fields: vec![
                FieldSpec {
                    name: "name".into(),
                    read: ReadSpecId(0),
                },
                FieldSpec {
                    name: "body".into(),
                    read: ReadSpecId(1),
                },
            ],
            binding: BindingId(2),
            selection_rules: vec![],
            styles: vec![],
        }],
        leaves: vec![Leaf {
            category: "Expr".into(),
            kind: kind("Leaf:Name")?,
            token_kind: kind("Token:Word")?,
            payload: TypeDescriptor::Text,
            binding: BindingId(3),
            selection_rules: vec![],
            styles: vec![],
        }],
        namespaces: vec![Namespace {
            name: "Value".into(),
            policy: NamespacePolicy::Lexical,
        }],
        bindings: vec![
            Binding::Bind {
                namespace: "Value".into(),
                name: NameSelector::Field("name".into()),
            },
            Binding::Visit("body".into()),
            Binding::Group(vec![BindingId(0), BindingId(1)]),
            Binding::Reference {
                namespace: "Value".into(),
                name: NameSelector::SelfValue,
            },
        ],
        extensions: vec![],
        recovery: nepl3_engine::recovery::RecoveryPlan {
            default_unexpected: nepl3_engine::recovery::UnexpectedPolicy::PreserveRemainder,
            rules: vec![],
        },
        provenance: PackageProvenance {
            sources: vec![],
            origins: vec![],
            source_maps: vec![],
            declarations: vec![],
        },
    };
    Ok((package, registry))
}

#[test]
fn persistent_tree_checks_parent_reads_spelling_payload_and_concrete_owner() -> TestResult {
    use nepl3_core::{origin::*, source::*, syntax::*, value::NdfValue, view::*};
    use nepl3_engine::{profile::*, recovery::*, selection::*, tree::*};
    let (package, registry) = fixture()?;
    let identity = package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .semantic_identity(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let profile = ParseProfile {
        id: "tree-test".into(),
        languages: vec![
            LanguageRegistration {
                alias: "A".into(),
                package: identity.clone(),
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "B".into(),
                package: identity,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            package.schema.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    };
    let packages = [&package];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let entry = resolved
        .entry("A", None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let execution = resolved
        .execution_digest("A", &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let source = SourceSnapshot::new(
        SourceId("tree".into()),
        0,
        "memory:tree".into(),
        b"let x y".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let token_kind = package.leaves[0].token_kind.clone();
    let spans = [source.span(0, 3), source.span(4, 5), source.span(6, 7)]
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{e:?}"))?;
    let tokens = spans
        .iter()
        .zip(["let", "x", "y"])
        .map(|(span, text)| Token {
            kind: token_kind.clone(),
            head: span.clone(),
            payload: NdfValue::Text(text.into()),
            views: ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            leading_trivia: vec![],
        })
        .collect();
    let node = |kind: &str, index: usize, fields: Vec<FieldValue>| SyntaxNode {
        schema: package.schema.clone(),
        kind: kind.into(),
        fields,
        head: Some(spans[index].clone()),
        cover: Some(spans[index].clone()),
        origin: OriginId(index as u64),
        token: Some(TokenRef(index as u64)),
    };
    let mut parent = node(
        "Form:Let",
        0,
        vec![FieldValue::Child(NodeRef(1)), FieldValue::Child(NodeRef(2))],
    );
    parent.cover = Some(source.span(0, 7).map_err(|e| format!("{e:?}"))?);
    let mut tree = ParseTree {
        profile_digest: resolved.digest(),
        bundle: SyntaxBundle {
            sources: vec![source],
            nodes: vec![
                parent,
                node("Builtin:Name", 1, vec![]),
                node("Leaf:Name", 2, vec![]),
            ],
            origins: spans.iter().cloned().map(Origin::Direct).collect(),
            root: NodeRef(0),
            environments: vec![],
            tokens,
            source_maps: vec![],
        },
        recovery: vec![],
        contexts: vec![BundleContext {
            path: vec![],
            nodes: vec![
                NodeSelection {
                    node: NodeRef(0),
                    entry: entry.clone(),
                    execution_digest: execution,
                    shape: ShapeSelection::Form { index: 0 },
                },
                NodeSelection {
                    node: NodeRef(1),
                    entry: entry.clone(),
                    execution_digest: execution,
                    shape: ShapeSelection::Builtin {
                        read: ReadSpecId(0),
                    },
                },
                NodeSelection {
                    node: NodeRef(2),
                    entry,
                    execution_digest: execution,
                    shape: ShapeSelection::Leaf { index: 0 },
                },
            ],
        }],
    };
    tree.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    let mut bad = tree.clone();
    bad.contexts[0].nodes[1].entry = resolved
        .entry("B", None, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(
        matches!(
            bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default()),
            Err(TreeError::Selection)
        ),
        "same package under another alias is not the parent's child context"
    );
    let mut bad = tree.clone();
    bad.bundle.nodes[0].fields[0] =
        FieldValue::Atom(nepl3_core::value::NdfScalar::Text("x".into()));
    assert!(
        bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .is_err(),
        "builtin fields own token-bearing child nodes"
    );
    let mut bad = tree.clone();
    bad.contexts[0].nodes[2].execution_digest.0[0] ^= 1;
    assert!(matches!(
        bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default()),
        Err(TreeError::ExecutionIdentity)
    ));
    let mut bad = tree.clone();
    bad.bundle.tokens[2].payload = NdfValue::U64(0);
    assert!(
        bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .is_err(),
        "NDF validity alone does not satisfy leaf payload type"
    );
    let mut bad = tree.clone();
    bad.bundle.tokens[2].kind = package.forms[0].kind.clone();
    assert!(
        bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    let mut bad = tree.clone();
    bad.bundle.nodes.truncate(1);
    bad.bundle.nodes[0].kind = "Leaf:Name".into();
    bad.bundle.nodes[0].fields.clear();
    bad.bundle.nodes[0].cover = bad.bundle.nodes[0].head.clone();
    bad.contexts[0].nodes.truncate(1);
    bad.contexts[0].nodes[0].shape = ShapeSelection::Leaf { index: 0 };
    assert!(
        matches!(
            bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default()),
            Err(TreeError::Selection)
        ),
        "a recognized form spelling cannot be reinterpreted as arity-zero leaf"
    );
    let mut bad = tree.clone();
    bad.bundle.nodes[0].head = Some(spans[1].clone());
    bad.bundle.nodes[0].token = Some(TokenRef(1));
    assert!(
        bad.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .is_err(),
        "form head compares original source spelling, not arbitrary token payload"
    );
    let op = nepl3_core::value::OperationRef {
        schema: package.schema.clone(),
        name: "facts".into(),
    };
    let implementation = ProviderImplementation {
        provider: "fixture-facts".into(),
        revision: 1,
        implementation_digest: Digest::of(b"fixture implementation"),
        operations: vec![op.clone()],
    };
    let mut dynamic_profile = profile.clone();
    dynamic_profile.providers.push(ProviderRequirement {
        provider: implementation.provider.clone(),
        revision: 1,
        implementation_digest: implementation.implementation_digest,
        operation: op.clone(),
    });
    dynamic_profile.allowlist.push(op.clone());
    let implementations = [implementation];
    let dynamic_resolved = dynamic_profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &implementations,
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let mut bad = tree.clone();
    bad.profile_digest = dynamic_resolved.digest();
    bad.contexts[0].nodes[0].shape = ShapeSelection::Dynamic {
        provider: HeadProviderRef {
            shape: op.clone(),
            child_context: op,
        },
        shape: Box::new(HeadShape {
            kind: package.forms[0].kind.clone(),
            fields: package.forms[0].fields.clone(),
            binding: package.forms[0].binding,
            selection_rules: vec![],
            styles: package.forms[0].styles.clone(),
        }),
        child_contexts: vec![
            bad.contexts[0].nodes[1].entry.clone(),
            bad.contexts[0].nodes[2].entry.clone(),
        ],
    };
    assert!(
        matches!(
            bad.validate(
                &dynamic_resolved,
                &mut budget(),
                &mut SourceAdmission::default()
            ),
            Err(TreeError::UnvalidatedDynamic)
        ),
        "an allowed facts(Text -> Unit) operation cannot stand in for a checked HeadProvider protocol"
    );
    let duplicate = tree.contexts[0].nodes[0].clone();
    tree.contexts[0].nodes.push(duplicate);
    assert!(
        tree.validate(&resolved, &mut budget(), &mut SourceAdmission::default())
            .is_err()
    );
    Ok(())
}
#[test]
fn package_checks_real_surface_shapes_modes_selectors_and_binding_coverage() -> TestResult {
    let (package, registry) = fixture()?;
    package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut bad = package.clone();
    bad.forms[0].fields[0].read = ReadSpecId(99);
    assert!(matches!(
        bad.check(&registry, &mut budget()),
        Err(PackageError::InvalidRead)
    ));
    let mut bad = package.clone();
    bad.bindings[2] = Binding::None;
    assert!(matches!(
        bad.check(&registry, &mut budget()),
        Err(PackageError::UnvisitedField)
    ));
    let mut bad = package.clone();
    bad.bindings[2] = Binding::Group(vec![BindingId(2)]);
    assert!(matches!(
        bad.check(&registry, &mut budget()),
        Err(PackageError::DirectCycle)
    ));
    let mut bad = package.clone();
    bad.categories[0].mode = "Missing".into();
    assert!(matches!(
        bad.check(&registry, &mut budget()),
        Err(PackageError::MissingMode)
    ));
    let mut bad = package.clone();
    bad.forms[0].fields[0].name = "wrong".into();
    assert!(matches!(
        bad.check(&registry, &mut budget()),
        Err(PackageError::KindShape)
    ));
    Ok(())
}
#[test]
fn listof_foreign_has_foreign_head_and_local_tail_without_losing_its_selector() -> TestResult {
    let (mut package, registry) = fixture()?;
    let kind = |name: &str| -> Result<KindRef, String> {
        Ok(KindRef {
            schema: package.schema.clone(),
            local_kind: registry
                .kind_id(&package.schema, name)
                .map_err(|e| format!("{e:?}"))?,
        })
    };
    let cons = kind("List:Cons")?;
    let nil = kind("List:Nil")?;
    package.reads.push(ReadSpec::Foreign {
        alias: "Guest".into(),
        category: "Sentence".into(),
    });
    package.reads.push(ReadSpec::WithMode {
        mode: "GuestOnly".into(),
        read: ReadSpecId(2),
    });
    package.reads.push(ReadSpec::ListOf {
        element: ReadSpecId(3),
        cons,
        nil,
    });
    package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    // A guest mode is symbolic at this local boundary, never satisfied by a host mode.
    package.reads.push(ReadSpec::WithMode {
        mode: "GuestOnly".into(),
        read: ReadSpecId(4),
    });
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::MissingMode)
    ));
    package.reads.pop();
    package.reads[3] = ReadSpec::WithMode {
        mode: "Code".into(),
        read: ReadSpecId(2),
    };
    package.reads[2] = ReadSpec::Local {
        category: "Expr".into(),
    };
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::KindShape)
    ));
    Ok(())
}

#[test]
fn package_rejects_direct_reader_cycles_and_counts_binding_entry_depth() -> TestResult {
    use nepl3_reader::plan::{ReaderExpr, ReaderId};
    let (mut package, registry) = fixture()?;
    package.reader.expressions = vec![ReaderExpr::Commit(ReaderId(0))];
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::DirectCycle)
    ));
    package.reader.expressions.clear();
    // No form needs a child binding. One leaf binding has exactly one active frame.
    package.forms.clear();
    package.reads.clear();
    package.bindings = vec![Binding::None];
    package.leaves[0].binding = BindingId(0);
    let mut limited = Budget::new(Limits {
        depth: 1,
        ..budget().limits()
    });
    package
        .check(&registry, &mut limited)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(limited.usage().depth, 1);
    Ok(())
}

#[test]
fn package_boundary_checks_extension_provenance_and_resource_limits() -> TestResult {
    use nepl3_core::{
        budget::StopReason,
        origin::{Origin, OriginId},
        source::{SourceId, SourceSnapshot},
        value::OperationRef,
    };
    let (mut package, registry) = fixture()?;
    let fact_type = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.engine".into(),
            revision: 1,
            name: name.into(),
        })
    };
    package.extensions.push(ExtensionRequirement {
        alias: "facts".into(),
        provider: "fixture.facts/v1".into(),
        signature: "facts/v1".into(),
        operation: OperationRef {
            schema: registry
                .selected("nepl3.engine", 1)
                .ok_or("engine")?
                .clone(),
            name: "bindingFacts".into(),
        },
        input: fact_type("FactsRequest"),
        output: fact_type("FactsReply"),
        pure: true,
    });
    package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    package.extensions[0].output = TypeDescriptor::Bool;
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::SignatureMismatch)
    ));
    package.extensions[0].output = fact_type("FactsReply");
    let source = SourceSnapshot::new(
        SourceId("grammar-source".into()),
        0,
        "memory:grammar".into(),
        b"grammar".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    package.provenance.origins.push(Origin::Direct(
        source.span(0, 7).map_err(|e| format!("{e:?}"))?,
    ));
    // A span cannot resolve through a host store outside the provenance table.
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::Origin(_))
    ));
    package.provenance.sources.push(source);
    package.provenance.declarations.push(DeclarationOrigin {
        category: None,
        kind: DeclarationKind::Category,
        name: "Expr".into(),
        origin: OriginId(1),
    });
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::Provenance)
    ));
    package.provenance.declarations[0].origin = OriginId(0);
    package
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        package.check(&registry, &mut stopped),
        Err(PackageError::Stopped(StopReason::WorkLimit))
    ));
    let mut stopped = Budget::new(Limits {
        depth: 0,
        ..budget().limits()
    });
    assert!(matches!(
        package.check(&registry, &mut stopped),
        Err(PackageError::Stopped(StopReason::DepthLimit))
    ));
    Ok(())
}

fn semantic(
    package: &LanguagePackage,
    registry: &SchemaRegistry,
) -> Result<nepl3_core::source::Digest, String> {
    package
        .check(registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .semantic_identity(&mut budget())
        .map(|v| v.semantic_digest)
        .map_err(|e| format!("{e:?}"))
}
#[test]
fn semantic_identity_removes_arena_layout_sharing_and_provenance() -> TestResult {
    use nepl3_core::{
        origin::Origin,
        source::{SourceId, SourceSnapshot},
    };
    use nepl3_reader::plan::{ReaderExpr, ReaderId, ReaderRule};
    let (mut a, registry) = fixture()?;
    a.reader.expressions = vec![
        ReaderExpr::Literal("a".into()),
        ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]),
    ];
    a.reader.rules = vec![ReaderRule {
        name: "Pair".into(),
        root: ReaderId(1),
        output: TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)),
    }];
    let digest = semantic(&a, &registry)?;
    let mut b = a.clone();
    b.reader.expressions = vec![
        ReaderExpr::Seq(vec![ReaderId(2), ReaderId(1)]),
        ReaderExpr::Literal("a".into()),
        ReaderExpr::Literal("a".into()),
        ReaderExpr::Literal("unused arena storage".into()),
    ];
    b.reader.rules[0].root = ReaderId(0);
    b.bindings.swap(0, 1);
    b.bindings[2] = Binding::Group(vec![BindingId(1), BindingId(0)]);
    b.reads.swap(0, 1);
    b.forms[0].fields[0].read = ReadSpecId(1);
    b.forms[0].fields[1].read = ReadSpecId(0);
    assert_ne!(
        a.reader
            .digest(&mut budget())
            .map_err(|e| format!("{e:?}"))?,
        b.reader
            .digest(&mut budget())
            .map_err(|e| format!("{e:?}"))?
    );
    assert_eq!(digest, semantic(&b, &registry)?);
    let execution = |p: &LanguagePackage| -> Result<_, String> {
        p.check(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .execution_digest(&mut budget())
            .map_err(|e| format!("{e:?}"))
    };
    assert_ne!(execution(&a)?, execution(&b)?);
    let before_provenance = execution(&b)?;
    let source = SourceSnapshot::new(
        SourceId("grammar-provenance".into()),
        4,
        "memory:grammar".into(),
        b"different layout".to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    b.provenance.origins.push(Origin::Direct(
        source.span(0, 9).map_err(|e| format!("{e:?}"))?,
    ));
    b.provenance.sources.push(source);
    assert_eq!(digest, semantic(&b, &registry)?);
    assert_ne!(before_provenance, execution(&b)?);
    b.reader.expressions.push(ReaderExpr::Commit(ReaderId(999)));
    assert!(b.check(&registry, &mut budget()).is_err());
    Ok(())
}

#[test]
fn recovery_strategy_and_sync_order_are_part_of_package_behavior() -> TestResult {
    use nepl3_engine::recovery::*;
    let (mut package, registry) = fixture()?;
    let original = semantic(&package, &registry)?;
    package.recovery.default_unexpected = UnexpectedPolicy::ConsumeToken;
    assert_ne!(original, semantic(&package, &registry)?);
    package.recovery.rules.push(RecoveryRule {
        category: "Expr".into(),
        unexpected: UnexpectedPolicy::PreserveRemainder,
        synchronization: vec![
            SyncToken {
                ancestor_category: "Expr".into(),
                kind: package.leaves[0].token_kind.clone(),
                spelling: Some("end1".into()),
            },
            SyncToken {
                ancestor_category: "Expr".into(),
                kind: package.leaves[0].token_kind.clone(),
                spelling: Some("end2".into()),
            },
        ],
    });
    let original = semantic(&package, &registry)?;
    package.recovery.rules[0].synchronization.reverse();
    assert_ne!(original, semantic(&package, &registry)?);
    package.recovery.rules[0].synchronization[0].ancestor_category = "Absent".into();
    assert!(matches!(
        package.check(&registry, &mut budget()),
        Err(PackageError::MissingCategory)
    ));
    Ok(())
}
#[test]
fn semantic_identity_preserves_ordered_behavior_and_presentation_fallback() -> TestResult {
    use nepl3_core::view::{FallbackRole, PresentationClass};
    use nepl3_reader::plan::{ReaderExpr, ReaderId, ReaderRule};
    let (mut a, registry) = fixture()?;
    a.reader.expressions = vec![
        ReaderExpr::Literal("a".into()),
        ReaderExpr::Literal("b".into()),
        ReaderExpr::Choice(vec![ReaderId(0), ReaderId(1)]),
    ];
    a.reader.rules = vec![ReaderRule {
        name: "Word".into(),
        root: ReaderId(2),
        output: TypeDescriptor::Unit,
    }];
    a.forms[0].styles.push(StyleRule {
        selector: StyleSelector::Head,
        class: PresentationClass {
            schema: a.schema.clone(),
            name: "head".into(),
            fallback: FallbackRole::Content,
        },
    });
    a.modes[0].take.push(TakeRule {
        reader: TokenReader::Builtin(BuiltinReader::Text),
        kind: a.leaves[0].token_kind.clone(),
    });
    let digest = semantic(&a, &registry)?;
    let mut b = a.clone();
    b.reader.expressions[2] = ReaderExpr::Choice(vec![ReaderId(1), ReaderId(0)]);
    assert_ne!(digest, semantic(&b, &registry)?);
    let mut b = a.clone();
    b.bindings[2] = Binding::Group(vec![BindingId(1), BindingId(0)]);
    assert_ne!(digest, semantic(&b, &registry)?);
    let mut b = a.clone();
    b.modes[0].take.reverse();
    assert_ne!(digest, semantic(&b, &registry)?);
    let mut b = a.clone();
    b.forms[0].styles[0].class.fallback = FallbackRole::Marker;
    assert_ne!(digest, semantic(&b, &registry)?);
    let checked = a
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut stopped = Budget::new(Limits {
        allocation_units: 0,
        ..budget().limits()
    });
    assert!(matches!(
        checked.semantic_identity(&mut stopped),
        Err(PackageError::Stopped(_))
    ));
    Ok(())
}

#[test]
fn resolved_profile_pins_real_package_host_providers_resources_and_foreign_modes() -> TestResult {
    use nepl3_core::{source::Digest, value::OperationRef};
    use nepl3_engine::profile::*;
    let (mut host, registry) = fixture()?;
    let mut guest = host.clone();
    guest.modes.push(nepl3_reader::tokenizer::ReaderMode {
        name: "GuestOnly".into(),
        skip: vec![],
        take: guest.modes[0].take.clone(),
    });
    host.reads.push(ReadSpec::Foreign {
        alias: "Guest".into(),
        category: "Expr".into(),
    });
    host.reads.push(ReadSpec::WithMode {
        mode: "GuestOnly".into(),
        read: ReadSpecId(2),
    });
    let identity = |p: &LanguagePackage| -> Result<PackageIdentity, String> {
        p.check(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .semantic_identity(&mut budget())
            .map_err(|e| format!("{e:?}"))
    };
    let op = OperationRef {
        schema: host.schema.clone(),
        name: "facts".into(),
    };
    // Test implementation manifest bytes are fixture data, never a distributed provider hash.
    let provider = ProviderImplementation {
        provider: "fixture-provider".into(),
        revision: 1,
        implementation_digest: Digest::of(b"fixture implementation manifest v1"),
        operations: vec![op.clone()],
    };
    let resource = ResourceSnapshot {
        id: "fixture-style".into(),
        bytes: b"style content".to_vec(),
    };
    let mut profile = ParseProfile {
        id: "fixture.profile".into(),
        languages: vec![
            LanguageRegistration {
                alias: "Host".into(),
                package: identity(&host)?,
                default_category: "Expr".into(),
            },
            LanguageRegistration {
                alias: "Guest".into(),
                package: identity(&guest)?,
                default_category: "Expr".into(),
            },
        ],
        schemas: vec![
            host.schema.clone(),
            registry
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?
                .clone(),
        ],
        head_providers: vec![],
        category_modes: vec![CategoryMode {
            alias: "Guest".into(),
            category: "Expr".into(),
            mode: "GuestOnly".into(),
        }],
        providers: vec![ProviderRequirement {
            provider: provider.provider.clone(),
            revision: provider.revision,
            implementation_digest: provider.implementation_digest,
            operation: op.clone(),
        }],
        allowlist: vec![op.clone()],
        resources: vec![ResourceIdentity {
            id: resource.id.clone(),
            digest: Digest::of(&resource.bytes),
        }],
        limits: budget().limits(),
    };
    let packages = [&host, &guest];
    let providers = [provider];
    let resources = [resource];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &providers,
        resources: &resources,
    };
    let digest = profile
        .resolve(&catalog, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?
        .digest();
    profile.languages.reverse();
    profile.schemas.reverse();
    let resolved = profile
        .resolve(&catalog, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(digest, resolved.digest());
    assert_eq!(
        resolved
            .entry("Host", None, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .mode,
        "Code"
    );
    assert_eq!(
        resolved
            .entry("Guest", None, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .mode,
        "GuestOnly"
    );
    let missing = RuntimeCatalog {
        providers: &[],
        ..catalog
    };
    assert!(matches!(
        profile.resolve(&missing, &registry, &mut budget()),
        Err(ProfileError::MissingProvider)
    ));
    let mut denied = profile.clone();
    denied.allowlist.clear();
    let resolved = denied
        .resolve(&catalog, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        resolved.provider(&op, &mut budget()),
        Err(ProfileError::NotAllowed)
    ));
    let mut bad = profile.clone();
    bad.providers[0].implementation_digest = Digest::of(b"self asserted change");
    assert!(matches!(
        bad.resolve(&catalog, &registry, &mut budget()),
        Err(ProfileError::ProviderIdentity)
    ));
    let mut updated = providers.clone();
    updated[0].implementation_digest = bad.providers[0].implementation_digest;
    let changed = RuntimeCatalog {
        providers: &updated,
        ..catalog
    };
    assert_ne!(
        digest,
        bad.resolve(&changed, &registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .digest()
    );
    let mut bad = profile.clone();
    bad.resources[0].digest = Digest::of(b"wrong resource");
    assert!(matches!(
        bad.resolve(&catalog, &registry, &mut budget()),
        Err(ProfileError::ResourceIdentity)
    ));
    let mut bad = profile.clone();
    bad.languages.retain(|v| v.alias != "Guest");
    bad.category_modes.clear();
    assert!(matches!(
        bad.resolve(&catalog, &registry, &mut budget()),
        Err(ProfileError::MissingAlias)
    ));
    let mut bad = profile.clone();
    bad.category_modes[0].mode = "Absent".into();
    assert!(matches!(
        bad.resolve(&catalog, &registry, &mut budget()),
        Err(ProfileError::MissingMode)
    ));
    let mut aliases = profile.clone();
    let mut duplicate = aliases
        .languages
        .iter()
        .find(|v| v.alias == "Guest")
        .ok_or("guest")?
        .clone();
    duplicate.alias = "GuestCopy".into();
    aliases.languages.push(duplicate);
    let resolved = aliases
        .resolve(&catalog, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        resolved
            .entry("GuestCopy", None, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .alias,
        "GuestCopy"
    );
    assert_eq!(
        resolved
            .entry("GuestCopy", None, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .mode,
        "Code"
    );
    let long = "p".repeat(100_000);
    let mut bad = profile.clone();
    bad.providers[0].provider = format!("{long}x");
    let mut long_providers = providers.clone();
    long_providers[0].provider = format!("{long}y");
    let long_catalog = RuntimeCatalog {
        providers: &long_providers,
        ..catalog
    };
    let mut limited = Budget::new(Limits {
        work: 50_000,
        ..budget().limits()
    });
    assert!(matches!(
        bad.resolve(&long_catalog, &registry, &mut limited),
        Err(ProfileError::Stopped(
            nepl3_core::budget::StopReason::WorkLimit
        ))
    ));
    let mut bad = profile.clone();
    bad.resources[0].id = format!("{long}x");
    let mut long_resources = resources.clone();
    long_resources[0].id = format!("{long}y");
    let long_catalog = RuntimeCatalog {
        resources: &long_resources,
        ..catalog
    };
    let mut limited = Budget::new(Limits {
        work: 50_000,
        ..budget().limits()
    });
    assert!(matches!(
        bad.resolve(&long_catalog, &registry, &mut limited),
        Err(ProfileError::Stopped(
            nepl3_core::budget::StopReason::WorkLimit
        ))
    ));
    let mut limited = Budget::new(Limits {
        work: 500,
        ..budget().limits()
    });
    assert!(matches!(
        resolved.entry("GuestCopy", Some(&long), &mut limited),
        Err(ProfileError::Stopped(
            nepl3_core::budget::StopReason::WorkLimit
        ))
    ));
    Ok(())
}
