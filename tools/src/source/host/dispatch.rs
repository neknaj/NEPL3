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
