//! Platform independent contracts shared by NEPL3 language implementations.
#![no_std]
extern crate alloc;

pub mod budget;
pub mod diagnostic;
pub mod origin;
pub mod schema;
pub mod source;
pub mod syntax;
pub mod value;
pub mod value_codec;
pub mod view;
