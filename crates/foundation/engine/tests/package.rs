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
            styles: vec![],
        }],
        leaves: vec![Leaf {
            category: "Expr".into(),
            kind: kind("Leaf:Name")?,
            token_kind: kind("Token:Word")?,
            payload: TypeDescriptor::Text,
            binding: BindingId(3),
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
    package.extensions.push(ExtensionRequirement {
        alias: "facts".into(),
        provider: "fixture.facts/v1".into(),
        signature: "facts/v1".into(),
        operation: OperationRef {
            schema: package.schema.clone(),
            name: "facts".into(),
        },
        input: TypeDescriptor::Text,
        output: TypeDescriptor::Unit,
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
    package.extensions[0].output = TypeDescriptor::Unit;
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
