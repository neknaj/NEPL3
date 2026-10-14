use super::{budget, load};
use nepl3_core::{budget::*, schema::*, source::*, value::*};
use nepl3_grammar_core::{artifact, compile::package::CompiledLanguage};
use nepl3_wire::foundation::FoundationCodec;
fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn registry(b: &mut Budget) -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(b),
        nepl3_reader::schema::descriptor(b),
        nepl3_engine::schema::descriptor(b),
        nepl3_grammar_core::schema::descriptor(b),
    ] {
        let d = d.map_err(error)?;
        r.register(d.reference(b).map_err(error)?, d, b)
            .map_err(error)?;
    }
    r.finalize(b).map_err(error)?;
    Ok(r)
}
fn compiled() -> Result<CompiledLanguage, String> {
    nepl3_tools::bootstrap::catalog::compile(
        &load("examples/grammar/binding.neplg")?,
        "test.artifact",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
fn encoded(c: &CompiledLanguage, b: &mut Budget) -> Result<NdfValue, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let checked = c
        .package
        .check_with_admission(&c.registry, b, &mut admission)
        .map_err(error)?;
    let mut codec = FoundationCodec::new(&c.registry, &store, &mut admission).map_err(error)?;
    artifact::to_value(&checked, &mut codec, b).map_err(error)
}
fn receive(
    raw: &NdfValue,
    expected: &SchemaRef,
    mut r: SchemaRegistry,
    b: &mut Budget,
) -> Result<CompiledLanguage, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let (d, pending) = {
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(error)?;
        artifact::prepare_received(raw, expected, &r, &mut codec, b).map_err(error)?
    };
    // Nothing escapes this owned transaction if registration or payload checking fails.
    let reference = d.reference(b).map_err(error)?;
    r.register(reference, d, b).map_err(error)?;
    r.finalize(b).map_err(error)?;
    let package = {
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(error)?;
        pending.decode(&r, &mut codec, b).map_err(error)?
    };
    Ok(CompiledLanguage {
        package,
        registry: r,
    })
}
#[test]
fn compiled_artifact_bootstraps_exact_surface_without_ambient_sources() -> Result<(), String> {
    let c = compiled()?;
    let value = encoded(&c, &mut budget())?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
    let cold = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let r = registry(&mut budget())?;
    assert!(r.selected("test.artifact", 1).is_none());
    let mut b = budget();
    let rebuilt = receive(&cold, &c.package.schema, r, &mut b)?;
    assert_eq!(rebuilt.package, c.package);
    assert_eq!(
        b.usage().source_bytes,
        c.package
            .provenance
            .sources
            .iter()
            .map(|s| s.text().len() as u64)
            .sum::<u64>()
    );
    assert_eq!(encoded(&rebuilt, &mut budget())?, value);
    Ok(())
}
#[test]
fn compiled_artifact_rejects_identity_conflicts_and_incomplete_payloads() -> Result<(), String> {
    let c = compiled()?;
    let value = encoded(&c, &mut budget())?;
    let mut wrong = c.package.schema.clone();
    wrong.digest = Digest::of(b"wrong surface");
    assert!(receive(&value, &wrong, registry(&mut budget())?, &mut budget()).is_err());
    let mut conflict = registry(&mut budget())?;
    let mut d = c
        .registry
        .descriptor(&c.package.schema)
        .ok_or("surface")?
        .clone();
    d.types[0].name.push_str("Changed");
    conflict
        .register(d.reference(&mut budget()).map_err(error)?, d, &mut budget())
        .map_err(error)?;
    conflict.finalize(&mut budget()).map_err(error)?;
    assert!(receive(&value, &c.package.schema, conflict, &mut budget()).is_err());
    let mut missing = value.clone();
    let NdfValue::Record(envelope) = &mut missing else {
        return Err("artifact".into());
    };
    let NdfValue::Record(package) = &mut envelope.fields[1] else {
        return Err("package".into());
    };
    let NdfValue::Record(provenance) = &mut package.fields[13] else {
        return Err("provenance".into());
    };
    provenance.fields[0] = NdfValue::List(Vec::new());
    assert!(
        receive(
            &missing,
            &c.package.schema,
            registry(&mut budget())?,
            &mut budget()
        )
        .is_err()
    );
    let mut malformed = value.clone();
    let NdfValue::Record(envelope) = &mut malformed else {
        return Err("artifact".into());
    };
    envelope.fields[0] = NdfValue::Unit;
    assert!(
        receive(
            &malformed,
            &c.package.schema,
            registry(&mut budget())?,
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn compiled_artifact_stages_do_not_admit_missing_dependencies_or_package_identity()
-> Result<(), String> {
    let c = compiled()?;
    let value = encoded(&c, &mut budget())?;
    let mut descriptor = c
        .registry
        .descriptor(&c.package.schema)
        .ok_or("surface")?
        .clone();
    descriptor.types.push(NamedType {
        name: "ExternalPayload".into(),
        shape: TypeShape::Record {
            fields: vec![FieldDescriptor {
                name: "value".into(),
                ty: TypeDescriptor::Named(TypeRef {
                    package: "missing.payload".into(),
                    revision: 1,
                    name: "Value".into(),
                }),
            }],
        },
        constraints: Vec::new(),
    });
    let expected = descriptor.reference(&mut budget()).map_err(error)?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.registry, &store, &mut a).map_err(error)?;
    use nepl3_core::value_codec::FoundationValueCodec;
    let mut missing = value.clone();
    let NdfValue::Record(envelope) = &mut missing else {
        return Err("artifact".into());
    };
    envelope.fields[0] = codec
        .encode_schema_descriptor(&descriptor, &mut budget())
        .map_err(error)?;
    assert!(receive(&missing, &expected, registry(&mut budget())?, &mut budget()).is_err());
    // A valid second surface descriptor still cannot accompany the original package.
    descriptor.types.pop();
    descriptor.package = "test.another".into();
    let other = descriptor.reference(&mut budget()).map_err(error)?;
    let mut mismatch = value.clone();
    let NdfValue::Record(envelope) = &mut mismatch else {
        return Err("artifact".into());
    };
    envelope.fields[0] = codec
        .encode_schema_descriptor(&descriptor, &mut budget())
        .map_err(error)?;
    let mut r = registry(&mut budget())?;
    let original = c
        .registry
        .descriptor(&c.package.schema)
        .ok_or("surface")?
        .clone();
    r.register(c.package.schema.clone(), original, &mut budget())
        .map_err(error)?;
    r.finalize(&mut budget()).map_err(error)?;
    let failure = receive(&mismatch, &other, r, &mut budget())
        .err()
        .ok_or("identity accepted")?;
    assert!(failure.contains("IdentityMismatch"), "{failure}");
    let mut forged = value;
    let NdfValue::Record(envelope) = &mut forged else {
        return Err("artifact".into());
    };
    envelope.schema.digest = Digest::of(b"forged grammar");
    assert!(
        receive(
            &forged,
            &c.package.schema,
            registry(&mut budget())?,
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}
#[test]
fn compiled_artifact_receipt_preserves_whole_transaction_limits() -> Result<(), String> {
    let c = compiled()?;
    let value = encoded(&c, &mut budget())?;
    let mut measured = budget();
    receive(
        &value,
        &c.package.schema,
        registry(&mut budget())?,
        &mut measured,
    )?;
    let usage = measured.usage();
    for resource in [
        Resource::Work,
        Resource::AllocationUnits,
        Resource::SourceBytes,
        Resource::Nodes,
    ] {
        let used = match resource {
            Resource::Work => usage.work,
            Resource::AllocationUnits => usage.allocation_units,
            Resource::SourceBytes => usage.source_bytes,
            Resource::Nodes => usage.nodes,
            _ => return Err("resource".into()),
        };
        assert!(used > 0);
        for (limit, success) in [(used, true), (used - 1, false)] {
            let mut limits = budget().limits();
            match resource {
                Resource::Work => limits.work = limit,
                Resource::AllocationUnits => limits.allocation_units = limit,
                Resource::SourceBytes => limits.source_bytes = limit,
                Resource::Nodes => limits.nodes = limit,
                _ => return Err("resource".into()),
            }
            let mut b = Budget::new(limits);
            let result = receive(&value, &c.package.schema, registry(&mut budget())?, &mut b);
            assert_eq!(
                result.is_ok(),
                success,
                "{resource:?}: {:?}",
                result.as_ref().err()
            );
            if !success {
                assert!(b.poll().is_err());
            }
        }
    }
    for (depth, success) in [(usage.depth, true), (usage.depth - 1, false)] {
        let mut b = Budget::new(Limits {
            depth,
            ..budget().limits()
        });
        assert_eq!(
            receive(&value, &c.package.schema, registry(&mut budget())?, &mut b).is_ok(),
            success
        );
    }
    // Cancellation after descriptor preparation must survive the codec rebind.
    let mut r = registry(&mut budget())?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut staged = budget();
    let (d, pending) = {
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(error)?;
        artifact::prepare_received(&value, &c.package.schema, &r, &mut codec, &mut staged)
            .map_err(error)?
    };
    r.register(d.reference(&mut staged).map_err(error)?, d, &mut staged)
        .map_err(error)?;
    r.finalize(&mut staged).map_err(error)?;
    staged.cancel();
    let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(error)?;
    assert!(matches!(
        pending.decode(&r, &mut codec, &mut staged),
        Err(nepl3_engine::portable::PortableError::Stopped(
            StopReason::Cancelled
        ))
    ));
    let mut b = budget();
    b.cancel();
    assert!(receive(&value, &c.package.schema, registry(&mut budget())?, &mut b).is_err());
    Ok(())
}
