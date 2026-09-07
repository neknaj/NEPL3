use super::*;
use crate::pages::{PagesHtmlRequest, PagesRenderError, RenderedPages, render_pages};
#[derive(Debug, Eq, PartialEq)]
pub enum PagesPortableError<'a, E> {
    Boundary(PortableError<E>),
    Render(PagesRenderError<'a, E>),
}
impl<E> From<PortableError<E>> for PagesPortableError<'_, E> {
    fn from(e: PortableError<E>) -> Self {
        Self::Boundary(e)
    }
}
pub fn request_to_value<C: FoundationValueCodec>(
    request: &PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let value = request.put(schema(r)?, r, c, b)?;
    check(&value, "PagesHtmlRequest", r, b)?;
    Ok(value)
}
pub fn request_from_value<C: FoundationValueCodec>(
    v: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PagesHtmlRequest, PortableError<C::Error>> {
    check(v, "PagesHtmlRequest", r, b)?;
    PagesHtmlRequest::read(v, schema(r)?, r, c, b)
}
pub fn rendered_to_value<'a, C: FoundationValueCodec>(
    rendered: &RenderedPages,
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PagesPortableError<'a, C::Error>> {
    let actual = render_pages(request, r, c, b).map_err(PagesPortableError::Render)?;
    let value = rendered.put(schema(r)?, r, c, b)?;
    let expected = actual.put(schema(r)?, r, c, b)?;
    check(&value, "RenderedPages", r, b)?;
    if canonical(&value, c, b)? != canonical(&expected, c, b)? {
        return Err(PortableError::Mismatch.into());
    }
    Ok(value)
}
pub fn rendered_from_value<'a, C: FoundationValueCodec>(
    v: &NdfValue,
    request: &'a PagesHtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<RenderedPages, PagesPortableError<'a, C::Error>> {
    check(v, "RenderedPages", r, b)?;
    let rendered = RenderedPages::read(v, schema(r)?, r, c, b)?;
    rendered_to_value(&rendered, request, r, c, b)?;
    Ok(rendered)
}
