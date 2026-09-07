//! Sentence recognition uses only core source and schema contracts. Host reader
//! adapters own any ReaderSession/engine envelope around this operation.
mod parser;
use crate::model::{DocValue, DocView};
use alloc::vec::Vec;
use nepl3_core::{
    budget::StopReason,
    origin::Origin,
    schema::SchemaError,
    source::{SourceError, SourceSnapshot, Span},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SentenceCode {
    UnterminatedLiteral,
    UnclosedAnnotation,
    UnexpectedDelimiter,
    SeparatorCount,
    EmptyAnnotationPart,
    DirectLineBreak,
    InvalidEscape,
    InvalidScalar,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentenceFailure {
    pub code: SentenceCode,
    pub primary: Span,
    pub opening: Option<Span>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum SentenceError {
    Stopped(StopReason),
    Source(SourceError),
    Schema(SchemaError),
    Shape(crate::check::ShapeError),
}
impl From<StopReason> for SentenceError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SourceError> for SentenceError {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(s) => Self::Stopped(s),
            v => Self::Source(v),
        }
    }
}
impl From<SchemaError> for SentenceError {
    fn from(v: SchemaError) -> Self {
        match v {
            SchemaError::Stopped(s) => Self::Stopped(s),
            v => Self::Schema(v),
        }
    }
}
impl From<crate::check::ShapeError> for SentenceError {
    fn from(v: crate::check::ShapeError) -> Self {
        match v {
            crate::check::ShapeError::Stopped(s) => Self::Stopped(s),
            v => Self::Shape(v),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct SentenceLiteral {
    pub head: Span,
    pub value: DocValue,
    pub origins: Vec<Origin>,
    pub view: DocView,
}
#[derive(Debug, Eq, PartialEq)]
pub enum SentenceOutcome {
    Matched(SentenceLiteral),
    NoMatch,
    NeedMore,
    Failed(SentenceFailure),
}
/// Every position and local Origin/View remains bound to this original immutable
/// source. No synthetic prefix or decoded snapshot stands in for the input.
pub struct SentenceScan<'a> {
    pub source: &'a SourceSnapshot,
    pub outcome: SentenceOutcome,
}
pub use parser::read;
