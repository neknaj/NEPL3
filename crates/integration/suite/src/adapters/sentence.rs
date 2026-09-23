//! Explicit Sentence consumers. Each optional feature selects its output model.
#[cfg(feature = "doc-sentence")]
mod document;
#[cfg(feature = "doc-sentence")]
pub use document::{Error, document};
#[cfg(feature = "sentence-html")]
pub mod html;
