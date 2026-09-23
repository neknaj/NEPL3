use super::*;
use external_composition_runtime::{program, syntax::Cursor};
use nepl3_core::schema::*;
use nepl3_engine::{parse::ParseOutcome, profile::RuntimeCatalog};

pub struct Model {
    pub registry: SchemaRegistry,
    pub runtime: Runtime,
    pub operation: OperationRef,
    pub plan: SchemaRef,
    pub foundation: SchemaRef,
}

pub fn identity() -> Digest {
    Digest::of(b"composition process test executable v1")
}

pub fn model() -> Result<Model, String> {
    let mut registry = SchemaRegistry::default();
    let runtime =
        Runtime::register(&mut registry, [identity(); 2], &mut budget()).map_err(error)?;
    let plan = transfer::descriptor(&mut budget())
        .map_err(error)?
        .reference(&mut budget())
        .map_err(error)?;
    let foundation = foundation::descriptor(&mut budget())
        .map_err(error)?
        .reference(&mut budget())
        .map_err(error)?;
    // Test-only outer operation: one packet evaluates to the existing plan Value.
    let descriptor = SchemaDescriptor {
        package: "test.composition.process".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Packet".into(),
            constraints: vec![],
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "bytes".into(),
                    ty: TypeDescriptor::Bytes,
                }],
            },
        }],
        operations: vec![OperationDescriptor {
            name: "evaluate".into(),
            pure: true,
            input: TypeDescriptor::Named(TypeRef {
                package: "test.composition.process".into(),
                revision: 1,
                name: "Packet".into(),
            }),
            output: TypeDescriptor::Named(TypeRef {
                package: transfer::PACKAGE.into(),
                revision: 1,
                name: "Value".into(),
            }),
        }],
    };
    let schema = descriptor.reference(&mut budget()).map_err(error)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(error)?;
    registry.finalize(&mut budget()).map_err(error)?;
    Ok(Model {
        registry,
        runtime,
        operation: OperationRef {
            schema,
            name: "evaluate".into(),
        },
        plan,
        foundation,
    })
}

pub fn sources(text: &str) -> Result<SourceStore, String> {
    let observation = composition::inspect(text, true)?;
    let ParseOutcome::Complete { tree, .. } = observation.parse.outcome else {
        return Err("complete fixture parse required".into());
    };
    let mut sources = SourceStore::default();
    for source in &tree.bundle.sources {
        sources
            .insert_ref_with_budget(source, &mut budget())
            .map_err(error)?;
    }
    Ok(sources)
}

pub fn packet(
    text: &str,
    model: &Model,
) -> Result<(Vec<u8>, SourceStore, OperationResult<TypedValue>), String> {
    let languages = composition::languages("Expr", "Frame")?;
    let profile = languages.profile("composition")?;
    let packages = languages
        .packages
        .iter()
        .map(|(_, p)| p)
        .collect::<Vec<_>>();
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &packages,
                providers: &[],
                resources: &[],
            },
            &languages.registry,
            &mut budget(),
        )
        .map_err(error)?;
    let observation = composition::inspect(text, true)?;
    let ParseOutcome::Complete { tree, .. } = observation.parse.outcome else {
        return Err("complete fixture parse required".into());
    };
    let proof = tree
        .validate(&resolved, &mut budget(), &mut SourceAdmission::default())
        .map_err(error)?;
    let cursor = Cursor::root(
        &proof,
        &packages[0].schema,
        &packages[1].schema,
        &mut budget(),
    )
    .map_err(error)?;
    let program = program::compile(cursor, &mut budget()).map_err(error)?;
    let sources = sources(text)?;
    let bytes = transfer::envelope::encode(
        &program,
        &model.plan,
        &model.foundation,
        &model.registry,
        &mut budget(),
    )
    .map_err(error)?;
    let result = model
        .runtime
        .run(
            &program,
            &sources,
            &model.registry,
            &mut budget(),
            &mut budget(),
            |_, _| {},
            |_| {},
        )
        .map_err(error)?;
    Ok((bytes, sources, result))
}

pub fn environment(model: &Model) -> TypedValue {
    TypedValue::Record(Record {
        schema: model.operation.schema.clone(),
        kind: "Packet".into(),
        fields: vec![NdfValue::Bytes(vec![])],
    })
}

pub fn complete_value(result: &OperationResult<TypedValue>) -> Result<&Record, String> {
    if let OperationResult::Complete {
        value: TypedValue::Record(record),
        ..
    } = result
    {
        Ok(record)
    } else {
        Err(format!("expected Complete: {result:?}"))
    }
}
