//! Package metadata owns surface syntax; domain operations are separate registrations.
mod bindings;
mod check;
pub(crate) mod identity;
mod model;
mod reader;
mod shape;
pub use check::{CheckedLanguagePackage, PackageError};
pub use model::*;
