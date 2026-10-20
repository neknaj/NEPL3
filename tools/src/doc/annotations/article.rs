//! Structural Sentence owner selection for an exact outer Article. Display
//! selection and Article namespace adoption are separate host operations.
pub mod namespace;
mod selection;
pub use selection::{
    Error, Owner, Selection, SelectionParts, SentenceOccurrence, collect_structural,
};

pub mod render;
