use super::*;
use nepl3_core::{schema::*, value::*};
use nepl3_reader::plan::{ProviderKind, ProviderSignature};
fn reader_type(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}
#[test]
fn unregistered_unused_reader_is_rejected_before_doc_parse_on_both_routes() -> Result<(), String> {
    let mut compiled = compiled()?;
    let descriptor = SchemaDescriptor {
        package: "test.unregistered.doc.reader".into(),
        revision: 1,
        types: vec![],
        operations: vec![OperationDescriptor {
            name: "phantom".into(),
            input: reader_type("ReadRequest"),
            output: reader_type("ReadReply"),
            pure: true,
        }],
    };
    let schema = descriptor.reference(&mut budget()).map_err(err)?;
    compiled
        .doc
        .registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(err)?;
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    compiled.doc.package.payload_schemas.push(schema.clone());
    compiled
        .doc
        .package
        .reader
        .providers
        .push(ProviderSignature {
            operation: OperationRef {
                schema,
                name: "phantom".into(),
            },
            kind: ProviderKind::Read,
            value_input: TypeDescriptor::Unit,
            value_output: TypeDescriptor::Text,
            pure: true,
            state_type: TypeDescriptor::Unit,
            continuation_type: reader_type("ReaderContinuation"),
        });
    compiled
        .doc
        .package
        .check(&compiled.doc.registry, &mut budget())
        .map_err(err)?;
    for native in [false, true] {
        let mut finished = false;
        let result = with_input_route(
            native,
            &compiled,
            "article ja \"ok\" body nil",
            "Article",
            |_, _, _, _| {
                finished = true;
                Ok(())
            },
        );
        assert!(
            matches!(result,Err(ref e) if e.contains("MissingProvider")),
            "unexpected result: {result:?}"
        );
        assert!(!finished);
    }
    Ok(())
}
#[test]
fn actual_doc_catalog_supports_both_routes_and_rejects_changed_implementation() -> Result<(), String>
{
    let compiled = compiled()?;
    let host = crate::doc::host::native(
        &compiled.doc.registry,
        host_identity(),
        "profile-test:".into(),
        &mut budget(),
    )
    .map_err(err)?;
    assert_eq!(host.providers().len(), 5);
    let mut exported = host.provider_catalog(&mut budget()).map_err(err)?;
    assert_eq!(exported, host.providers());
    let first = exported[0].clone();
    exported[0].provider.push('x');
    exported[0].operations[0].name.push('x');
    assert_eq!(host.providers()[0], first);
    for native in [false, true] {
        with_input_route(
            native,
            &compiled,
            "article ja \"Host\" body cons paragraph cons \"Body\" nil cons display Math add 1 2 nil",
            "Article",
            |_, resolved, _, _| {
                for requirement in &resolved.profile().providers {
                    assert!(
                        host.providers()
                            .iter()
                            .any(|p| p.provider == requirement.provider
                                && p.revision == requirement.revision
                                && p.implementation_digest == requirement.implementation_digest
                                && p.operations.contains(&requirement.operation))
                    );
                }
                let mut catalog = host.provider_catalog(&mut budget()).map_err(err)?;
                for p in &mut catalog {
                    p.implementation_digest = Digest::of(b"different executable");
                }
                let packages = std::iter::once(&compiled.doc.package)
                    .chain(compiled.others.iter())
                    .collect::<Vec<_>>();
                let profile = resolved.profile().clone();
                assert!(matches!(
                    profile.resolve(
                        &RuntimeCatalog {
                            packages: &packages,
                            providers: &catalog,
                            resources: &[]
                        },
                        &compiled.doc.registry,
                        &mut budget()
                    ),
                    Err(ProfileError::ProviderIdentity)
                ));
                Ok(())
            },
        )?;
    }
    Ok(())
}
#[test]
fn doc_host_preparation_and_catalog_copy_stop_without_finishing_parse() -> Result<(), String> {
    let compiled = compiled()?;
    for native in [false, true] {
        for (limits, reason) in [
            (
                Limits {
                    work: 0,
                    ..budget().limits()
                },
                "WorkLimit",
            ),
            (
                Limits {
                    allocation_units: 0,
                    ..budget().limits()
                },
                "AllocationLimit",
            ),
        ] {
            let mut finished = false;
            let result = with_named_input_limits(
                native,
                &compiled,
                "article ja \"stopped\" body nil",
                "stopped-doc",
                "Article",
                limits,
                |_, _, _, _| {
                    finished = true;
                    Ok(())
                },
            );
            assert!(matches!(result,Err(ref e) if e.contains(reason)));
            assert!(!finished);
        }
    }
    let host = crate::doc::host::native(
        &compiled.doc.registry,
        host_identity(),
        "catalog-test:".into(),
        &mut budget(),
    )
    .map_err(err)?;
    for (limits, reason) in [
        (
            Limits {
                work: 0,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut b = Budget::new(limits);
        assert!(
            matches!(host.provider_catalog(&mut b),Err(nepl3_engine::parse::ParseError::Stopped(e)) if e==reason)
        );
        let prefix = b.usage();
        assert_eq!(b.poll(), Err(reason));
        assert!(host.provider_catalog(&mut b).is_err());
        assert_eq!(b.usage(), prefix);
    }
    let mut measured = budget();
    let expected_catalog = host.provider_catalog(&mut measured).map_err(err)?;
    let expected_work = 1
        + expected_catalog.len() as u64
        + expected_catalog
            .iter()
            .map(|p| {
                p.provider.len() as u64
                    + 41
                    + p.operations
                        .iter()
                        .map(|o| (o.name.len() + o.schema.package.len()) as u64 + 41)
                        .sum::<u64>()
            })
            .sum::<u64>();
    let expected_allocation = (expected_catalog.len()
        * core::mem::size_of::<ProviderImplementation>()) as u64
        + expected_catalog
            .iter()
            .map(|p| {
                (p.provider.len()
                    + p.operations.len() * core::mem::size_of::<OperationRef>()
                    + p.operations
                        .iter()
                        .map(|o| o.name.len() + o.schema.package.len())
                        .sum::<usize>()) as u64
            })
            .sum::<u64>();
    assert_eq!(measured.usage().work, expected_work);
    assert_eq!(measured.usage().allocation_units, expected_allocation);
    for (limits, reason) in [
        (
            Limits {
                work: expected_work - 1,
                ..budget().limits()
            },
            StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: expected_allocation - 1,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
    ] {
        let mut limited = Budget::new(limits);
        assert!(
            matches!(host.provider_catalog(&mut limited),Err(nepl3_engine::parse::ParseError::Stopped(e)) if e==reason)
        );
        assert!(limited.usage().work > 0);
        assert!(limited.usage().allocation_units > 0);
        let prefix = limited.usage();
        assert!(host.provider_catalog(&mut limited).is_err());
        assert_eq!(limited.usage(), prefix);
        assert_eq!(host.providers(), expected_catalog);
    }
    let mut b = budget();
    b.charge(Resource::Work, 17).map_err(err)?;
    b.stop(StopReason::Cancelled);
    let prefix = b.usage();
    assert!(matches!(
        host.provider_catalog(&mut b),
        Err(nepl3_engine::parse::ParseError::Stopped(
            StopReason::Cancelled
        ))
    ));
    assert_eq!(b.usage(), prefix);
    assert_eq!(host.providers().len(), 5);
    Ok(())
}
