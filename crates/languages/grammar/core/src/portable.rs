//! Source-closed Grammar Document value boundary. A successful round trip proves
//! constructor/source invariants, not parsing provenance or successful compilation.
mod value;
use crate::model::{Document, ModelError};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    source::SourceStore,
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use value::{Value, fields, record};

#[derive(Debug, Eq, PartialEq)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Foundation(E),
    Schema(SchemaError),
    Model(ModelError),
    Shape,
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl<E> From<SchemaError> for PortableError<E> {
    fn from(value: SchemaError) -> Self {
        match value {
            SchemaError::Stopped(stop) => Self::Stopped(stop),
            value => Self::Schema(value),
        }
    }
}
impl<E> From<ModelError> for PortableError<E> {
    fn from(value: ModelError) -> Self {
        match value {
            ModelError::Stopped(stop) => Self::Stopped(stop),
            value => Self::Model(value),
        }
    }
}
fn boundary<E: FoundationCodecError>(value: E) -> PortableError<E> {
    match value.stop_reason() {
        Some(stop) => PortableError::Stopped(stop),
        None => PortableError::Foundation(value),
    }
}
fn schema<E>(registry: &SchemaRegistry) -> Result<&SchemaRef, PortableError<E>> {
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected("nepl3.grammar", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn check<E>(
    registry: &SchemaRegistry,
    raw: &NdfValue,
    budget: &mut Budget,
) -> Result<(), PortableError<E>> {
    budget.charge(
        Resource::AllocationUnits,
        ("nepl3.grammar".len() + "Document".len()) as u64,
    )?;
    registry.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.grammar".into(),
            revision: 1,
            name: "Document".into(),
        }),
        raw,
        budget,
    )?;
    Ok(())
}
fn sources<E>(document: &Document, budget: &mut Budget) -> Result<SourceStore, PortableError<E>> {
    let mut store = SourceStore::default();
    for source in &document.sources {
        store
            .insert_with_budget(source.clone_with_budget(budget)?, budget)
            .map_err(ModelError::from)?;
    }
    Ok(store)
}
/// Encode a validated, explicitly declared source closure using the shared ledger.
pub fn to_value<C: FoundationValueCodec>(
    document: &Document,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    document.validate(budget, codec.source_admission())?;
    let schema = schema(registry)?;
    let declarations = codec
        .encode_sources(&document.sources, budget)
        .map_err(boundary)?;
    let store = sources(document, budget)?;
    let mut scoped = codec.scoped(&store);
    let nodes = document.nodes.put(schema, &mut scoped, budget)?;
    let root = document.root.put(schema, &mut scoped, budget)?;
    let value = record(schema, "Document", [declarations, nodes, root], budget)?;
    check(registry, &value, budget)?;
    Ok(value)
}
/// Decode spans only against this document's declared sources, never ambient ones.
/// The foundation wire boundary canonicalizes declaration order; native source
/// vector order is not part of this round-trip guarantee.
pub fn from_value<C: FoundationValueCodec>(
    raw: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<Document, PortableError<C::Error>> {
    check(registry, raw, budget)?;
    let schema = schema(registry)?;
    let fields = fields(raw, schema, "Document", 3)?;
    let declared = codec.decode_sources(&fields[0], budget).map_err(boundary)?;
    let mut document = Document {
        sources: declared,
        nodes: alloc::vec::Vec::new(),
        root: crate::model::NodeId(0),
    };
    let store = sources(&document, budget)?;
    let mut scoped = codec.scoped(&store);
    document.nodes = Value::read(&fields[1], schema, &mut scoped, budget)?;
    document.root = Value::read(&fields[2], schema, &mut scoped, budget)?;
    document.validate(budget, scoped.source_admission())?;
    Ok(document)
}
