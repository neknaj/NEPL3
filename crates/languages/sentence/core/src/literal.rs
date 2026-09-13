//! Sentence recognition uses only core source and schema contracts. Host reader
//! adapters own any ReaderSession/engine envelope around this operation.
mod parser;
mod print;
use crate::syntax::SentenceSyntax;
use nepl3_core::{
    budget::StopReason,
    schema::SchemaError,
    source::{SourceError, SourceSnapshot, Span},
};
pub use print::{PrintError, print};

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
    Shape(crate::check::Error),
    Syntax(crate::syntax::Error),
    SchemaIdentity,
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
impl From<crate::check::Error> for SentenceError {
    fn from(v: crate::check::Error) -> Self {
        match v {
            crate::check::Error::Stopped(s) => Self::Stopped(s),
            v => Self::Shape(v),
        }
    }
}
impl From<crate::syntax::Error> for SentenceError {
    fn from(v: crate::syntax::Error) -> Self {
        match v {
            crate::syntax::Error::Stopped(s) => Self::Stopped(s),
            other => Self::Syntax(other),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
pub struct SentenceLiteral {
    pub head: Span,
    pub syntax: SentenceSyntax,
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
