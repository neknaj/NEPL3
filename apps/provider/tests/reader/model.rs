use super::*;

pub(super) fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 1_000_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}

pub(super) fn reader_type(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}

pub(super) fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let mut b = budget();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut b)?;
    let reference = foundation.reference(&mut b)?;
    registry.register(reference, foundation, &mut b)?;
    let reader = nepl3_reader::schema::descriptor(&mut b)?;
    let reference = reader.reference(&mut b)?;
    registry.register(reference, reader, &mut b)?;
    let descriptor = SchemaDescriptor {
        package: "test".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            constraints: vec![],
            shape: TypeShape::Record { fields: vec![] },
        }],
        operations: vec![
            OperationDescriptor {
                name: "read".into(),
                input: reader_type("ReadRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "transform".into(),
                input: reader_type("TransformRequest"),
                output: reader_type("TransformReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "dependent".into(),
                input: reader_type("DependentRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
        ],
    };
    let schema = descriptor.reference(&mut b)?;
    registry.register(schema.clone(), descriptor, &mut b)?;
    registry.finalize(&mut b)?;
    Ok((registry, schema))
}

pub(super) fn plan(
    schema: &SchemaRef,
    expressions: Vec<ReaderExpr>,
    root: u64,
    output: TypeDescriptor,
) -> ReaderPlan {
    ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions,
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(root),
            output,
        }],
        providers: vec![],
    }
}

pub(super) fn context(
    schema: &SchemaRef,
    registry: &SchemaRegistry,
) -> Result<ReaderContext, ReaderError> {
    let mut context = ReaderContext {
        schema: schema.clone(),
        category: "Token".into(),
        mode: "test".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest: Digest([0; 32]),
            value: Environment {
                bindings: vec![],
                resources: vec![],
            },
        },
        origins: vec![],
    };
    context.environment.digest = nepl3_wire::environment::environment_digest(
        &context.environment.value,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or(SchemaError::UnknownSchema)?,
        registry,
        &mut budget(),
    )
    .map_err(|_| ReaderError::Context)?;
    Ok(context)
}

pub(super) fn check_context<'a>(
    raw: &'a ReaderContext,
    store: &'a SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<nepl3_reader::context::CheckedReaderContext<'a>, ReaderError> {
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(registry, store, admission)
        .map_err(|_| ReaderError::Context)?;
    raw.check(&mut codec, store, registry, budget)
        .map_err(|_| ReaderError::Context)
}

pub(super) fn source(text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("test".into()),
        0,
        "memory:test".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}

pub(super) fn signature(schema: &SchemaRef, kind: ProviderKind) -> ProviderSignature {
    ProviderSignature {
        operation: OperationRef {
            schema: schema.clone(),
            name: match kind {
                ProviderKind::Read => "read",
                ProviderKind::Transform => "transform",
                ProviderKind::Dependent => "dependent",
            }
            .into(),
        },
        kind,
        value_input: if kind == ProviderKind::Read {
            TypeDescriptor::Unit
        } else {
            TypeDescriptor::Text
        },
        value_output: TypeDescriptor::Text,
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: reader_type("ReaderContinuation"),
    }
}

pub(super) fn provider_plan(schema: &SchemaRef) -> ReaderPlan {
    let signature = signature(schema, ProviderKind::Read);
    let mut p = plan(
        schema,
        vec![ReaderExpr::Call(signature.operation.clone())],
        0,
        TypeDescriptor::Text,
    );
    p.providers.push(signature);
    p
}

pub(super) fn terminal(
    value: &str,
    end: u64,
    budget: &mut Budget,
) -> Result<ProviderReply, StopReason> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text(value.into()),
        end,
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })))
}
