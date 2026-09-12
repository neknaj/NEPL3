//! Neutral MathML structure boundary. Decoding proves no layout or guest semantics.
use super::*;
use crate::mathml::{Error, Fragment, validate};

impl<E> From<Error> for PortableError<E> {
    fn from(error: Error) -> Self {
        match error {
            Error::Stopped(reason) => Self::Stopped(reason),
            error => Self::MathMl(error),
        }
    }
}

/// Encode only structurally valid MathML using the selected schema digest.
pub fn to_value<C: FoundationValueCodec>(
    fragment: &Fragment,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    validate(fragment, b)?;
    let value = fragment.put(schema(r)?, c, b)?;
    check_named(r, "MathMlFragment", &value, b)?;
    Ok(value)
}

/// Validate the neutral schema before reconstruction, then recheck all structure.
/// The caller retains the same budget through transport, validation and rendering.
pub fn from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<Fragment, PortableError<C::Error>> {
    check_named(r, "MathMlFragment", value, b)?;
    let fragment = Fragment::read(value, schema(r)?, c, b)?;
    validate(&fragment, b)?;
    Ok(fragment)
}
