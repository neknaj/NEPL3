//! Pure operation dispatch. Hosts supply executable identity, grants and I/O.
#![no_std]
extern crate alloc;

pub mod dispatch;
pub mod grants;
pub mod scheduler;
pub mod suspension;
