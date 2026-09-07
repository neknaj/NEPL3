//! Owned page-set closure and recomputed link plans. Raw data is never a proof.
use super::*;
use crate::pages::{self, PageDocument, PageLinkPlan, PageSet};
use alloc::vec::Vec;

pub fn set_to_value<C: FoundationValueCodec>(
    set: &PageSet,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let s = schema(r)?;
    let mut values = Vec::new();
    for page in &set.pages {
        b.charge(Resource::Work, 1)?;
        let registration = page.registration.put(s, c, b)?;
        let document = to_value(&page.document, r, c, b)?;
        let value = record(s, "PageDocument", [registration, document], b)?;
        pages::push(&mut values, value, b)?;
    }
    let value = record(s, "PageSet", [NdfValue::List(values)], b)?;
    text::check_type(&value, "PageSet", r, b)?;
    Ok(value)
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
    let f = fields(input, s, "PageSet", 1)?;
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
    Ok(PageSet { pages })
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
