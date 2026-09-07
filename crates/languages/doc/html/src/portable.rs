//! Explicit raw request transport and replay-checked rendered values. A
//! received fragment never manufactures a prepared document or asset authority.
mod value;
use crate::*;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    value::{NdfValue, SchemaRef},
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use value::Value;
#[derive(Debug, Eq, PartialEq)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Doc(nepl3_doc_core::portable::PortableError<E>),
    Markup(nepl3_markup::portable::PortableError<E>),
    Foundation(E),
    Render(RenderError),
    Shape,
    Mismatch,
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl<E> From<SchemaError> for PortableError<E> {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::Stopped(s) => Self::Stopped(s),
            e => Self::Schema(e),
        }
    }
}
fn schema<E>(r: &SchemaRegistry) -> Result<&SchemaRef, PortableError<E>> {
    if !r.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    r.selected("nepl3.doc.html", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn check<E>(
    v: &NdfValue,
    name: &str,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(Resource::AllocationUnits, (name.len() + 14) as u64)?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.doc.html".into(),
            revision: 1,
            name: name.into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
pub fn request_to_value<C: FoundationValueCodec>(
    request: &LocalHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = request.put(schema(r)?, r, c, b)?;
    check(&value, "LocalHtmlRequest", r, b)?;
    Ok(value)
}
pub fn request_from_value<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<LocalHtmlRequest, PortableError<C::Error>> {
    check(v, "LocalHtmlRequest", r, b)?;
    LocalHtmlRequest::read(v, schema(r)?, r, c, b)
}
fn canonical<C: FoundationValueCodec>(
    v: &NdfValue,
    c: &mut C,
    b: &mut Budget,
) -> Result<Digest, PortableError<C::Error>> {
    c.canonical_value_digest(b"NEPL3.Doc.Html.Fragment.v1\0", v, b)
        .map_err(|e| match e.stop_reason() {
            Some(s) => PortableError::Stopped(s),
            None => PortableError::Foundation(e),
        })
}
fn replay<C: FoundationValueCodec>(
    v: &NdfValue,
    prepared: &PreparedLocalArticle<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<(), PortableError<C::Error>> {
    let actual = render(prepared, b).map_err(|e| match e {
        RenderError::Stopped(s) => PortableError::Stopped(s),
        e => PortableError::Render(e),
    })?;
    let actual = actual.put(schema(r)?, r, c, b)?;
    if canonical(v, c, b)? != canonical(&actual, c, b)? {
        return Err(PortableError::Mismatch);
    }
    Ok(())
}
pub fn rendered_to_value<C: FoundationValueCodec>(
    fragment: &RenderedFragment,
    prepared: &PreparedLocalArticle<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = fragment.put(schema(r)?, r, c, b)?;
    check(&value, "RenderedFragment", r, b)?;
    replay(&value, prepared, r, c, b)?;
    Ok(value)
}
pub fn rendered_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    prepared: &PreparedLocalArticle<'_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenderedFragment, PortableError<C::Error>> {
    check(value, "RenderedFragment", r, b)?;
    let fragment = RenderedFragment::read(value, schema(r)?, r, c, b)?;
    replay(value, prepared, r, c, b)?;
    Ok(fragment)
}
