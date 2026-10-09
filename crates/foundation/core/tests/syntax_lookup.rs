use nepl3_core::{budget::*, origin::*, schema::*, syntax::*, value::*};
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
fn fixture(padding: usize, types: usize) -> Result<(SchemaRegistry, SyntaxBundle), SyntaxError> {
    let mut registry = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let named = |name| NamedType {
        name,
        shape: TypeShape::Record { fields: vec![] },
        constraints: vec![],
    };
    let mut definitions: Vec<_> = (0..types).map(|i| named(format!("A{i:02}"))).collect();
    definitions.push(named("ZNode".into()));
    definitions.reverse(); // Registration must normalize before binary lookup.
    let d = SchemaDescriptor {
        package: "target".into(),
        revision: 1,
        types: definitions,
        operations: vec![],
    };
    let schema = d.reference(&mut budget())?;
    registry.register(schema.clone(), d, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((
        registry,
        SyntaxBundle {
            sources: vec![],
            nodes: vec![SyntaxNode {
                schema,
                kind: "ZNode".into(),
                // Shared syntax checks existence, not domain-field correspondence.
                fields: vec![FieldValue::Atom(NdfScalar::Bool(true))],
                head: None,
                cover: None,
                origin: OriginId(0),
                token: None,
            }],
            origins: vec![Origin::Synthetic {
                reason: "test".into(),
                anchor: None,
            }],
            root: NodeRef(0),
            environments: vec![],
            tokens: vec![],
            source_maps: vec![],
        },
    ))
}
#[test]
fn syntax_lookup_charges_catalog_and_normalized_type_probes() -> Result<(), SyntaxError> {
    let (registry, input) = fixture(0, 0)?;
    let mut baseline = budget();
    input.validate(&registry, &mut baseline)?;
    for (padding, types, extra) in [(32, 0, 32 * (10 + 6 + 9)), (0, 32, 4 * (3 + 5 + 1))] {
        let (registry, input) = fixture(padding, types)?;
        let mut b = budget();
        input.validate(&registry, &mut b)?;
        let mut expected = baseline.usage();
        expected.work += extra;
        assert_eq!(b.usage(), expected);
        let mut limits = budget().limits();
        limits.work = baseline.usage().work;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            input.validate(&registry, &mut stopped),
            Err(SyntaxError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}

#[test]
fn syntax_schema_and_type_errors_precede_origin_errors() -> Result<(), SyntaxError> {
    for mutation in 0..4 {
        let (registry, mut input) = fixture(32, 32)?;
        input.nodes[0].origin = OriginId(99);
        match mutation {
            0 => input.nodes[0].schema.package = "missing".into(),
            1 => input.nodes[0].schema.revision += 1,
            2 => input.nodes[0].schema.digest.0[0] ^= 1,
            _ => input.nodes[0].kind = "missing".into(),
        }
        let expected = if mutation == 3 {
            SchemaError::UnknownType
        } else {
            SchemaError::UnknownSchema
        };
        assert!(
            matches!(input.validate(&registry, &mut budget()), Err(SyntaxError::Schema(e)) if e == expected)
        );
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            input.validate(&registry, &mut cancelled),
            Err(SyntaxError::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(cancelled.usage(), Usage::default());
    }
    Ok(())
}

#[test]
fn namespace_and_foreign_schema_checks_share_the_callers_budget() -> Result<(), SyntaxError> {
    use nepl3_core::source::{Digest, SourceAdmission, SourceStore};
    for case in 0..2 {
        let mut usage = vec![];
        for padding in [0, 32] {
            let (registry, input) = fixture(padding, 0)?;
            let schema = input.nodes[0].schema.clone();
            let env = Environment {
                bindings: vec![EnvironmentBinding {
                    namespace: NamespaceRef {
                        schema: schema.clone(),
                        name: "arbitrary-namespace".into(),
                    },
                    name: "binding".into(),
                    value: TypedValue::Record(Record {
                        schema: schema.clone(),
                        kind: "ZNode".into(),
                        fields: vec![],
                    }),
                    origin: None,
                }],
                resources: vec![],
            };
            let sources = SourceStore::default();
            let mut b = budget();
            if case == 0 {
                env.validate(&[], &sources, &registry, &mut b)?;
                let mut invalid = env;
                invalid.bindings[0].namespace.schema.package = "missing".into();
                invalid.bindings[0].name.clear();
                assert!(matches!(
                    invalid.validate(&[], &sources, &registry, &mut budget()),
                    Err(SyntaxError::Schema(SchemaError::UnknownSchema))
                ));
            } else {
                let mut closure = ForeignClosure {
                    syntax: ForeignSyntax {
                        schema,
                        category: "arbitrary-category".into(),
                        root: input.root,
                        bundle: input,
                        environment: EnvironmentRef {
                            id: 0,
                            digest: Digest([0; 32]),
                        },
                    },
                    owner_environment: EnvironmentEntry {
                        id: 0,
                        digest: Digest([0; 32]),
                        value: Environment {
                            bindings: vec![],
                            resources: vec![],
                        },
                    },
                    owner_origins: vec![],
                    owner_sources: vec![],
                    owner_source_maps: vec![],
                };
                closure.validate(&registry, &mut b, &mut SourceAdmission::default())?;
                let mut before_outer = budget();
                closure
                    .syntax
                    .bundle
                    .validate(&registry, &mut before_outer)?;
                let mut limits = budget().limits();
                limits.work = before_outer.usage().work;
                assert!(matches!(
                    closure.validate(
                        &registry,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default()
                    ),
                    Err(SyntaxError::Stopped(StopReason::WorkLimit))
                ));
                closure.syntax.schema.digest.0[0] ^= 1;
                assert!(matches!(
                    closure.validate(&registry, &mut budget(), &mut SourceAdmission::default()),
                    Err(SyntaxError::Schema(SchemaError::UnknownSchema))
                ));
                closure.syntax.bundle.nodes[0].kind = "missing".into();
                assert!(matches!(
                    closure.validate(&registry, &mut budget(), &mut SourceAdmission::default()),
                    Err(SyntaxError::Schema(SchemaError::UnknownType))
                ));
            }
            usage.push(b.usage());
        }
        // Namespace + typed payload, or guest node + outer foreign schema.
        let mut expected = usage[0];
        expected.work += 2 * 32 * (10 + 6 + 9);
        assert_eq!(usage[1], expected, "case {case}");
    }
    Ok(())
}

#[test]
fn syntax_long_names_and_unicode_use_normalized_lookup() -> Result<(), SyntaxError> {
    let (_, mut input) = fixture(0, 0)?;
    let names = ["Ω".to_string(), "A".repeat(1024), "節点".to_string()];
    let descriptor = SchemaDescriptor {
        package: "package".repeat(256),
        revision: 1,
        types: names
            .iter()
            .map(|name| NamedType {
                name: name.clone(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            })
            .collect(),
        operations: vec![],
    };
    let mut registry = SchemaRegistry::default();
    let schema = descriptor.reference(&mut budget())?;
    registry.register(schema.clone(), descriptor, &mut budget())?;
    registry.finalize(&mut budget())?;
    input.nodes[0].schema = schema;
    for name in names {
        input.nodes[0].kind = name;
        let mut b = budget();
        input.validate(&registry, &mut b)?;
        // Normalized order is [long ASCII, Ω, 節点], with Ω probed first.
        let name_work = match input.nodes[0].kind.as_str() {
            "Ω" => 2 + 2 + 1,
            "節点" => (2 + 6 + 1) + (6 + 6 + 1),
            _ => (2 + 1024 + 1) + (1024 + 1024 + 1),
        };
        // Existing graph/field work is 7; lookup entry costs 1. Selection compares
        // both package strings, then full identity charges package + 41.
        assert_eq!(
            b.usage().work,
            7 + 1 + (1792 + 1792 + 9) + (1792 + 41) + name_work
        );
        let mut limits = budget().limits();
        limits.work = b.usage().work - 1;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            input.validate(&registry, &mut stopped),
            Err(SyntaxError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}
