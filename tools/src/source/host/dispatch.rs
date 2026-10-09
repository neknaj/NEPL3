use super::*;
use nepl3_core::{
    budget::Limits,
    diagnostic::Report,
    schema::*,
    source::SourceId,
    syntax::{Environment, EnvironmentEntry},
    value::{NdfValue, OperationRef},
};
use nepl3_reader::{
    model::{OwnedReadRequest, ReadReply, ReaderContext},
    runtime::ReaderError,
};
fn b() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        source_bytes: 1000,
        nodes: 100_000,
        depth: 100,
        output_bytes: 10_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn first(
    _: &OperationRef,
    request: ReadRequest<'_>,
    _: &SchemaRegistry,
    _: &SourceStore,
    b: &mut Budget,
    _: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    b.poll()?;
    Ok(ReadReply::NoMatch {
        expected: vec![],
        furthest: request.start,
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
fn second(
    _: &OperationRef,
    _: ReadRequest<'_>,
    _: &SchemaRegistry,
    _: &SourceStore,
    b: &mut Budget,
    _: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    b.poll()?;
    Ok(ReadReply::NeedMore {
        expected: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
#[test]
fn explicit_dispatch_distinguishes_same_named_operations_and_rejects_forged_binding()
-> Result<(), String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_reader::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    let named = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.reader".into(),
            revision: 1,
            name: name.into(),
        })
    };
    let mut operations = vec![];
    for package in ["test.first", "test.second"] {
        let d = SchemaDescriptor {
            package: package.into(),
            revision: 1,
            types: vec![],
            operations: vec![OperationDescriptor {
                name: "read".into(),
                input: named("ReadRequest"),
                output: named("ReadReply"),
                pure: true,
            }],
        };
        let schema = d.reference(&mut b()).map_err(err)?;
        r.register(schema.clone(), d, &mut b()).map_err(err)?;
        operations.push(OperationRef {
            schema,
            name: "read".into(),
        });
    }
    r.finalize(&mut b()).map_err(err)?;
    let implementation = Digest::of(b"test-only callbacks first and second");
    let source = SourceSnapshot::new(
        SourceId("dispatch".into()),
        1,
        "memory:dispatch".into(),
        vec![],
        &mut b(),
    )
    .map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let context = ReaderContext {
        schema: operations[0].schema.clone(),
        category: "Token".into(),
        mode: "Code".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest: nepl3_wire::environment::environment_digest(
                &environment,
                r.selected("nepl3.foundation", 1).ok_or("foundation")?,
                &r,
                &mut b(),
            )
            .map_err(err)?,
            value: environment,
        },
        origins: vec![],
    };
    for reversed in [false, true] {
        let mut readers = vec![
            NativeReader {
                operation: operations[0].clone(),
                read: first,
            },
            NativeReader {
                operation: operations[1].clone(),
                read: second,
            },
        ];
        if reversed {
            readers.reverse();
        }
        let mut host = NativeHost::new(&r, implementation, "dispatch:".into(), readers, &mut b())
            .map_err(err)?;
        for (index, operation) in operations.iter().enumerate() {
            let call = ProviderCall::Read {
                session_id: "dispatch".into(),
                call_id: 1,
                depth_base: 0,
                operation: operation.clone(),
                request: OwnedReadRequest {
                    snapshot: source.reference(),
                    sources: vec![source.clone()],
                    start: 0,
                    limit: 0,
                    final_input: false,
                    context: context.clone(),
                    state: NdfValue::Unit,
                },
            };
            let mut requirement = ProviderRequirement {
                provider: "read".into(),
                revision: 1,
                implementation_digest: implementation,
                operation: operation.clone(),
            };
            let reply = host
                .provider(
                    &call,
                    &requirement,
                    &mut b(),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?;
            assert!(
                matches!(reply, Some(ProviderReply::Read(reply)) if matches!(*reply,
                ReadReply::NoMatch { .. } if index == 0) || matches!(*reply, ReadReply::NeedMore { .. } if index == 1))
            );
            requirement.implementation_digest = Digest::of(b"wrong implementation");
            assert!(matches!(
                host.provider(
                    &call,
                    &requirement,
                    &mut b(),
                    &mut SourceAdmission::default()
                ),
                Err(ParseError::Context)
            ));
        }
    }
    assert!(matches!(
        NativeHost::new(
            &r,
            implementation,
            "duplicate:".into(),
            vec![
                NativeReader {
                    operation: operations[0].clone(),
                    read: first
                },
                NativeReader {
                    operation: operations[0].clone(),
                    read: second
                }
            ],
            &mut b()
        ),
        Err(ParseError::Context)
    ));
    Ok(())
}

#[test]
fn registration_meters_registry_search_and_preserves_schema_identity() -> Result<(), String> {
    let registry = |padding: usize| -> Result<(SchemaRegistry, OperationRef), String> {
        let mut r = SchemaRegistry::default();
        for d in [
            nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?,
            nepl3_reader::schema::descriptor(&mut b()).map_err(err)?,
        ] {
            r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
                .map_err(err)?;
        }
        for i in 0..padding {
            let d = SchemaDescriptor {
                package: format!("test.padding.{i}"),
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
                .map_err(err)?;
        }
        let named = |name: &str| {
            TypeDescriptor::Named(TypeRef {
                package: "nepl3.reader".into(),
                revision: 1,
                name: name.into(),
            })
        };
        let d = SchemaDescriptor {
            package: "test.registration".into(),
            revision: 1,
            types: vec![],
            operations: vec![OperationDescriptor {
                name: "read".into(),
                input: named("ReadRequest"),
                output: named("ReadReply"),
                pure: true,
            }],
        };
        let schema = d.reference(&mut b()).map_err(err)?;
        r.register(schema.clone(), d, &mut b()).map_err(err)?;
        r.finalize(&mut b()).map_err(err)?;
        Ok((
            r,
            OperationRef {
                schema,
                name: "read".into(),
            },
        ))
    };
    let (small, operation) = registry(0)?;
    let (large, same_operation) = registry(32)?;
    assert_eq!(operation, same_operation);
    let make = |r: &SchemaRegistry, operation: OperationRef, budget: &mut Budget| {
        NativeHost::new(
            r,
            Digest::of(b"registration-test"),
            "registration:".into(),
            vec![NativeReader {
                operation,
                read: first,
            }],
            budget,
        )
        .map(|_| ())
    };
    let mut small_budget = b();
    make(&small, operation.clone(), &mut small_budget).map_err(err)?;
    let mut large_budget = b();
    make(&large, operation.clone(), &mut large_budget).map_err(err)?;
    // Each added schema is examined before the target. The registry lookup
    // charges both package lengths plus revision/comparison overhead (9).
    let extra: u64 = (0..32)
        .map(|i| {
            format!("test.padding.{i}").len() as u64 + operation.schema.package.len() as u64 + 9
        })
        .sum();
    assert_eq!(large_budget.usage().work - small_budget.usage().work, extra);
    assert_eq!(
        large_budget.usage().allocation_units,
        small_budget.usage().allocation_units
    );
    let mut limited = Budget::new(Limits {
        work: small_budget.usage().work,
        ..b().limits()
    });
    assert!(matches!(
        make(&large, operation.clone(), &mut limited),
        Err(ParseError::Stopped(
            nepl3_core::budget::StopReason::WorkLimit
        ))
    ));
    assert_eq!(
        limited.poll(),
        Err(nepl3_core::budget::StopReason::WorkLimit)
    );
    assert_eq!(limited.usage().allocation_units, 0);
    let mut forged = operation;
    forged.schema.digest = Digest::of(b"different schema");
    assert!(matches!(
        make(&large, forged, &mut b()),
        Err(ParseError::Context)
    ));
    Ok(())
}

#[test]
fn registration_rejects_non_read_envelopes_without_restricting_purity() -> Result<(), String> {
    let named = |package: &str, revision: u64, name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: package.into(),
            revision,
            name: name.into(),
        })
    };
    let input = named("nepl3.reader", 1, "ReadRequest");
    let output = named("nepl3.reader", 1, "ReadReply");
    let cases = [
        (input.clone(), output.clone(), true, true),
        (input.clone(), output.clone(), false, true),
        (
            named("test.other.reader", 1, "ReadRequest"),
            output.clone(),
            true,
            false,
        ),
        (
            input.clone(),
            named("nepl3.reader", 2, "ReadReply"),
            true,
            false,
        ),
        (TypeDescriptor::Unit, output.clone(), true, false),
        (input.clone(), TypeDescriptor::Unit, true, false),
        (
            named("nepl3.reader", 1, "TransformRequest"),
            output.clone(),
            true,
            false,
        ),
        (
            TypeDescriptor::Option(Box::new(input.clone())),
            output.clone(),
            true,
            false,
        ),
        (
            input.clone(),
            TypeDescriptor::List(Box::new(output.clone())),
            true,
            false,
        ),
    ];
    for (index, (input, output, pure, accepted)) in cases.into_iter().enumerate() {
        let mut r = SchemaRegistry::default();
        for d in [
            nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?,
            nepl3_reader::schema::descriptor(&mut b()).map_err(err)?,
        ] {
            r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
                .map_err(err)?;
        }
        for (package, revision) in [("test.other.reader", 1), ("nepl3.reader", 2)] {
            let d = SchemaDescriptor {
                package: package.into(),
                revision,
                types: ["ReadRequest", "ReadReply"]
                    .into_iter()
                    .map(|name| NamedType {
                        name: name.into(),
                        shape: TypeShape::Record { fields: vec![] },
                        constraints: vec![],
                    })
                    .collect(),
                operations: vec![],
            };
            r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
                .map_err(err)?;
        }
        let descriptor = SchemaDescriptor {
            package: format!("test.envelope.{index}"),
            revision: 1,
            types: vec![],
            operations: vec![OperationDescriptor {
                name: "custom-read".into(),
                input,
                output,
                pure,
            }],
        };
        let schema = descriptor.reference(&mut b()).map_err(err)?;
        r.register(schema.clone(), descriptor, &mut b())
            .map_err(err)?;
        r.finalize(&mut b()).map_err(err)?;
        let make = |budget: &mut Budget| {
            NativeHost::new(
                &r,
                Digest::of(b"envelope-test"),
                "envelope:".into(),
                vec![NativeReader {
                    operation: OperationRef {
                        schema: schema.clone(),
                        name: "custom-read".into(),
                    },
                    read: first,
                }],
                budget,
            )
        };
        let mut budget = b();
        let result = make(&mut budget);
        if accepted {
            let host = result.map_err(err)?;
            assert_eq!(host.providers().len(), 1);
            // With one provider, no later Work charge follows the envelope
            // comparison. One less than success stops in the output check.
            let mut limited = Budget::new(Limits {
                work: budget.usage().work - 1,
                ..b().limits()
            });
            assert!(matches!(
                make(&mut limited),
                Err(ParseError::Stopped(
                    nepl3_core::budget::StopReason::WorkLimit
                ))
            ));
            assert_eq!(limited.usage().allocation_units, 0);
            assert_eq!(
                limited.poll(),
                Err(nepl3_core::budget::StopReason::WorkLimit)
            );
            let mut cancelled = b();
            cancelled.cancel();
            let before = cancelled.usage();
            assert!(matches!(
                make(&mut cancelled),
                Err(ParseError::Stopped(
                    nepl3_core::budget::StopReason::Cancelled
                ))
            ));
            assert_eq!(cancelled.usage(), before);
        } else {
            assert!(matches!(result, Err(ParseError::Context)), "case {index}");
            assert_eq!(budget.usage().allocation_units, 0);
        }
    }
    Ok(())
}
