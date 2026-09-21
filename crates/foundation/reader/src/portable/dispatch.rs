//! Operation inputs checked against a host-selected provider signature.
use super::*;
use crate::{
    model::{DependentRequest, TransformRequest},
    plan::{ProviderKind, ProviderSignature},
};
use alloc::boxed::Box;
use nepl3_core::{origin::Mapping, value::TypedValue};

/// An operation input without host-owned session/call correlation metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderInput {
    Read(Box<OwnedReadRequest>),
    Dependent(Box<DependentRequest>),
    Transform(Box<TransformRequest>),
}

/// Explicit source and mapping authority selected for one dispatch.
pub struct DispatchContext<'a> {
    pub signature: &'a ProviderSignature,
    pub sources: &'a SourceStore,
    pub mappings: &'a [Mapping],
    pub registry: &'a SchemaRegistry,
}

impl DispatchContext<'_> {
    fn check<E>(&self, input: &ProviderInput, b: &mut Budget) -> Result<(), PortableError<E>> {
        self.signature
            .check(self.registry, b)
            .map_err(PortableError::Plan)?;
        let (kind, value, state) = match input {
            ProviderInput::Read(r) => (ProviderKind::Read, None, Some(&r.state)),
            ProviderInput::Dependent(r) => (
                ProviderKind::Dependent,
                Some(&r.first),
                Some(&r.request.state),
            ),
            ProviderInput::Transform(r) => (ProviderKind::Transform, Some(&r.value), None),
        };
        if kind != self.signature.kind {
            return Err(PortableError::Shape);
        }
        if let Some(value) = value {
            self.registry
                .validate(&self.signature.value_input, value, b)
                .map_err(PortableError::Schema)?;
        }
        if let Some(state) = state {
            self.registry
                .validate(&self.signature.state_type, state, b)
                .map_err(PortableError::Schema)?;
        }
        Ok(())
    }
    fn schema<E>(&self) -> Result<&SchemaRef, PortableError<E>> {
        self.registry
            .selected(crate::schema::PACKAGE, crate::schema::REVISION)
            .ok_or(PortableError::Shape)
    }
}

/// Encode the input after signature and operation-specific boundary validation.
pub fn to_value<C: FoundationValueCodec>(
    input: &ProviderInput,
    context: &DispatchContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<TypedValue, PortableError<C::Error>> {
    context.check(input, b)?;
    let schema = context.schema()?;
    let value = match input {
        ProviderInput::Read(r) => {
            request_to_value(r, schema, codec, context.sources, context.registry, b)?
        }
        ProviderInput::Dependent(r) => {
            dependent::to_value(r, schema, codec, context.sources, context.registry, b)?
        }
        ProviderInput::Transform(r) => transform::request::to_value(
            r,
            schema,
            codec,
            context.sources,
            context.mappings,
            context.registry,
            b,
        )?,
    };
    value.charge_clone(b)?;
    match &value {
        NdfValue::Record(record) => Ok(TypedValue::Record(record.clone())),
        _ => Err(PortableError::Shape),
    }
}

/// Decode using the selected operation kind, then check its concrete value types.
/// The host admits Read/Dependent source tables before constructing this context.
pub fn from_value<C: FoundationValueCodec>(
    value: &TypedValue,
    context: &DispatchContext<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<ProviderInput, PortableError<C::Error>> {
    context
        .signature
        .check(context.registry, b)
        .map_err(PortableError::Plan)?;
    let value = match value.clone_with_budget(b)? {
        TypedValue::Record(v) => NdfValue::Record(v),
        TypedValue::Variant(_) => return Err(PortableError::Shape),
    };
    let schema = context.schema()?;
    let input = match context.signature.kind {
        ProviderKind::Read => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<OwnedReadRequest>() as u64,
            )?;
            ProviderInput::Read(Box::new(request_from_value(
                &value,
                schema,
                codec,
                context.sources,
                context.registry,
                b,
            )?))
        }
        ProviderKind::Dependent => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<DependentRequest>() as u64,
            )?;
            ProviderInput::Dependent(Box::new(dependent::from_value(
                &value,
                schema,
                codec,
                context.sources,
                context.registry,
                b,
            )?))
        }
        ProviderKind::Transform => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<TransformRequest>() as u64,
            )?;
            ProviderInput::Transform(Box::new(transform::request::from_value(
                &value,
                schema,
                codec,
                context.sources,
                context.mappings,
                context.registry,
                b,
            )?))
        }
    };
    context.check(&input, b)?;
    Ok(input)
}
