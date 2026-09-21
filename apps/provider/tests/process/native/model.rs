use super::*;
pub fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
pub fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        work: 10_000_000,
        depth: 128,
        nodes: 100_000,
        allocation_units: 10_000_000,
        output_bytes: 100_000,
        diagnostics: 100,
        events: 100,
    })
}
pub fn identity() -> Digest {
    Digest::of(b"process conformance increment provider")
}
pub fn context(call: &Invoke, registry: &SchemaRegistry) -> Result<Digest, String> {
    nepl3_wire::operation::context_digest(call, identity(), registry, &mut budget()).map_err(error)
}
pub fn fixture() -> Result<(SchemaRegistry, Invoke), String> {
    let mut registry = SchemaRegistry::default();
    let foundation = foundation::descriptor(&mut budget()).map_err(error)?;
    registry
        .register(
            foundation.reference(&mut budget()).map_err(error)?,
            foundation,
            &mut budget(),
        )
        .map_err(error)?;
    let ty = TypeDescriptor::Named(TypeRef {
        package: "test.process".into(),
        revision: 1,
        name: "Number".into(),
    });
    let descriptor = SchemaDescriptor {
        package: "test.process".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Number".into(),
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: TypeDescriptor::U64,
                }],
            },
            constraints: vec![],
        }],
        operations: ["dependent", "increment"]
            .into_iter()
            .map(|name| OperationDescriptor {
                name: name.into(),
                input: ty.clone(),
                output: ty.clone(),
                pure: true,
            })
            .collect(),
    };
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    let value = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Number".into(),
        fields: vec![NdfValue::U64(41)],
    });
    Ok((
        registry,
        Invoke {
            request_id: 17,
            operation: OperationRef {
                schema,
                name: "dependent".into(),
            },
            input: value.clone(),
            environment: value,
            sources: vec![],
            resources: vec![],
            limits: budget().limits(),
        },
    ))
}
pub fn increment(
    request: &Invoke,
    _: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 1)?;
    let mut value = request.input.clone_with_budget(b)?;
    if let TypedValue::Record(record) = &mut value
        && let [NdfValue::U64(number)] = record.fields.as_mut_slice()
        && let Some(next) = number.checked_add(1)
    {
        *number = next;
        return Ok(OperationReply::Result(OperationResult::Complete {
            value,
            report: Report::default(),
        }));
    }
    Ok(OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    }))
}
pub fn await_increment(
    parent: &Invoke,
    context: Digest,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 7)?;
    // Test fixture construction; real dispatch and all wire validation are used.
    let mut call = parent.clone();
    call.request_id = 18;
    call.operation.name = "increment".into();
    Ok(OperationReply::Await {
        continuation: Continuation {
            provider: parent.operation.clone(),
            parent_request: parent.request_id,
            snapshot_digest: context,
            state: parent.input.clone_with_budget(b)?,
        },
        calls: vec![call],
        report: Report::default(),
    })
}
pub fn finish(
    _: &Invoke,
    request: &Resume,
    _: &SchemaRegistry,
    b: &mut Budget,
) -> Result<OperationReply, StopReason> {
    b.charge(Resource::Work, 5)?;
    if let [OperationReply::Result(OperationResult::Complete { value, .. })] =
        request.dependency_results.as_slice()
    {
        return Ok(OperationReply::Result(OperationResult::Complete {
            value: value.clone_with_budget(b)?,
            report: Report::default(),
        }));
    }
    Ok(OperationReply::Result(OperationResult::Invalid {
        partial: None,
        report: Report::default(),
    }))
}
