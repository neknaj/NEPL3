use super::*;
use nepl3_core::{budget::StopReason, value::OperationRef};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn extension_schema_admission_meters_visited_prefix_and_retains_subject() -> Result<(), String> {
    let (template, original) = fixture()?;
    let mut baseline = None;
    for (count, width, suffix) in [
        (0, 0, false),
        (8, 0, false),
        (8, 128, false),
        (8, 128, true),
    ] {
        let mut package = template.clone();
        let target = SchemaDescriptor {
            package: "test.extension".into(),
            revision: 1,
            types: vec![],
            operations: vec![OperationDescriptor {
                name: "available".into(),
                input: TypeDescriptor::Unit,
                output: TypeDescriptor::Unit,
                pure: true,
            }],
        };
        let reference = target.reference(&mut budget()).map_err(err)?;
        package.extensions = vec![ExtensionRequirement {
            alias: "probe".into(),
            provider: "probe.provider".into(),
            signature: "probe/v1".into(),
            operation: OperationRef {
                schema: reference.clone(),
                name: "missing".into(),
            },
            input: TypeDescriptor::Unit,
            output: TypeDescriptor::Unit,
            pure: true,
        }];
        let mut descriptors = Vec::new();
        for name in [
            "nepl3.foundation",
            "nepl3.reader",
            "nepl3.engine",
            package.schema.package.as_str(),
        ] {
            let r = original.selected(name, 1).ok_or("fixture schema")?;
            descriptors.push(original.descriptor(r).ok_or("fixture descriptor")?.clone());
        }
        if suffix {
            descriptors.push(target.clone());
        }
        for i in 0..count {
            descriptors.push(SchemaDescriptor {
                package: format!("padding.{i}{}", "x".repeat(width)),
                revision: 1,
                types: vec![],
                operations: vec![],
            });
        }
        if !suffix {
            descriptors.push(target);
        }
        let mut registry = SchemaRegistry::default();
        let mut fee = 1;
        let mut found = false;
        for d in descriptors {
            if !found {
                fee += (d.package.len() + reference.package.len() + 9) as u64;
                found = d.package == reference.package;
            }
            registry
                .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        fee += reference.package.len() as u64 + 41;
        registry.finalize(&mut budget()).map_err(err)?;
        let mut b = budget();
        let failure = package
            .check_detailed(&registry, &mut b)
            .err()
            .ok_or("expected missing operation")?;
        assert_eq!(failure.error, PackageError::MissingExtension);
        assert_eq!(failure.subject, Some(PackageSubject::Extension(0)));
        // Earlier package/reader lookups resolve among the first four owners.
        // Remove the independently calculated lookup fee and the existing one-unit
        // operation-table precharge; all other
        // historical Usage fields must be invariant under padding placement.
        let mut prefix = b.usage();
        prefix.work = prefix
            .work
            .checked_sub(fee + 1)
            .ok_or("missing admission charge")?;
        if let Some(expected) = baseline {
            assert_eq!(prefix, expected);
        } else {
            baseline = Some(prefix);
        }
        let mut limits = budget().limits();
        limits.work = prefix.work + fee - 1;
        let mut stopped = Budget::new(limits);
        let failure = package
            .check_detailed(&registry, &mut stopped)
            .err()
            .ok_or("expected stop")?;
        assert_eq!(failure.error, PackageError::Stopped(StopReason::WorkLimit));
        assert_eq!(failure.subject, Some(PackageSubject::Extension(0)));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        let mut expected = prefix;
        expected.work += fee - reference.package.len() as u64 - 41;
        assert_eq!(stopped.usage(), expected);
        package.extensions[0].operation.name = "available".into();
        package.check(&registry, &mut budget()).map_err(err)?;
        // A matching operation succeeds with the real identity. A forged
        // identity must fail before operation-table admission, so an absent
        // operation cannot mask an erroneously accepted descriptor here.
        package.extensions[0].operation.schema.digest.0[0] ^= 1;
        let mut b = budget();
        let failure = package
            .check_detailed(&registry, &mut b)
            .err()
            .ok_or("expected mismatched schema")?;
        assert_eq!(failure.error, PackageError::MissingExtension);
        assert_eq!(failure.subject, Some(PackageSubject::Extension(0)));
        expected.work = prefix.work + fee;
        assert_eq!(b.usage(), expected);
    }
    Ok(())
}
