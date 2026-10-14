//! Portable compilation input. This is not an executed or authorized operation.
use crate::{
    compile::profile::{CompileProfile, portable as profile},
    model::Document,
    portable as document,
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::Digest,
    value::{NdfValue, Record},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

/// The exact document and symbolic catalogs supplied for one compilation.
/// A profile's applicability is checked by compilation, not by decoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequest {
    pub document: Document,
    pub profile: CompileProfile,
}
#[derive(Debug)]
pub enum RequestError<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Document(document::PortableError<E>),
    Profile(profile::ProfileError<E>),
    Boundary(E),
    Shape,
}
impl<E> From<StopReason> for RequestError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl<E> From<SchemaError> for RequestError<E> {
    fn from(error: SchemaError) -> Self {
        match error {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            other => Self::Schema(other),
        }
    }
}
fn document_error<E>(error: document::PortableError<E>) -> RequestError<E> {
    match error {
        document::PortableError::Stopped(reason) => RequestError::Stopped(reason),
        other => RequestError::Document(other),
    }
}
fn profile_error<E>(error: profile::ProfileError<E>) -> RequestError<E> {
    match error {
        profile::ProfileError::Stopped(reason) => RequestError::Stopped(reason),
        other => RequestError::Profile(other),
    }
}
fn boundary<E: FoundationCodecError>(error: E) -> RequestError<E> {
    match error.stop_reason() {
        Some(reason) => RequestError::Stopped(reason),
        None => RequestError::Boundary(error),
    }
}
fn validate<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), RequestError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.grammar".len() + "CompileRequest".len()) as u64,
    )?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.grammar".into(),
            revision: 1,
            name: "CompileRequest".into(),
        }),
        value,
        budget,
    )?;
    Ok(())
}
/// Encode using one shared codec, source admission and cumulative budget.
pub fn to_value<C: FoundationValueCodec>(
    request: &CompileRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, RequestError<C::Error>> {
    budget.poll()?;
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    let schema = registry
        .selected("nepl3.grammar", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let document =
        document::to_value(&request.document, registry, codec, budget).map_err(document_error)?;
    let profile =
        profile::to_value(&request.profile, registry, codec, budget).map_err(profile_error)?;
    budget.charge(
        Resource::Work,
        (schema.package.len() + "CompileRequest".len()) as u64 + 1,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + "CompileRequest".len() + 2 * core::mem::size_of::<NdfValue>())
            as u64,
    )?;
    let value = NdfValue::Record(Record {
        schema: schema.clone(),
        kind: "CompileRequest".into(),
        fields: Vec::from([document, profile]),
    });
    validate(&value, registry, budget)?;
    Ok(value)
}
/// Validate the complete structural envelope before admitting document sources.
/// Document decoding retains its explicit source closure; profile entries do not
/// gain registry membership, runtime providers or execution permission.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<CompileRequest, RequestError<C::Error>> {
    budget.poll()?;
    validate(value, registry, budget)?;
    let schema = registry
        .selected("nepl3.grammar", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let NdfValue::Record(record) = value else {
        return Err(RequestError::Shape);
    };
    if record.schema != *schema || record.kind != "CompileRequest" || record.fields.len() != 2 {
        return Err(RequestError::Shape);
    }
    let document =
        document::from_value(&record.fields[0], registry, codec, budget).map_err(document_error)?;
    let profile =
        profile::from_value(&record.fields[1], registry, codec, budget).map_err(profile_error)?;
    Ok(CompileRequest { document, profile })
}
/// Canonical identity for a host-retained input. This includes document source
/// bytes and every profile field. An echoed digest alone is not proof that a
/// provider compiled that input correctly, nor a request-lifetime check.
pub fn fingerprint<C: FoundationValueCodec>(
    request: &CompileRequest,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<Digest, RequestError<C::Error>> {
    let value = to_value(request, registry, codec, budget)?;
    codec
        .canonical_value_digest(b"nepl3.grammar.compile-request/1\0", &value, budget)
        .map_err(boundary)
}
