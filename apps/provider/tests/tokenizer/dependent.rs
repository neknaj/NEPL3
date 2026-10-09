//! Test-only host-selected Dependent handler, not a standard provider executor.
use super::*;

pub(super) enum Actual<'a> {
    Name(portable::read::sender::ExecutedName<'a>),
    Dependent { reply: ReadReply, usage: Usage },
}
impl Actual<'_> {
    pub(super) fn reply(&self) -> &ReadReply {
        match self {
            Self::Name(r) => r.reply(),
            Self::Dependent { reply, .. } => reply,
        }
    }
    pub(super) fn execution_usage(&self) -> Usage {
        match self {
            Self::Name(r) => r.execution_usage(),
            Self::Dependent { usage, .. } => *usage,
        }
    }
}
fn reader_type(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}
pub(super) fn register(registry: &mut SchemaRegistry) -> Result<ProviderSignature, String> {
    let descriptor = SchemaDescriptor {
        package: "test.dependent".into(),
        revision: 1,
        types: vec![],
        operations: vec![OperationDescriptor {
            name: "readTail".into(),
            input: reader_type("DependentRequest"),
            output: reader_type("ReadReply"),
            pure: true,
        }],
    };
    let schema = descriptor.reference(&mut budget()).map_err(err)?;
    registry
        .register(schema.clone(), descriptor, &mut budget())
        .map_err(err)?;
    Ok(ProviderSignature {
        operation: OperationRef {
            schema,
            name: "readTail".into(),
        },
        kind: ProviderKind::Dependent,
        value_input: TypeDescriptor::Text,
        value_output: TypeDescriptor::Text,
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: reader_type("ReaderContinuation"),
    })
}
#[allow(clippy::too_many_arguments)]
pub(super) fn execute<'a, C: FoundationValueCodec>(
    signature: &ProviderSignature,
    operation: &OperationRef,
    input: &TypedValue,
    registry: &'a SchemaRegistry,
    authorized: &SourceStore,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Actual<'a>, String>
where
    C::Error: std::fmt::Debug,
{
    if signature.kind == ProviderKind::Read {
        return portable::read::sender::execute_name(
            operation, input, registry, authorized, codec, b,
        )
        .map(Actual::Name)
        .map_err(err);
    }
    b.poll().map_err(err)?;
    assert_eq!(operation, &signature.operation);
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or("reader schema")?;
    let raw = match input.clone_with_budget(b).map_err(err)? {
        TypedValue::Record(r) => NdfValue::Record(r),
        _ => return Err("record".into()),
    };
    let declared = {
        let mut scoped = codec.scoped(authorized);
        portable::dependent::sources(&raw, schema, &mut scoped, registry, b).map_err(err)?
    };
    let mut local = SourceStore::default();
    for source in &declared {
        let approved = authorized
            .get_revision_with_budget(&source.identity().source, source.identity().revision, b)
            .map_err(err)?
            .ok_or("unauthorized source")?;
        if !approved.eq_with_budget(source, b).map_err(err)? {
            return Err("source conflict".into());
        }
        local.insert_ref_with_budget(approved, b).map_err(err)?;
    }
    let mut scoped = codec.scoped(&local);
    let decoded = portable::dispatch::from_value(
        input,
        &portable::dispatch::DispatchContext {
            signature,
            sources: &local,
            mappings: &[],
            registry,
        },
        &mut scoped,
        b,
    )
    .map_err(err)?;
    let portable::dispatch::ProviderInput::Dependent(request) = decoded else {
        return Err("Dependent".into());
    };
    assert_eq!(request.first, NdfValue::Text("a".into()));
    assert_eq!(request.end, 2);
    assert_eq!(request.request.start, 2);
    let nested = &request.request;
    let context = nested
        .context
        .check(&mut scoped, &local, registry, b)
        .map_err(err)?;
    let snapshot = local
        .resolve_with_budget(&nested.snapshot, b)
        .map_err(err)?
        .ok_or("snapshot")?;
    let reply = nepl3_reader::builtin::read(
        BuiltinReader::Name,
        ReadRequest {
            snapshot,
            start: nested.start,
            limit: nested.limit,
            final_input: nested.final_input,
            context: &context,
            state: &nested.state,
        },
        None,
        registry,
        &local,
        b,
        scoped.source_admission(),
    )
    .map_err(err)?;
    Ok(Actual::Dependent {
        reply,
        usage: b.usage(),
    })
}
