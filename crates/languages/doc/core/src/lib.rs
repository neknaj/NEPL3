//! Doc semantics and source-aware sentence processing, without host I/O or a
//! dependency on the parser engine or on any embedded language's semantic core.
#![no_std]
extern crate alloc;
pub mod check;
mod copy;
pub mod labels;
pub mod lower;
pub mod model;
pub mod normalize;
pub mod pages;
pub mod portable;
pub mod prepare;
pub mod print;
pub mod schema;
pub mod sentence;
pub mod text;
