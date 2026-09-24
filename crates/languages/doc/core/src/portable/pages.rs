//! Owned page-set closure and recomputed link plans. Raw data is never a proof.
use super::*;
use crate::pages::{self, PageDocument, PageLinkPlan, PageSet};
use alloc::vec::Vec;

/// Consume an already checked, locally generated PageSet into the v2 namespace
/// identity input. Root documents are bound by member-0 digests, so their full
/// canonical values need not be hashed a second time. This is not a decoder.
pub(crate) fn namespace_identity_input<E>(
    mut value: NdfValue,
    members: Vec<NdfValue>,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    let NdfValue::Record(set) = &mut value else {
        return Err(PortableError::Shape);
    };
    let [mut pages, files]: [NdfValue; 2] = core::mem::take(&mut set.fields)
        .try_into()
        .map_err(|_| PortableError::Shape)?;
    let NdfValue::List(pages) = &mut pages else {
        return Err(PortableError::Shape);
    };
    let mut registrations = Vec::new();
    for mut page in core::mem::take(pages) {
        b.charge(Resource::Work, 1)?;
        let NdfValue::Record(page) = &mut page else {
            return Err(PortableError::Shape);
        };
        let [registration, _document]: [NdfValue; 2] = core::mem::take(&mut page.fields)
            .try_into()
            .map_err(|_| PortableError::Shape)?;
        pages::push(&mut registrations, registration, b)?;
    }
    let mut fields = Vec::new();
    for value in [
        NdfValue::List(registrations),
        files,
        NdfValue::List(members),
    ] {
        pages::push(&mut fields, value, b)?;
    }
    Ok(NdfValue::List(fields))
}

/// Borrow a generated PageSet's document value. This shape accessor is private
/// to the crate and does not grant validation to arbitrary incoming NDF values.
pub(crate) fn document_value<'a, E>(
    value: &'a NdfValue,
    page: usize,
    b: &mut Budget,
) -> Result<&'a NdfValue, PortableError<E>> {
    b.charge(Resource::Work, 1)?;
    let NdfValue::Record(set) = value else {
        return Err(PortableError::Shape);
    };
    let [NdfValue::List(pages), _] = set.fields.as_slice() else {
        return Err(PortableError::Shape);
    };
    let Some(NdfValue::Record(page)) = pages.get(page) else {
        return Err(PortableError::Shape);
    };
    let [_, document] = page.fields.as_slice() else {
        return Err(PortableError::Shape);
    };
    Ok(document)
}

pub fn set_to_value<C: FoundationValueCodec>(
    set: &PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    set_to_value_with_structures(set, r, c, b).map(|(value, _)| value)
}

pub(crate) fn set_to_value_with_structures<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(NdfValue, Vec<crate::check::ValidatedDocumentSyntax<'a>>), PortableError<C::Error>> {
    let s = schema(r)?;
    let mut values = Vec::new();
    let mut structures = Vec::new();
    for page in &set.pages {
        b.charge(Resource::Work, 1)?;
        let registration = page.registration.put(s, c, b)?;
        let (document, structure) = encode_with_structure(&page.document, r, c, b)?;
        pages::push(&mut structures, structure, b)?;
        let value = record(s, "PageDocument", [registration, document], b)?;
        pages::push(&mut values, value, b)?;
    }
    let files = set.files.put(s, c, b)?;
    let value = record(s, "PageSet", [NdfValue::List(values), files], b)?;
    // This recursively validates every DocumentSyntax, registration and file.
    // Checking each document again before this traversal duplicates the same
    // schema work. No generated value leaves this boundary before this check.
    text::check_type(&value, "PageSet", r, b)?;
    Ok((value, structures))
}
/// Structural receiver. Call pages::resolve to check registrations, actual
/// labels and links. Separate documents cannot redefine one source revision.
pub fn set_from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageSet, PortableError<C::Error>> {
    text::check_type(input, "PageSet", r, b)?;
    let s = schema(r)?;
    let f = fields(input, s, "PageSet", 2)?;
    let NdfValue::List(items) = &f[0] else {
        return Err(PortableError::Shape);
    };
    let mut pages = Vec::new();
    for value in items {
        b.charge(Resource::Work, 1)?;
        let f = fields(value, s, "PageDocument", 2)?;
        let registration = Value::read(&f[0], s, c, b)?;
        let document = from_value(&f[1], r, c, b)?;
        pages::push(
            &mut pages,
            PageDocument {
                registration,
                document,
            },
            b,
        )?;
    }
    let files = Value::read(&f[1], s, c, b)?;
    Ok(PageSet { pages, files })
}
pub fn plan_to_value<'a, C: FoundationValueCodec>(
    plan: &PageLinkPlan,
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, pages::PageError<'a, C::Error>> {
    let checked = pages::resolve(set, r, c, b)?;
    let actual = checked.plan().put(schema(r)?, c, b)?;
    let supplied = plan.put(schema(r)?, c, b)?;
    let left = c
        .canonical_value_digest(b"NEPL3.Doc.PagePlan.v1\0", &actual, b)
        .map_err(boundary)?;
    let right = c
        .canonical_value_digest(b"NEPL3.Doc.PagePlan.v1\0", &supplied, b)
        .map_err(boundary)?;
    if left != right {
        return Err(PortableError::Shape.into());
    }
    text::check_type(&supplied, "PageLinkPlan", r, b)?;
    Ok(supplied)
}
pub fn plan_from_value<'a, C: FoundationValueCodec>(
    input: &NdfValue,
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageLinkPlan, pages::PageError<'a, C::Error>> {
    text::check_type(input, "PageLinkPlan", r, b)?;
    let plan = PageLinkPlan::read(input, schema(r)?, c, b)?;
    plan_to_value(&plan, set, r, c, b)?;
    Ok(plan)
}
