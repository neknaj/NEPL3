//! A host-selected projection crosses NDF before a separately authorized call.
use external_hello_language::{budget, error};
use nepl3_core::{
    budget::{Budget, StopReason},
    diagnostic::{OperationResult, Report},
    operation::Invoke,
    origin::{Origin, OriginId},
    schema::*,
    source::{Digest, SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    syntax::{Environment, EnvironmentBinding, NamespaceRef, ResourceContent},
    value::*,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_suite::{
    dispatch,
    environment::*,
    grants::{GrantError, Grants},
};
use nepl3_wire::foundation::FoundationCodec;

fn fixture() -> Result<(SchemaRegistry, SchemaRef), String> {
    let ty = TypeDescriptor::Named(TypeRef {
        package: "org.example.environment".into(),
        revision: 1,
        name: "Number".into(),
    });
    let descriptor = SchemaDescriptor {
        package: "org.example.environment".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Number".into(),
            constraints: vec![],
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::U64,
                }],
            },
        }],
        operations: vec![OperationDescriptor {
            name: "selected".into(),
            input: ty.clone(),
            output: ty,
            pure: true,
        }],
    };
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    let mut registry = SchemaRegistry::default();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(error)?;
    registry
        .register(
            foundation.reference(&mut budget()).map_err(error)?,
            foundation,
            &mut budget(),
        )
        .map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    Ok((registry, schema))
}

fn typed(value: NdfValue) -> Result<TypedValue, String> {
    match &value {
        NdfValue::Record(record) => Ok(TypedValue::Record(record.clone())),
        _ => Err("expected environment entry record".into()),
    }
}

fn selected(
    call: &Invoke,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationResult<TypedValue>, StopReason> {
    let result = (|| -> Result<TypedValue, nepl3_wire::WireError> {
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(registry, &sources, &mut admission)?;
        let value = match call.environment.clone_with_budget(b)? {
            TypedValue::Record(record) => NdfValue::Record(record),
            TypedValue::Variant(variant) => NdfValue::Variant(variant),
        };
        let entry = codec.decode_environment(&value, b)?;
        let binding = entry
            .value
            .bindings
            .first()
            .ok_or(nepl3_wire::WireError::InvalidType)?;
        Ok(binding.value.clone_with_budget(b)?)
    })();
    match result {
        Ok(value) => Ok(OperationResult::Complete {
            value,
            report: Report::default(),
        }),
        Err(e) => match e.stop_reason() {
            Some(reason) => Err(reason),
            None => Ok(OperationResult::Invalid {
                partial: None,
                report: Report::default(),
            }),
        },
    }
}

#[test]
fn projected_environment_crosses_ndf_and_host_grants_before_dispatch() -> Result<(), String> {
    let (registry, schema) = fixture()?;
    let snapshot = SourceSnapshot::new(
        SourceId("input".into()),
        1,
        "memory:input".into(),
        "値".as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let origins = [Origin::Direct(snapshot.span(0, 3).map_err(error)?)];
    let mut sources = SourceStore::default();
    sources
        .insert_with_budget(snapshot.clone(), &mut budget())
        .map_err(error)?;
    let namespace = NamespaceRef {
        schema: schema.clone(),
        name: "host".into(),
    };
    let number = |n| {
        TypedValue::Record(Record {
            schema: schema.clone(),
            kind: "Number".into(),
            fields: vec![NdfValue::U64(n)],
        })
    };
    let resource = ResourceContent {
        id: "asset".into(),
        digest: Digest::of(b"asset"),
        bytes: b"asset".to_vec(),
    };
    let source = Environment {
        bindings: vec![
            EnvironmentBinding {
                namespace: namespace.clone(),
                name: "answer".into(),
                value: number(41),
                origin: Some(OriginId(0)),
            },
            EnvironmentBinding {
                namespace: namespace.clone(),
                name: "private".into(),
                value: number(99),
                origin: None,
            },
        ],
        resources: vec![resource.clone()],
    };
    let proof = source
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(error)?;
    let guest = NamespaceRef {
        schema: schema.clone(),
        name: "guest".into(),
    };
    let expected = TypeDescriptor::Named(TypeRef {
        package: schema.package.clone(),
        revision: 1,
        name: "Number".into(),
    });
    let selection = [BindingProjection {
        namespace: &namespace,
        name: "answer",
        target_namespace: &guest,
        target_name: "selected",
        expected: &expected,
    }];
    let projected = project(&proof, &selection, &["asset"], &mut budget()).map_err(error)?;
    let before = projected.value().bindings.as_ptr();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(error)?;
    let published = projected
        .publish(7, &mut codec, &mut budget())
        .map_err(error)?;
    // Publication moves the allocated selection and keeps the exact arena.
    assert_eq!(published.entry().value.bindings.as_ptr(), before);
    assert!(core::ptr::eq(published.origins(), origins.as_slice()));
    assert!(core::ptr::eq(published.sources(), &sources));
    assert_eq!(published.entry().value.bindings.len(), 1);
    assert_eq!(published.entry().value.bindings[0].namespace, guest);
    assert_eq!(published.entry().value.bindings[0].value, number(41));
    let encoded = codec
        .encode_environment(published.entry(), &mut budget())
        .map_err(error)?;
    let approved_environment = typed(encoded.clone())?;
    let request = Invoke {
        request_id: 1,
        operation: OperationRef {
            schema: schema.clone(),
            name: "selected".into(),
        },
        input: number(0),
        environment: typed(encoded)?,
        sources: vec![snapshot],
        resources: vec![resource.clone()],
        limits: budget().limits(),
    };
    let bytes = nepl3_wire::operation::encode_invoke(
        &request,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    let received = nepl3_wire::operation::decode_invoke(
        &bytes,
        &registry,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(error)?;
    let value = match &received.environment {
        TypedValue::Record(r) => NdfValue::Record(r.clone()),
        _ => return Err("entry".into()),
    };
    let decoded = codec
        .decode_environment(&value, &mut budget())
        .map_err(error)?;
    let mut corrupted_wire = value.clone();
    if let NdfValue::Record(record) = &mut corrupted_wire {
        record.fields[1] = NdfValue::Bytes(vec![0; 32]);
    }
    assert!(
        codec
            .decode_environment(&corrupted_wire, &mut budget())
            .is_err()
    );
    decoded
        .value
        .validate(&origins, &sources, &registry, &mut budget())
        .map_err(error)?;
    assert!(
        decoded
            .value
            .validate(&[], &sources, &registry, &mut budget())
            .is_err()
    );
    assert!(
        decoded
            .value
            .validate(&origins, &SourceStore::default(), &registry, &mut budget())
            .is_err()
    );
    let grants = Grants::new(
        &approved_environment,
        &sources,
        std::slice::from_ref(&resource),
        &mut budget(),
    )
    .map_err(error)?;
    let approved = grants.admit(&received, &mut budget()).map_err(error)?;
    let implementation = Digest::of(b"selected implementation");
    let registrations = [dispatch::Registration {
        operation: &request.operation,
        implementation,
        invoke: selected,
    }];
    let outcome = dispatch::invoke_terminal(
        &registrations,
        &request.operation,
        implementation,
        approved.request(),
        &registry,
        &sources,
        &mut budget(),
        &mut budget(),
    )
    .map_err(error)?;
    // The callback observes only the explicitly selected value, not private=99.
    let OperationResult::Complete { value, .. } = outcome else {
        return Err("complete".into());
    };
    assert_eq!(value, number(41));
    let empty = SourceStore::default();
    let denied_source = Grants::new(
        &approved_environment,
        &empty,
        std::slice::from_ref(&resource),
        &mut budget(),
    )
    .map_err(error)?;
    assert!(matches!(
        denied_source.admit(&received, &mut budget()),
        Err(GrantError::Source)
    ));
    let denied_resource =
        Grants::new(&approved_environment, &sources, &[], &mut budget()).map_err(error)?;
    assert!(matches!(
        denied_resource.admit(&received, &mut budget()),
        Err(GrantError::Resource)
    ));
    let mut corrupted = published.entry().clone();
    corrupted.digest = Digest::of(b"wrong");
    assert!(codec.encode_environment(&corrupted, &mut budget()).is_err());
    let mut changed = received.clone();
    changed.environment = number(99);
    assert!(matches!(
        grants.admit(&changed, &mut budget()),
        Err(GrantError::Environment)
    ));
    for (work, allocation, output, reason) in [
        (0, u64::MAX, u64::MAX, StopReason::WorkLimit),
        (1, u64::MAX, u64::MAX, StopReason::WorkLimit),
        (32, u64::MAX, u64::MAX, StopReason::WorkLimit),
        (u64::MAX, 0, u64::MAX, StopReason::AllocationLimit),
        (u64::MAX, 128, u64::MAX, StopReason::AllocationLimit),
        (u64::MAX, u64::MAX, 0, StopReason::OutputLimit),
    ] {
        let projected = project(&proof, &selection, &[], &mut budget()).map_err(error)?;
        let mut limits = budget().limits();
        limits.work = work;
        limits.allocation_units = allocation;
        limits.output_bytes = output;
        let mut stopped = Budget::new(limits);
        let error = match projected.publish(7, &mut codec, &mut stopped) {
            Err(error) => error,
            Ok(_) => return Err("must stop".into()),
        };
        assert_eq!(error.stop_reason(), Some(reason));
        assert_eq!(source.bindings[0].value, number(41));
    }
    Ok(())
}
