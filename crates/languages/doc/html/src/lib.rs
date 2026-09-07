#![no_std]
extern crate alloc;

mod build;
pub mod portable;
mod prepare;
pub mod schema;
use alloc::{string::String, vec::Vec};
use nepl3_core::{budget::StopReason, source::Digest};
use nepl3_markup::html::{HtmlError, HtmlRequest};
pub use prepare::{LocalPreparationError, PreparedLocalArticle, prepare_local};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParallelMode {
    Rows,
    Columns,
    Single {
        language: String,
        fallbacks: Vec<String>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    pub parallel: ParallelMode,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalHtmlRequest {
    pub document: nepl3_doc_core::model::DocumentSyntax,
    pub options: RenderOptions,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementOrigin {
    pub element: u64,
    pub node: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedFragment {
    pub document_digest: Digest,
    pub options: RenderOptions,
    pub markup: HtmlRequest,
    /// Each emitted element/text's semantic cause, in the explicit document.
    /// These are not fabricated generated-source Spans.
    pub origins: Vec<ElementOrigin>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum RenderError {
    Stopped(StopReason),
    Markup(HtmlError),
    OutputDepth { node: u64 },
    InternalShape,
}
impl From<StopReason> for RenderError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<HtmlError> for RenderError {
    fn from(e: HtmlError) -> Self {
        match e {
            HtmlError::Stopped(s) => Self::Stopped(s),
            e => Self::Markup(e),
        }
    }
}
pub use build::render;

/// Fixed backend resource; a future document shell includes these exact bytes.
pub const STYLESHEET: &str = include_str!("../assets/doc.css");
