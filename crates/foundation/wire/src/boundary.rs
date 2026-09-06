//! Shared mechanics for typed foundation adapters; no language-specific dispatch.
use crate::{WireError, source::*, view::*};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    source::{SourceSnapshot, SourceStore},
    value::{NdfValue, SchemaRef, Variant},
};

pub(crate) fn variant<const N: usize>(
    schema: &SchemaRef,
    name: &str,
    case: &str,
    fields: [NdfValue; N],
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    budget.charge(
        Resource::AllocationUnits,
        (schema.package.len() + name.len() + case.len() + N * core::mem::size_of::<NdfValue>())
            as u64,
    )?;
    Ok(NdfValue::Variant(Variant {
        schema: schema.clone(),
        type_name: name.into(),
        variant: case.into(),
        fields: Vec::from(fields),
    }))
}
pub(crate) fn variant_parts<'a>(
    value: &'a NdfValue,
    schema: &SchemaRef,
    name: &str,
) -> Result<(&'a str, &'a [NdfValue]), WireError> {
    match value {
        NdfValue::Variant(v) if &v.schema == schema && v.type_name == name => {
            Ok((&v.variant, &v.fields))
        }
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn option<T>(
    value: Option<&T>,
    budget: &mut Budget,
    convert: impl FnOnce(&T, &mut Budget) -> Result<NdfValue, WireError>,
) -> Result<NdfValue, WireError> {
    match value {
        None => Ok(NdfValue::None),
        Some(value) => {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NdfValue>() as u64,
            )?;
            Ok(NdfValue::Some(Box::new(convert(value, budget)?)))
        }
    }
}
pub(crate) fn option_from<T>(
    value: &NdfValue,
    budget: &mut Budget,
    convert: impl FnOnce(&NdfValue, &mut Budget) -> Result<T, WireError>,
) -> Result<Option<T>, WireError> {
    match value {
        NdfValue::None => Ok(None),
        NdfValue::Some(value) => Ok(Some(convert(value, budget)?)),
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn id_value(
    id: u64,
    name: &str,
    schema: &SchemaRef,
    budget: &mut Budget,
) -> Result<NdfValue, WireError> {
    record(schema, name, [NdfValue::U64(id)], budget)
}
pub(crate) fn id_from(value: &NdfValue, name: &str, schema: &SchemaRef) -> Result<u64, WireError> {
    as_u64(&fields(value, schema, name, 1)?[0])
}
pub(crate) fn bytes(value: &[u8], budget: &mut Budget) -> Result<NdfValue, WireError> {
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(NdfValue::Bytes(value.to_vec()))
}
pub(crate) fn bytes_from(value: &NdfValue, budget: &mut Budget) -> Result<Vec<u8>, WireError> {
    match value {
        NdfValue::Bytes(value) => {
            budget.charge(Resource::AllocationUnits, value.len() as u64)?;
            Ok(value.clone())
        }
        _ => Err(WireError::InvalidType),
    }
}
pub(crate) fn store(
    snapshots: &[SourceSnapshot],
    budget: &mut Budget,
) -> Result<SourceStore, WireError> {
    let mut store = SourceStore::default();
    for source in snapshots {
        budget.charge(
            Resource::AllocationUnits,
            (source.identity().source.0.len()
                + source.uri().len()
                + source.text().len()
                + core::mem::size_of::<SourceSnapshot>()) as u64,
        )?;
        store.insert(source.clone())?;
    }
    Ok(store)
}
pub(crate) fn sequence<T>(
    items: &[T],
    budget: &mut Budget,
    convert: impl FnMut(&T, &mut Budget) -> Result<NdfValue, WireError>,
) -> Result<NdfValue, WireError> {
    Ok(NdfValue::List(collect(items, budget, convert)?))
}
