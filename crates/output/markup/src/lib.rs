#![no_std]
//! Checked markup primitives. Full tree, resource and document serialization
//! build on these primitives; text escaping alone is not an HTML render proof.
extern crate alloc;
pub mod html;
pub mod portable;
pub mod schema;
pub mod text;
