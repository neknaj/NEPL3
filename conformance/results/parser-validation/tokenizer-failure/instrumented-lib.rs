//! Portable reader plans and transactional execution for NEPL3.
#![no_std]
extern crate alloc;

pub mod builtin;
pub mod context;
pub mod model;
pub mod plan;
pub mod portable;
mod primitive;
pub mod runtime;
pub mod schema;
pub mod tokenizer;

#[cfg(test)] extern crate std;
#[cfg(test)] extern crate self as nepl3_reader;
