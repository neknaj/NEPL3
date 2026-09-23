//! Pure adapters selected by the integrating host. Each feature declares the
//! language cores needed by that adapter; default dispatch stays domain-free.
#[cfg(any(feature = "doc-sentence", feature = "sentence-html"))]
pub mod sentence;
