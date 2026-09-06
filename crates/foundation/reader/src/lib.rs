//! Portable reader plans and transactional execution for NEPL3.
#![no_std]
extern crate alloc;

pub mod context;
pub mod model;
pub mod plan;
pub mod portable;
mod primitive;
pub mod runtime;
pub mod schema;
