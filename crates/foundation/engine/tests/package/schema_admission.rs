use super::*;
use nepl3_core::{
    budget::{StopReason, Usage},
    value::SchemaRef,
};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn descriptor(name: &str) -> SchemaDescriptor {
    SchemaDescriptor {
        package: name.into(),
        revision: 1,
        types: vec![],
        operations: vec![],
    }
}
fn fee(names: &[String], query: &SchemaRef) -> u64 {
    let mut fee = 1;
    for name in names {
        fee += (name.len() + query.package.len() + 9) as u64;
        if name == &query.package {
            return fee + query.package.len() as u64 + 41;
        }
    }
    fee
}
#[test]
fn package_surface_and_payload_schema_admission_preserves_stop_and_error_subject()
-> Result<(), String> {
    for payload in [false, true] {
        for suffix in [false, true] {
            let (mut package, _) = fixture()?;
            let surface = descriptor("test.surface");
            package.schema = surface.reference(&mut budget()).map_err(err)?;
            let target = descriptor("test.payload");
            let target_ref = target.reference(&mut budget()).map_err(err)?;
            package.payload_schemas = if payload { vec![target_ref] } else { vec![] };
            // Reader still belongs to the original fixture: after all owner
            // lookups succeed the first later check is KindShape, without I/O.
            let mut registry = SchemaRegistry::default();
            let padding = descriptor(&"padding".repeat(128));
            let targets = if payload {
                vec![surface, target]
            } else {
                vec![surface]
            };
            let descriptors: Vec<_> = if suffix {
                targets
                    .into_iter()
                    .chain(core::iter::once(padding))
                    .collect()
            } else {
                core::iter::once(padding).chain(targets).collect()
            };
            let names: Vec<_> = descriptors.iter().map(|d| d.package.clone()).collect();
            for d in descriptors {
                registry
                    .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                    .map_err(err)?;
            }
            registry.finalize(&mut budget()).map_err(err)?;
            let base = 2 + fee(&names, &package.schema);
            let full = base
                + package
                    .payload_schemas
                    .first()
                    .map_or(0, |s| 1 + fee(&names, s));
            for prior in [0, 7] {
                let mut b = budget();
                b.charge(nepl3_core::budget::Resource::Work, prior)
                    .map_err(err)?;
                let failure = package
                    .check_detailed(&registry, &mut b)
                    .err()
                    .ok_or("expected KindShape")?;
                assert_eq!(failure.error, PackageError::KindShape);
                assert_eq!(failure.subject, None);
                assert_eq!(
                    b.usage(),
                    Usage {
                        work: prior + full,
                        ..Usage::default()
                    }
                );
                let mut limits = budget().limits();
                limits.work = prior + full - 1;
                let mut b = Budget::new(limits);
                b.charge(nepl3_core::budget::Resource::Work, prior)
                    .map_err(err)?;
                let failure = package
                    .check_detailed(&registry, &mut b)
                    .err()
                    .ok_or("expected stop")?;
                assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
                assert_eq!(failure.subject, None);
                assert_eq!(b.poll(), Err(StopReason::WorkLimit));
                let last = package.payload_schemas.last().unwrap_or(&package.schema);
                assert_eq!(
                    b.usage(),
                    Usage {
                        work: prior + full - last.package.len() as u64 - 41,
                        ..Usage::default()
                    }
                );
            }
            let mut forged = package.clone();
            let query = if payload {
                &mut forged.payload_schemas[0]
            } else {
                &mut forged.schema
            };
            query.digest.0[0] ^= 1;
            let mut b = budget();
            let failure = forged
                .check_detailed(&registry, &mut b)
                .err()
                .ok_or("expected digest mismatch")?;
            assert_eq!(
                failure.error,
                PackageError::Schema(SchemaError::UnknownSchema)
            );
            assert_eq!(failure.subject, None);
            assert_eq!(
                b.usage(),
                Usage {
                    work: full,
                    ..Usage::default()
                }
            );
            let mut absent = package.clone();
            let query = if payload {
                &mut absent.payload_schemas[0]
            } else {
                &mut absent.schema
            };
            query.package = "absent".into();
            let missing = if payload {
                base + 1 + fee(&names, query)
            } else {
                2 + fee(&names, query)
            };
            let mut b = budget();
            let failure = absent
                .check_detailed(&registry, &mut b)
                .err()
                .ok_or("expected missing")?;
            assert_eq!(
                failure.error,
                PackageError::Schema(SchemaError::UnknownSchema)
            );
            assert_eq!(failure.subject, None);
            assert_eq!(
                b.usage(),
                Usage {
                    work: missing,
                    ..Usage::default()
                }
            );
            let mut b = budget();
            b.cancel();
            let failure = package
                .check_detailed(&registry, &mut b)
                .err()
                .ok_or("expected cancellation")?;
            assert_eq!(failure.error, PackageError::Stopped(StopReason::Cancelled));
            assert_eq!(b.usage(), Usage::default());
            assert_eq!(failure.subject, None);
            assert_eq!(b.poll(), Err(StopReason::Cancelled));
        }
    }
    Ok(())
}
