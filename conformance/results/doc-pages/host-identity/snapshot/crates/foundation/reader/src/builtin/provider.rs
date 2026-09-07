//! Explicit standard-provider dispatch. Hosts choose these operations and retain
//! the caller's shared depth, budget, admission and checked context.
use super::BuiltinReader;
use crate::{
    model::{ReadReply, ReadRequest},
    plan::{ProviderKind, ProviderSignature},
    runtime::ReaderError,
};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::{SourceAdmission, SourceStore},
    value::{NdfValue, OperationRef},
};
fn name(kind: BuiltinReader) -> Result<&'static str, ReaderError> {
    match kind {
        BuiltinReader::Name => Ok("builtinName"),
        BuiltinReader::Trivia => Ok("builtinTrivia"),
        BuiltinReader::Number => Ok("builtinNumber"),
        _ => Err(ReaderError::ProviderContract),
    }
}
/// Resolve a registered standard operation. Other builtins use their explicit
/// native entry until their portable reservation/envelope contract is selected.
pub fn operation(
    kind: BuiltinReader,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<OperationRef, ReaderError> {
    let name = name(kind)?;
    let schema = registry
        .selected("nepl3.reader", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let descriptor = registry
        .descriptor(schema)
        .ok_or(SchemaError::UnknownSchema)?;
    budget.charge(
        Resource::Work,
        descriptor.operations.len() as u64 * (name.len() as u64 + 1),
    )?;
    let op = descriptor
        .operations
        .iter()
        .find(|v| v.name == name)
        .ok_or(ReaderError::ProviderContract)?;
    let envelope = |ty: &TypeDescriptor, expected: &str| matches!(ty,TypeDescriptor::Named(v) if v.package=="nepl3.reader"&&v.revision==1&&v.name==expected);
    if !op.pure || !envelope(&op.input, "ReadRequest") || !envelope(&op.output, "ReadReply") {
        return Err(ReaderError::ProviderContract);
    }
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<OperationRef>() as u64
            + schema.package.len() as u64
            + name.len() as u64,
    )?;
    Ok(OperationRef {
        schema: schema.clone(),
        name: name.into(),
    })
}
/// The standard provider has Unit state and never returns Await.
pub fn signature(
    kind: BuiltinReader,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<ProviderSignature, ReaderError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<ProviderSignature>() as u64,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.reader".len() + "ReaderContinuation".len()) as u64,
    )?;
    Ok(ProviderSignature {
        operation: operation(kind, registry, budget)?,
        kind: ProviderKind::Read,
        value_input: TypeDescriptor::Unit,
        value_output: match kind {
            BuiltinReader::Name => TypeDescriptor::Text,
            BuiltinReader::Number => TypeDescriptor::Rational,
            BuiltinReader::Trivia => TypeDescriptor::Unit,
            _ => return Err(ReaderError::ProviderContract),
        },
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: TypeDescriptor::Named(TypeRef {
            package: "nepl3.reader".into(),
            revision: 1,
            name: "ReaderContinuation".into(),
        }),
    })
}
/// Dispatch an explicitly selected operation to the same production builtin path.
/// The host establishes the validated ProviderCall depth before invoking this.
#[allow(clippy::too_many_arguments)]
pub fn read(
    operation_ref: &OperationRef,
    request: ReadRequest<'_>,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ReadReply, ReaderError> {
    let result = (|| {
        budget.charge(
            Resource::Work,
            (operation_ref.name.len() + operation_ref.schema.package.len()) as u64 + 33,
        )?;
        let kind = match operation_ref.name.as_str() {
            "builtinName" => BuiltinReader::Name,
            "builtinTrivia" => BuiltinReader::Trivia,
            "builtinNumber" => BuiltinReader::Number,
            _ => return Err(ReaderError::ProviderContract),
        };
        if registry.selected("nepl3.reader", 1) != Some(&operation_ref.schema)
            || request.state != &NdfValue::Unit
        {
            return Err(ReaderError::ProviderContract);
        }
        // Verify the selected registered envelope without accepting a caller's digest assertion.
        let expected = operation(kind, registry, budget)?;
        if &expected != operation_ref {
            return Err(ReaderError::ProviderContract);
        }
        super::read(kind, request, None, registry, sources, budget, admission)
    })();
    crate::runtime::stopped(result, budget)
}
