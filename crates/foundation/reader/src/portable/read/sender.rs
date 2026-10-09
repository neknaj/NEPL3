//! Issued builtinName results and standalone execution at the portable boundary.
use super::*;
use crate::{
    builtin::{BuiltinReader, provider},
    model::ReadRequest,
};
use nepl3_core::{
    budget::Usage,
    operation::OperationReply,
    value::{OperationRef, TypedValue},
};

/// An actual native result with immutable execution accounting and sources.
/// Standalone execution starts from its supplied budget; pending native dispatch
/// retains the caller's cumulative accounting on the same budget.
/// This proof is local only: its wire encoding does not authenticate a remote peer.
pub struct ExecutedName<'a> {
    reply: ReadReply,
    usage: Usage,
    registry: &'a SchemaRegistry,
    sources: SourceStore,
}
impl ExecutedName<'_> {
    pub fn reply(&self) -> &ReadReply {
        &self.reply
    }
    /// Consume the result for the existing typed native Reader API. This raw
    /// reply still requires ordinary saved-dispatch validation at resume.
    pub fn into_reply(self) -> ReadReply {
        self.reply
    }
    pub fn execution_usage(&self) -> Usage {
        self.usage
    }
    /// Serialize with a separately bounded encoding budget. A retry never resumes
    /// or reruns execution, including when its original budget is stopped.
    pub fn to_operation_reply<C: FoundationValueCodec>(
        &self,
        codec: &mut C,
        encoding: &mut Budget,
    ) -> Result<OperationReply, PortableError<C::Error>> {
        encoding.poll()?;
        let (schema, _) = self
            .registry
            .selected_descriptor_with_budget(
                crate::schema::PACKAGE,
                crate::schema::REVISION,
                encoding,
            )?
            .ok_or(PortableError::Shape)?;
        let encoded = {
            let mut scoped = codec.scoped(&self.sources);
            value::encode(&self.reply, schema, self.registry, &mut scoped, encoding)?
        };
        super::super::validate_named(&encoded, schema, "ReadReply", self.registry, encoding)?;
        operation::wrap(&self.reply, encoded, encoding)
    }
}

/// Execute only the selected builtinName operation. Every declared source must
/// already be authorized; resolution and retained encoding authority are narrowed
/// to the request's table. This does not reconcile a remote caller's Usage/depth.
pub fn execute_name<'a, C: FoundationValueCodec>(
    operation: &OperationRef,
    input: &TypedValue,
    registry: &'a SchemaRegistry,
    authorized: &SourceStore,
    codec: &mut C,
    execution: &mut Budget,
) -> Result<ExecutedName<'a>, PortableError<C::Error>> {
    execution.poll()?;
    let signature =
        provider::signature(BuiltinReader::Name, registry, execution).map_err(reader)?;
    execution.charge(
        Resource::Work,
        (operation.name.len() as u64)
            .saturating_add(operation.schema.package.len() as u64)
            .saturating_add(signature.operation.name.len() as u64)
            .saturating_add(signature.operation.schema.package.len() as u64)
            .saturating_add(33),
    )?;
    if operation != &signature.operation {
        return Err(reader(ReaderError::ProviderContract));
    }
    let (schema, _) = registry
        .selected_descriptor_with_budget(
            crate::schema::PACKAGE,
            crate::schema::REVISION,
            execution,
        )?
        .ok_or(PortableError::Shape)?;
    // Phase one has no context resolution. Establish the exact table first.
    let raw = match input.clone_with_budget(execution)? {
        TypedValue::Record(v) => NdfValue::Record(v),
        TypedValue::Variant(_) => return Err(PortableError::Shape),
    };
    let declared = {
        let mut scoped = codec.scoped(authorized);
        super::super::request_sources(&raw, schema, &mut scoped, registry, execution)?
    };
    let mut local = SourceStore::default();
    for source in &declared {
        execution.charge(Resource::Work, 1)?;
        let approved = authorized
            .get_revision_with_budget(
                &source.identity().source,
                source.identity().revision,
                execution,
            )?
            .ok_or(PortableError::UndeclaredSource)?;
        if !approved.eq_with_budget(source, execution)? {
            return Err(PortableError::UndeclaredSource);
        }
        local
            .insert_ref_with_budget(approved, execution)
            .map_err(PortableError::Source)?;
    }
    let input = {
        let mut scoped = codec.scoped(&local);
        super::super::dispatch::from_value(
            input,
            &super::super::dispatch::DispatchContext {
                signature: &signature,
                sources: &local,
                mappings: &[],
                registry,
            },
            &mut scoped,
            execution,
        )?
    };
    let super::super::dispatch::ProviderInput::Read(request) = input else {
        return Err(PortableError::Shape);
    };
    let reply = {
        let mut scoped = codec.scoped(&local);
        let context = request
            .context
            .check(&mut scoped, &local, registry, execution)
            .map_err(PortableError::Context)?;
        let snapshot = local
            .resolve_with_budget(&request.snapshot, execution)?
            .ok_or(PortableError::UndeclaredSource)?;
        provider::read(
            operation,
            ReadRequest {
                snapshot,
                start: request.start,
                limit: request.limit,
                final_input: request.final_input,
                context: &context,
                state: &request.state,
            },
            registry,
            &local,
            execution,
            scoped.source_admission(),
        )
        .map_err(reader)?
    };
    Ok(issued(reply, registry, local, execution))
}

// Only trusted native dispatch paths call this after executing the real builtin.
pub(super) fn issued<'a>(
    reply: ReadReply,
    registry: &'a SchemaRegistry,
    sources: SourceStore,
    execution: &Budget,
) -> ExecutedName<'a> {
    ExecutedName {
        reply,
        usage: execution.usage(),
        registry,
        sources,
    }
}
