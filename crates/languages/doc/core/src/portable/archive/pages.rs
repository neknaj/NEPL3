//! Explicit structural PageSet archive. Resolution remains a separate operation.
use super::*;
use crate::{
    pages::{PageDocument, PageSet, push},
    schema::archive::pages as contract,
};
use alloc::vec::Vec;
fn schema<'a, E>(r: &'a SchemaRegistry, b: &mut Budget) -> Result<&'a SchemaRef, PortableError<E>> {
    r.selected_descriptor_with_budget(contract::PACKAGE, contract::REVISION, b)?
        .map(|(reference, _)| reference)
        .ok_or_else(|| SchemaError::UnknownSchema.into())
}
fn check<E>(v: &NdfValue, r: &SchemaRegistry, b: &mut Budget) -> Result<(), PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        (contract::PACKAGE.len() + contract::ROOT.len()) as u64,
    )?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: contract::PACKAGE.into(),
            revision: contract::REVISION,
            name: contract::ROOT.into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
/// Encode the complete archive envelope; does not establish link readiness.
pub fn to_value<C: FoundationGroupArchiveCodec>(
    set: &PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    to_value_with_structures(set, r, c, b).map(|encoded| encoded.value)
}
pub(crate) fn to_value_with_structures<'a, C: FoundationGroupArchiveCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<crate::portable::pages::PageSetEncoding<'a>, PortableError<C::Error>> {
    let s = schema(r, b)?;
    let doc = crate::portable::schema(r)?;
    let mut values = Vec::new();
    let mut structures = Vec::new();
    let mut components = Vec::new();
    for page in &set.pages {
        b.charge(Resource::Work, 1)?;
        let registration = page.registration.put(doc, c, b)?;
        let (encoded, structure) = super::to_value_with_structure(&page.document, r, c, b)?;
        push(&mut structures, structure, b)?;
        push(&mut components, encoded.guest_components, b)?;
        push(
            &mut values,
            record(s, contract::PAGE, [registration, encoded.value], b)?,
            b,
        )?;
    }
    let files = set.files.put(doc, c, b)?;
    let value = record(s, contract::ROOT, [NdfValue::List(values), files], b)?;
    check(&value, r, b)?;
    b.poll()?;
    Ok(crate::portable::pages::PageSetEncoding {
        value,
        structures,
        components,
    })
}
/// Decode with one codec/admission ledger; source redefinitions across pages reject.
/// Registrations, labels and links still need their consuming resolver.
pub fn from_value<C: FoundationGroupArchiveCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageSet, PortableError<C::Error>> {
    check(input, r, b)?;
    let s = schema(r, b)?;
    let doc = crate::portable::schema(r)?;
    let f = fields(input, s, contract::ROOT, 2)?;
    let NdfValue::List(items) = &f[0] else {
        return Err(PortableError::Shape);
    };
    let mut pages = Vec::new();
    for value in items {
        b.charge(Resource::Work, 1)?;
        let f = fields(value, s, contract::PAGE, 2)?;
        let registration = Value::read(&f[0], doc, c, b)?;
        let document = super::from_value(&f[1], r, c, b)?;
        push(
            &mut pages,
            PageDocument {
                registration,
                document,
            },
            b,
        )?;
    }
    let files = Value::read(&f[1], doc, c, b)?;
    b.poll()?;
    Ok(PageSet { pages, files })
}
