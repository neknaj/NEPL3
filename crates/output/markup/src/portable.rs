//! Closed NDF value boundaries for HTML and MathML fragments. A caller must compare the
//! policy with its independent backend resource catalog before actual use.
pub mod mathml;
mod value;
use crate::html::{HtmlError, HtmlRequest, validate};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeRef},
    value::{NdfValue, SchemaRef},
    value_codec::FoundationValueCodec,
};
use value::Value;
#[derive(Debug, Eq, PartialEq)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Html(HtmlError),
    MathMl(crate::mathml::Error),
    Shape,
    Foundation(E),
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
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
impl<E> From<HtmlError> for PortableError<E> {
    fn from(e: HtmlError) -> Self {
        match e {
            HtmlError::Stopped(s) => Self::Stopped(s),
            e => Self::Html(e),
        }
    }
}
fn schema<E>(r: &SchemaRegistry) -> Result<&SchemaRef, PortableError<E>> {
    if !r.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    r.selected("nepl3.markup", 2)
        .ok_or(SchemaError::UnknownSchema.into())
}
fn check_named<E>(
    r: &SchemaRegistry,
    name: &str,
    v: &NdfValue,
    b: &mut Budget,
) -> Result<(), PortableError<E>> {
    b.charge(
        Resource::AllocationUnits,
        ("nepl3.markup".len() + name.len()) as u64,
    )?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "nepl3.markup".into(),
            revision: 2,
            name: name.into(),
        }),
        v,
        b,
    )?;
    Ok(())
}
pub fn to_value<C: FoundationValueCodec>(
    request: &HtmlRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    validate(&request.fragment, request.slot, &request.policy, b)?;
    let value = request.put(schema(r)?, c, b)?;
    check_named(r, "HtmlRequest", &value, b)?;
    Ok(value)
}
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<HtmlRequest, PortableError<C::Error>> {
    check_named(r, "HtmlRequest", value, b)?;
    let request = HtmlRequest::read(value, schema(r)?, c, b)?;
    validate(&request.fragment, request.slot, &request.policy, b)?;
    Ok(request)
}
