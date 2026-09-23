//! Pure adapters selected by the integrating host. Each feature declares the
//! language cores needed by that adapter; default dispatch stays domain-free.
#[cfg(feature = "doc-sentence")]
pub mod sentence;
