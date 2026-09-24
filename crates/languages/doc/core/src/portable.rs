//! Explicit Doc NDF schema adapters. Decoding produces raw data followed by the
//! same source/category/graph checks used by native callers, not a render proof.
pub mod pages;
pub mod prepare;
pub mod print;
pub mod text;
mod value;
use crate::{check::StructureError, model::DocumentSyntax};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::SourceStore,
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use value::{Value, fields, record};
/// Encode the explicit input variant for identity binding. Semantic consumption
/// of a typed Sentence payload remains the selected language's responsibility.
pub fn embed_value<C: FoundationValueCodec>(
    input: &crate::model::DocEmbed,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    input.put(schema(registry)?, codec, budget)
}
/// Borrow embeds from a DocumentSyntax value generated and validated by this
/// crate in the same call. This accessor supplies no proof for incoming data.
pub(crate) fn embedded_values<'a, E>(
    value: &'a NdfValue,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<&'a [NdfValue], PortableError<E>> {
    budget.charge(Resource::Work, 1)?;
    let schema = schema(registry)?;
    let document = fields(value, schema, "DocumentSyntax", 5)?;
    let value = fields(&document[0], schema, "DocValue", 3)?;
    match &value[2] {
        NdfValue::List(values) => Ok(values),
        _ => Err(PortableError::Shape),
    }
}
pub(crate) fn label_arguments<C: FoundationValueCodec>(
    name: &str,
    paths: Option<&crate::model::LabelOccurrencePaths>,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<nepl3_core::value::TypedValue, PortableError<C::Error>> {
    let s = schema(registry)?;
    b.charge(Resource::Work, name.len() as u64)?;
    b.charge(Resource::AllocationUnits, name.len() as u64)?;
    let paths = match paths {
        Some(paths) => {
            let value = paths.put(s, c, b)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            NdfValue::Some(alloc::boxed::Box::new(value))
        }
        None => NdfValue::None,
    };
    let kind = "LabelDiagnosticArguments";
    b.charge(Resource::Work, (s.package.len() + kind.len() + 1) as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (s.package.len() + kind.len() + 2 * core::mem::size_of::<NdfValue>()) as u64,
    )?;
    Ok(nepl3_core::value::TypedValue::Record(
        nepl3_core::value::Record {
            schema: s.clone(),
            kind: kind.into(),
            fields: alloc::vec![NdfValue::Text(name.into()), paths],
        },
    ))
}
#[derive(Debug, Eq, PartialEq)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Foundation(E),
    Schema(SchemaError),
    Structure(StructureError),
    Shape,
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl<E> From<SchemaError> for PortableError<E> {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(s) => Self::Stopped(s),
            v => Self::Schema(v),
        }
    }
}
impl<E> From<StructureError> for PortableError<E> {
    fn from(v: StructureError) -> Self {
        match v {
            StructureError::Stopped(s) => Self::Stopped(s),
            v => Self::Structure(v),
        }
    }
}
fn boundary<E: FoundationCodecError>(e: E) -> PortableError<E> {
    match e.stop_reason() {
        Some(s) => PortableError::Stopped(s),
        None => PortableError::Foundation(e),
    }
}
fn schema<E>(registry: &SchemaRegistry) -> Result<&SchemaRef, PortableError<E>> {
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected("nepl3.doc", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn check<E>(
    registry: &SchemaRegistry,
    v: &NdfValue,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, 26)?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.doc".into(),
            revision: 1,
            name: "DocumentSyntax".into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
pub fn to_value<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let (value, _) = encode_with_structure(document, registry, c, b)?;
    check(registry, &value, b)?;
    Ok(value)
}

// Retain the native immutable proof for the same operation's label traversal.
// The caller must validate the generated NDF at its output boundary: either
// DocumentSyntax in to_value, or the enclosing PageSet in pages::set_to_value.
// This helper retains all native structure and foundation codec validation.
fn encode_with_structure<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(NdfValue, crate::check::ValidatedDocumentSyntax<'a>), PortableError<C::Error>> {
    let structure = document.validate_structure(registry, b, c.source_admission())?;
    let s = schema(registry)?;
    let sources = c.encode_sources(&document.sources, b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in &document.sources {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped_with_mappings(&store, &document.source_maps);
    let value = document.value.put(s, &mut scoped, b)?;
    let origins = scoped
        .encode_origins(&document.origins, b)
        .map_err(boundary)?;
    let views = document.views.put(s, &mut scoped, b)?;
    let maps = document.source_maps.put(s, &mut scoped, b)?;
    let output = record(
        s,
        "DocumentSyntax",
        [value, sources, origins, views, maps],
        b,
    )?;
    Ok((output, structure))
}
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    registry: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocumentSyntax, PortableError<C::Error>> {
    check(registry, input, b)?;
    let s = schema(registry)?;
    let fields = fields(input, s, "DocumentSyntax", 5)?;
    let sources = c.decode_sources(&fields[1], b).map_err(boundary)?;
    let mut store = SourceStore::default();
    for source in &sources {
        store
            .insert_with_budget(source.clone_with_budget(b)?, b)
            .map_err(StructureError::from)?;
    }
    let mut scoped = c.scoped(&store);
    let value = Value::read(&fields[0], s, &mut scoped, b)?;
    let origins = scoped.decode_origins(&fields[2], b).map_err(boundary)?;
    let source_maps: alloc::vec::Vec<nepl3_core::origin::Mapping> =
        Value::read(&fields[4], s, &mut scoped, b)?;
    let views = {
        let mut mapped = scoped.scoped_with_mappings(&store, &source_maps);
        Value::read(&fields[3], s, &mut mapped, b)?
    };
    let output = DocumentSyntax {
        value,
        sources,
        origins,
        views,
        source_maps,
    };
    output.validate_structure(registry, b, scoped.source_admission())?;
    Ok(output)
}
