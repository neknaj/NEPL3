//! Grammar-owned profile records compose owner-provided symbolic value codecs.
use super::{CompileProfile, NamedClass, NamedView, ReaderImport};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    value::{NdfValue, Record, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_engine::portable::package as engine;
use nepl3_reader::portable::plan as reader;

#[derive(Debug)]
pub enum ProfileError<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Boundary(E),
    Reader(nepl3_reader::portable::PortableError<E>),
    Engine(nepl3_engine::portable::PortableError<E>),
    Shape,
}
impl<E> From<StopReason> for ProfileError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl<E> From<SchemaError> for ProfileError<E> {
    fn from(error: SchemaError) -> Self {
        match error {
            SchemaError::Stopped(reason) => Self::Stopped(reason),
            other => Self::Schema(other),
        }
    }
}
fn boundary<E: FoundationCodecError>(error: E) -> ProfileError<E> {
    match error.stop_reason() {
        Some(reason) => ProfileError::Stopped(reason),
        None => ProfileError::Boundary(error),
    }
}
fn reader_error<E>(error: nepl3_reader::portable::PortableError<E>) -> ProfileError<E> {
    match error {
        nepl3_reader::portable::PortableError::Stopped(reason) => ProfileError::Stopped(reason),
        other => ProfileError::Reader(other),
    }
}
fn engine_error<E>(error: nepl3_engine::portable::PortableError<E>) -> ProfileError<E> {
    match error {
        nepl3_engine::portable::PortableError::Stopped(reason) => ProfileError::Stopped(reason),
        other => ProfileError::Engine(other),
    }
}
fn schema<E>(registry: &SchemaRegistry) -> Result<&SchemaRef, ProfileError<E>> {
    registry
        .selected("nepl3.grammar", 1)
        .ok_or(ProfileError::Schema(SchemaError::UnknownSchema))
}
fn validate<E>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ProfileError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.grammar".len() + "CompileProfile".len()) as u64,
    )?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.grammar".into(),
            revision: 1,
            name: "CompileProfile".into(),
        }),
        value,
        budget,
    )?;
    Ok(())
}
fn text<E>(value: &str, budget: &mut Budget) -> Result<String, ProfileError<E>> {
    budget.charge(Resource::Work, value.len() as u64 + 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}
fn read_text<E>(value: &NdfValue, budget: &mut Budget) -> Result<String, ProfileError<E>> {
    match value {
        NdfValue::Text(value) => text(value, budget),
        _ => Err(ProfileError::Shape),
    }
}
fn record<E, const N: usize>(
    schema: &SchemaRef,
    name: &str,
    fields: [NdfValue; N],
    budget: &mut Budget,
) -> Result<NdfValue, ProfileError<E>> {
    budget.charge(
        Resource::Work,
        (schema.package.len() + name.len()) as u64 + 1,
    )?;
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + N * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(NdfValue::Record(Record {
        schema: schema.clone(),
        kind: name.into(),
        fields: Vec::from(fields),
    }))
}
fn fields<'a, E>(
    value: &'a NdfValue,
    schema: &SchemaRef,
    name: &str,
    arity: usize,
) -> Result<&'a [NdfValue], ProfileError<E>> {
    match value {
        NdfValue::Record(value)
            if &value.schema == schema && value.kind == name && value.fields.len() == arity =>
        {
            Ok(&value.fields)
        }
        _ => Err(ProfileError::Shape),
    }
}
fn map<T, U, E>(
    values: &[T],
    budget: &mut Budget,
    mut convert: impl FnMut(&T, &mut Budget) -> Result<U, ProfileError<E>>,
) -> Result<Vec<U>, ProfileError<E>> {
    let mut output = Vec::new();
    for value in values {
        budget.charge(Resource::Work, 1)?;
        budget.charge(Resource::AllocationUnits, core::mem::size_of::<U>() as u64)?;
        output.push(convert(value, budget)?);
    }
    Ok(output)
}
fn list<E>(value: &NdfValue) -> Result<&[NdfValue], ProfileError<E>> {
    match value {
        NdfValue::List(values) => Ok(values),
        _ => Err(ProfileError::Shape),
    }
}
/// Encode all six catalogs in their existing order. This validates their NDF
/// representation, not their applicability to a particular Grammar document.
pub fn to_value<C: FoundationValueCodec>(
    profile: &CompileProfile,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, ProfileError<C::Error>> {
    budget.poll()?;
    let schema = schema(registry)?;
    let package = NdfValue::Text(text(&profile.package, budget)?);
    let state = codec
        .encode_type_descriptor(&profile.state_type, budget)
        .map_err(boundary)?;
    let imports = map(&profile.reader_imports, budget, |entry, budget| {
        record(
            schema,
            "ReaderImport",
            [
                NdfValue::Text(text(&entry.provider, budget)?),
                reader::signature_to_value(&entry.signature, registry, codec, budget)
                    .map_err(reader_error)?,
            ],
            budget,
        )
    })?;
    let extensions = map(&profile.extensions, budget, |entry, budget| {
        engine::extension_to_value(entry, registry, codec, budget).map_err(engine_error)
    })?;
    let classes = map(&profile.classes, budget, |entry, budget| {
        record(
            schema,
            "NamedClass",
            [
                NdfValue::Text(text(&entry.name, budget)?),
                codec
                    .encode_presentation_class(&entry.class, budget)
                    .map_err(boundary)?,
            ],
            budget,
        )
    })?;
    let views = map(&profile.views, budget, |entry, budget| {
        record(
            schema,
            "NamedView",
            [
                NdfValue::Text(text(&entry.name, budget)?),
                codec
                    .encode_kind_ref(&entry.kind, budget)
                    .map_err(boundary)?,
            ],
            budget,
        )
    })?;
    let value = record(
        schema,
        "CompileProfile",
        [
            package,
            state,
            NdfValue::List(imports),
            NdfValue::List(extensions),
            NdfValue::List(classes),
            NdfValue::List(views),
        ],
        budget,
    )?;
    validate(&value, registry, budget)?;
    Ok(value)
}
/// Decode after structural validation. Unknown symbolic targets remain data;
/// native compilation performs catalog-name and selected-contract validation.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<CompileProfile, ProfileError<C::Error>> {
    budget.poll()?;
    validate(value, registry, budget)?;
    let schema = schema(registry)?;
    let values = fields(value, schema, "CompileProfile", 6)?;
    let package = read_text(&values[0], budget)?;
    let state_type = codec
        .decode_type_descriptor(&values[1], budget)
        .map_err(boundary)?;
    let reader_imports = map(list(&values[2])?, budget, |entry, budget| {
        let values = fields(entry, schema, "ReaderImport", 2)?;
        Ok(ReaderImport {
            provider: read_text(&values[0], budget)?,
            signature: reader::signature_from_value(&values[1], registry, codec, budget)
                .map_err(reader_error)?,
        })
    })?;
    let extensions = map(list(&values[3])?, budget, |entry, budget| {
        engine::extension_from_value(entry, registry, codec, budget).map_err(engine_error)
    })?;
    let classes = map(list(&values[4])?, budget, |entry, budget| {
        let values = fields(entry, schema, "NamedClass", 2)?;
        Ok(NamedClass {
            name: read_text(&values[0], budget)?,
            class: codec
                .decode_presentation_class(&values[1], budget)
                .map_err(boundary)?,
        })
    })?;
    let views = map(list(&values[5])?, budget, |entry, budget| {
        let values = fields(entry, schema, "NamedView", 2)?;
        Ok(NamedView {
            name: read_text(&values[0], budget)?,
            kind: codec
                .decode_kind_ref(&values[1], budget)
                .map_err(boundary)?,
        })
    })?;
    Ok(CompileProfile {
        package,
        state_type,
        reader_imports,
        extensions,
        classes,
        views,
    })
}
