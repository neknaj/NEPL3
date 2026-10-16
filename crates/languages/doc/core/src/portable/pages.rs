//! Owned page-set closure and recomputed link plans. Raw data is never a proof.
use super::*;
use crate::pages::{self, PageDocument, PageLinkPlan, PageSet};
use alloc::vec::Vec;

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
    encode_set(set, r, c, b, None)
}
pub(crate) struct PageSetEncoding<'a> {
    pub(crate) value: NdfValue,
    pub(crate) structures: Vec<crate::check::ValidatedDocumentSyntax<'a>>,
    pub(crate) components: Vec<Vec<nepl3_core::source::Digest>>,
}

/// Same checked PageSet boundary plus identities for each exact encoded group.
pub(crate) fn set_to_value_with_components<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageSetEncoding<'a>, PortableError<C::Error>> {
    let mut components = Vec::new();
    let (value, structures) = encode_set(set, r, c, b, Some(&mut components))?;
    if components.len() != set.pages.len() {
        return Err(PortableError::Shape);
    }
    Ok(PageSetEncoding {
        value,
        structures,
        components,
    })
}
fn encode_set<'a, C: FoundationValueCodec>(
    set: &'a PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
    mut components: Option<&mut Vec<Vec<nepl3_core::source::Digest>>>,
) -> Result<(NdfValue, Vec<crate::check::ValidatedDocumentSyntax<'a>>), PortableError<C::Error>> {
    let s = schema(r)?;
    let mut values = Vec::new();
    let mut structures = Vec::new();
    for page in &set.pages {
        b.charge(Resource::Work, 1)?;
        let registration = page.registration.put(s, c, b)?;
        let (document, structure) = if let Some(all_components) = components.as_deref_mut() {
            let mut page_components = Vec::new();
            let encoded = encode_structure(&page.document, r, c, b, Some(&mut page_components))?;
            if page_components.len() != page.document.value.embeds.group.entries.len() {
                return Err(PortableError::Shape);
            }
            pages::push(all_components, page_components, b)?;
            encoded
        } else {
            encode_with_structure(&page.document, r, c, b)?
        };
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
