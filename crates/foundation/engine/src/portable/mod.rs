//! Engine-owned typed NDF adapters. The host composes these with a wire codec;
//! production engine and wire never depend on one another.
pub mod tree;
mod value;
use nepl3_core::{
    budget::StopReason, schema::SchemaError, source::SourceError,
    syntax::canonical::CanonicalError, value_codec::FoundationCodecError,
};

#[derive(Debug)]
pub enum PortableError<E> {
    Stopped(StopReason),
    Boundary(E),
    Schema(SchemaError),
    Source(SourceError),
    Tree(crate::tree::TreeError),
    Canonical(CanonicalError),
    Shape,
    NonCanonical,
}
impl<E> From<StopReason> for PortableError<E> {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl<E> From<SchemaError> for PortableError<E> {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(r) => Self::Stopped(r),
            v => Self::Schema(v),
        }
    }
}
impl<E> From<SourceError> for PortableError<E> {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(r) => Self::Stopped(r),
            v => Self::Source(v),
        }
    }
}
impl<E> From<CanonicalError> for PortableError<E> {
    fn from(v: CanonicalError) -> Self {
        match v {
            CanonicalError::Stopped(r) => Self::Stopped(r),
            v => Self::Canonical(v),
        }
    }
}
impl<E> From<crate::tree::TreeError> for PortableError<E> {
    fn from(v: crate::tree::TreeError) -> Self {
        match v {
            crate::tree::TreeError::Stopped(r) => Self::Stopped(r),
            v => Self::Tree(v),
        }
    }
}
fn boundary<E: FoundationCodecError>(v: E) -> PortableError<E> {
    match v.stop_reason() {
        Some(r) => PortableError::Stopped(r),
        None => PortableError::Boundary(v),
    }
}
