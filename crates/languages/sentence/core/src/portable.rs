//! Explicit SentenceValue boundary. Schema, arena and foreign closures are
//! checked in both directions. No guest meaning or rendering is executed.
pub mod literal;
pub mod syntax;
mod value;
use crate::{check, model::SentenceValue};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Structure(check::Error),
    Foundation(E),
    Shape,
    SchemaIdentity,
    Presentation(crate::syntax::Error),
}
impl<E> From<crate::syntax::Error> for Error<E> {
    fn from(e: crate::syntax::Error) -> Self {
        match e {
            crate::syntax::Error::Stopped(s) => Self::Stopped(s),
            other => Self::Presentation(other),
        }
    }
}
impl<E> From<StopReason> for Error<E> {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl<E> From<SchemaError> for Error<E> {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::Stopped(s) => Self::Stopped(s),
            other => Self::Schema(other),
        }
    }
}
impl<E> From<check::Error> for Error<E> {
    fn from(e: check::Error) -> Self {
        match e {
            check::Error::Stopped(s) => Self::Stopped(s),
            other => Self::Structure(other),
        }
    }
}
fn boundary<E: FoundationCodecError>(e: E) -> Error<E> {
    match e.stop_reason() {
        Some(s) => Error::Stopped(s),
        None => Error::Foundation(e),
    }
}
fn schema<'a, E>(registry: &'a SchemaRegistry, b: &mut Budget) -> Result<&'a SchemaRef, Error<E>> {
    b.poll()?;
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    let selected = registry
        .selected("nepl3.sentence", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    let expected = crate::schema::descriptor(b)?.reference(b)?;
    b.charge(Resource::Work, expected.package.len() as u64 + 40)?;
    if selected != &expected {
        return Err(Error::SchemaIdentity);
    }
    Ok(selected)
}
fn validate<E>(registry: &SchemaRegistry, raw: &NdfValue, b: &mut Budget) -> Result<(), Error<E>> {
    validate_named(registry, raw, "SentenceValue", b)
}
fn validate_named<E>(
    registry: &SchemaRegistry,
    raw: &NdfValue,
    name: &str,
    b: &mut Budget,
) -> Result<(), Error<E>> {
    b.charge(
        Resource::AllocationUnits,
        ("nepl3.sentence".len() + name.len()) as u64,
    )?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.sentence".into(),
            revision: 1,
            name: name.into(),
        }),
        raw,
        b,
    )?;
    Ok(())
}

pub fn to_value<C: FoundationValueCodec>(
    input: &SentenceValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    let schema = schema(registry, b)?;
    input
        .validate_shape(b)?
        .validate_foreign(registry, b, codec.source_admission())?;
    let output = value::encode(input, schema, codec, b)?;
    validate(registry, &output, b)?;
    b.poll()?;
    Ok(output)
}

pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<SentenceValue, Error<C::Error>> {
    let schema = schema(registry, b)?;
    validate(registry, input, b)?;
    let output = value::decode(input, schema, codec, b)?;
    output
        .validate_shape(b)?
        .validate_foreign(registry, b, codec.source_admission())?;
    b.poll()?;
    Ok(output)
}
