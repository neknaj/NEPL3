//! NDF/1 canonical CBOR profile. Raw codecs check intrinsic values; checked
//! boundaries additionally require a finalized schema registry and expected type.
#![no_std]
extern crate alloc;

mod boundary;
mod decode;
mod encode;
pub mod environment;
pub mod facts;
pub mod foundation;
pub mod origin;
pub mod source;
pub mod syntax;
pub mod view;

use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, StopReason},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    value::{NdfValue, NumberError},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireError {
    Stopped(StopReason),
    UnexpectedEnd,
    TrailingData,
    NonCanonical,
    UnreachableNode,
    InvalidType,
    InvalidTag,
    InvalidLength,
    InvalidUtf8,
    Number(NumberError),
    Schema(SchemaError),
    Source(nepl3_core::source::SourceError),
    View(nepl3_core::view::ViewError),
    Origin(nepl3_core::origin::OriginError),
    Syntax(nepl3_core::syntax::SyntaxError),
    Facts(nepl3_core::facts::FactError),
}
impl From<nepl3_core::facts::FactError> for WireError {
    fn from(value: nepl3_core::facts::FactError) -> Self {
        match value {
            nepl3_core::facts::FactError::Stopped(r) => Self::Stopped(r),
            v => Self::Facts(v),
        }
    }
}
impl From<StopReason> for WireError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<NumberError> for WireError {
    fn from(value: NumberError) -> Self {
        Self::Number(value)
    }
}
impl From<SchemaError> for WireError {
    fn from(value: SchemaError) -> Self {
        Self::Schema(value)
    }
}
impl From<nepl3_core::source::SourceError> for WireError {
    fn from(value: nepl3_core::source::SourceError) -> Self {
        match value {
            nepl3_core::source::SourceError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Source(error),
        }
    }
}
impl From<nepl3_core::view::ViewError> for WireError {
    fn from(value: nepl3_core::view::ViewError) -> Self {
        match value {
            nepl3_core::view::ViewError::Stopped(reason) => Self::Stopped(reason),
            error => Self::View(error),
        }
    }
}
impl From<nepl3_core::origin::OriginError> for WireError {
    fn from(value: nepl3_core::origin::OriginError) -> Self {
        match value {
            nepl3_core::origin::OriginError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Origin(error),
        }
    }
}
impl From<nepl3_core::syntax::SyntaxError> for WireError {
    fn from(value: nepl3_core::syntax::SyntaxError) -> Self {
        match value {
            nepl3_core::syntax::SyntaxError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Syntax(error),
        }
    }
}

/// Decode one whole intrinsic value. Domain schema checks require `decode_checked`.
pub fn decode(bytes: &[u8], budget: &mut Budget) -> Result<NdfValue, WireError> {
    decode::decode(bytes, budget)
}

/// Encode intrinsic values canonically, without asserting domain semantic validity.
pub fn encode(value: &NdfValue, budget: &mut Budget) -> Result<Vec<u8>, WireError> {
    encode::encode(value, budget)
}

/// Owned structural proof. Domain-specific invariants still require domain check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralValue {
    value: NdfValue,
}
impl StructuralValue {
    pub fn value(&self) -> &NdfValue {
        &self.value
    }
    pub fn into_value(self) -> NdfValue {
        self.value
    }
}

pub fn decode_checked(
    bytes: &[u8],
    expected: &TypeDescriptor,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<StructuralValue, WireError> {
    let value = decode(bytes, budget)?;
    registry.validate(expected, &value, budget)?;
    Ok(StructuralValue { value })
}

pub fn encode_checked(
    value: &NdfValue,
    expected: &TypeDescriptor,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    registry.validate(expected, value, budget)?;
    encode(value, budget)
}
