//! Explicit development-host phase limits and execution provenance.
pub mod job;
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

/// User-selected limits for a single-document operation. Repository CI does
/// not read this file; omission selects the ordinary finite host defaults.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionLimits {
    #[serde(default)]
    pub parse_limits: OperationLimits,
    #[serde(default)]
    pub lower_limits: OperationLimits,
    #[serde(default)]
    pub output_limits: OperationLimits,
}

impl ExecutionLimits {
    pub fn read(path: &std::path::Path) -> crate::Result<Self> {
        read_configuration(path)
    }
    pub fn phases(self) -> PhaseLimits {
        PhaseLimits {
            parse: self.parse_limits.budget().limits(),
            lower: self.lower_limits.budget().limits(),
        }
    }
}
pub(crate) fn read_configuration<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> crate::Result<T> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err("limit configuration exceeds 65536 bytes".into());
    }
    let text = std::str::from_utf8(&bytes)?;
    crate::repository::json::validate(text)?;
    Ok(serde_json::from_str(text)?)
}
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
pub(crate) fn limits(value: Limits) -> serde_json::Value {
    serde_json::json!({"source_bytes":value.source_bytes,"work":value.work,"depth":value.depth,
        "nodes":value.nodes,"allocation_units":value.allocation_units,"output_bytes":value.output_bytes,
        "diagnostics":value.diagnostics,"events":value.events})
}
pub(crate) fn usage(value: Usage) -> serde_json::Value {
    serde_json::json!({"source_bytes":value.source_bytes,"work":value.work,"depth":value.depth,
        "nodes":value.nodes,"allocation_units":value.allocation_units,"output_bytes":value.output_bytes,
        "diagnostics":value.diagnostics,"events":value.events})
}
/// Fixed-order big-endian u64 records, independent of JSON property ordering.
pub(super) fn execution_identity(
    content: Digest,
    limits: Limits,
    initial: Usage,
    renderer: crate::doc::export::MathRenderer,
) -> Digest {
    let mut bytes = Vec::from(b"nepl3.local-doc-pages.execution/2\0".as_slice());
    bytes.extend_from_slice(&content.0);
    bytes.push(match renderer {
        crate::doc::export::MathRenderer::KaTeXPreferred => 0,
        crate::doc::export::MathRenderer::MathMLOnly => 1,
    });
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
    fn configuration_file_is_bounded_and_strict() -> crate::Result<()> {
        let f = crate::testing::Fixture::new()?;
        let path = f.root().join("limits.json");
        f.write("limits.json", "{}")?;
        assert_eq!(
            ExecutionLimits::read(&path)?.parse_limits,
            OperationLimits::default()
        );
        let defaults = serde_json::to_value(ExecutionLimits::default())?;
        for field in ["parse_limits", "lower_limits", "output_limits"] {
            for invalid in [serde_json::json!(null), serde_json::json!({"work":1})] {
                let mut value = defaults.clone();
                value[field] = invalid;
                f.json("limits.json", &value)?;
                assert!(ExecutionLimits::read(&path).is_err());
            }
            for invalid in [
                serde_json::json!(-1),
                serde_json::json!(1.5),
                serde_json::json!("1"),
            ] {
                let mut value = defaults.clone();
                value[field]["work"] = invalid;
                f.json("limits.json", &value)?;
                assert!(ExecutionLimits::read(&path).is_err());
            }
        }
        for invalid in [r#"{"extra":1}"#, r#"{"parse_limits":{},"parse_limits":{}}"#] {
            f.write("limits.json", invalid)?;
            assert!(ExecutionLimits::read(&path).is_err());
        }
        f.write("limits.json", " ".repeat(65_537))?;
        assert!(ExecutionLimits::read(&path).is_err());
        Ok(())
    }
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
