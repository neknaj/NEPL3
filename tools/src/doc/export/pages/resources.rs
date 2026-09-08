//! Explicit development-host output limits; no change to parser Profiles.
use nepl3_core::{
    budget::{Budget, Limits, Usage},
    source::Digest,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OutputLimits {
    pub source_bytes: u64,
    pub work: u64,
    pub depth: u64,
    pub nodes: u64,
    pub allocation_units: u64,
    pub output_bytes: u64,
    pub diagnostics: u64,
    pub events: u64,
}
impl Default for OutputLimits {
    fn default() -> Self {
        let value = crate::doc::source::budget().limits();
        Self {
            source_bytes: value.source_bytes,
            work: value.work,
            depth: value.depth,
            nodes: value.nodes,
            allocation_units: value.allocation_units,
            output_bytes: value.output_bytes,
            diagnostics: value.diagnostics,
            events: value.events,
        }
    }
}
impl OutputLimits {
    pub fn budget(self) -> Budget {
        Budget::new(Limits {
            source_bytes: self.source_bytes,
            work: self.work,
            depth: self.depth,
            nodes: self.nodes,
            allocation_units: self.allocation_units,
            output_bytes: self.output_bytes,
            diagnostics: self.diagnostics,
            events: self.events,
        })
    }
}
pub(super) fn limits(value: Limits) -> serde_json::Value {
    serde_json::json!({"source_bytes":value.source_bytes,"work":value.work,"depth":value.depth,
        "nodes":value.nodes,"allocation_units":value.allocation_units,"output_bytes":value.output_bytes,
        "diagnostics":value.diagnostics,"events":value.events})
}
pub(super) fn usage(value: Usage) -> serde_json::Value {
    serde_json::json!({"source_bytes":value.source_bytes,"work":value.work,"depth":value.depth,
        "nodes":value.nodes,"allocation_units":value.allocation_units,"output_bytes":value.output_bytes,
        "diagnostics":value.diagnostics,"events":value.events})
}
/// Fixed-order big-endian u64 records, independent of JSON property ordering.
pub(super) fn execution_identity(content: Digest, limits: Limits, initial: Usage) -> Digest {
    let mut bytes = Vec::from(b"nepl3.local-doc-pages.execution/1\0".as_slice());
    bytes.extend_from_slice(&content.0);
    for value in [
        limits.source_bytes,
        limits.work,
        limits.depth,
        limits.nodes,
        limits.allocation_units,
        limits.output_bytes,
        limits.diagnostics,
        limits.events,
        initial.source_bytes,
        initial.work,
        initial.depth,
        initial.nodes,
        initial.allocation_units,
        initial.output_bytes,
        initial.diagnostics,
        initial.events,
    ] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    Digest::of(&bytes)
}
