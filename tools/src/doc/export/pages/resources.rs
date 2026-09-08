//! Explicit development-host phase limits and execution provenance.
use nepl3_core::{
    budget::{Budget, Limits, Usage},
    source::Digest,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationLimits {
    pub source_bytes: u64,
    pub work: u64,
    pub depth: u64,
    pub nodes: u64,
    pub allocation_units: u64,
    pub output_bytes: u64,
    pub diagnostics: u64,
    pub events: u64,
}
/// Kept for existing output-only callers.
pub type OutputLimits = OperationLimits;
impl Default for OperationLimits {
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
impl OperationLimits {
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

/// One parse and one lower operation per page, selected before either starts.
#[derive(Clone, Copy, Debug)]
pub struct PhaseLimits {
    pub parse: Limits,
    pub lower: Limits,
}
impl Default for PhaseLimits {
    fn default() -> Self {
        Self {
            parse: crate::doc::source::budget().limits(),
            lower: crate::doc::source::budget().limits(),
        }
    }
}

pub(super) fn phase_identity(output: Digest, profiles: &[Digest], phases: PhaseLimits) -> Digest {
    let mut bytes = Vec::from(b"nepl3.local-doc-pages.phases/1\0".as_slice());
    bytes.extend_from_slice(&output.0);
    bytes.extend_from_slice(&(profiles.len() as u64).to_be_bytes());
    for profile in profiles {
        bytes.extend_from_slice(&profile.0);
        for limits in [phases.parse, phases.lower] {
            for value in [
                limits.source_bytes,
                limits.work,
                limits.depth,
                limits.nodes,
                limits.allocation_units,
                limits.output_bytes,
                limits.diagnostics,
                limits.events,
            ] {
                bytes.extend_from_slice(&value.to_be_bytes());
            }
            // The configuration entry starts a fresh operation, with zero usage.
            bytes.extend_from_slice(&[0; 64]);
        }
    }
    Digest::of(&bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn phase_digest_matches_independent_fixed_big_endian_vector() {
        let phases = PhaseLimits {
            parse: Limits {
                source_bytes: 1,
                work: 2,
                depth: 3,
                nodes: 4,
                allocation_units: 5,
                output_bytes: 6,
                diagnostics: 7,
                events: 8,
            },
            lower: Limits {
                source_bytes: 9,
                work: 10,
                depth: 11,
                nodes: 12,
                allocation_units: 13,
                output_bytes: 14,
                diagnostics: 15,
                events: 16,
            },
        };
        let value = phase_identity(
            Digest([0x11; 32]),
            &[Digest([0x22; 32]), Digest([0x33; 32])],
            phases,
        );
        // Independently encoded as domain + output[32] + >Q page count +
        // two (profile[32], >8Q parse, zero[64], >8Q lower, zero[64]) records.
        assert_eq!(
            crate::doc::export::digest_hex(value),
            "c995d35c394382d67fafaea050394f600cfd1d9fd88cbb60117044f9a2aa5008"
        );
        assert_ne!(
            value,
            phase_identity(
                Digest([0x11; 32]),
                &[Digest([0x33; 32]), Digest([0x22; 32])],
                phases
            )
        );
    }
}
