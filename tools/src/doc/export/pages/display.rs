//! Native display options and bounded, domain-separated identity composition.
use super::*;
use nepl3_core::budget::{Resource, StopReason};
/// Explicit native display selection. CLI and retained jobs keep their Rows default.
#[derive(Clone, Debug)]
pub struct Options {
    pub document: RenderOptions,
    pub math_renderer: super::MathRenderer,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            document: RenderOptions {
                parallel: ParallelMode::Rows,
            },
            math_renderer: super::MathRenderer::KaTeXPreferred,
        }
    }
}

/// Exact requested options, not normalization by equivalent rendered output.
pub(super) fn bind_identity(
    domain: &[u8],
    base: Digest,
    mode: &ParallelMode,
    b: &mut Budget,
) -> Result<Digest, String> {
    let mut bytes = Vec::new();
    append(&mut bytes, domain, b)?;
    append(&mut bytes, &base.0, b)?;
    append(
        &mut bytes,
        &[match mode {
            ParallelMode::Rows => 0,
            ParallelMode::Columns => 1,
            ParallelMode::Single { .. } => 2,
        }],
        b,
    )?;
    if let ParallelMode::Single {
        language,
        fallbacks,
    } = mode
    {
        append(&mut bytes, &(language.len() as u64).to_le_bytes(), b)?;
        append(&mut bytes, language.as_bytes(), b)?;
        append(&mut bytes, &(fallbacks.len() as u64).to_le_bytes(), b)?;
        for value in fallbacks {
            append(&mut bytes, &(value.len() as u64).to_le_bytes(), b)?;
            append(&mut bytes, value.as_bytes(), b)?;
        }
    }
    b.charge(Resource::Work, bytes.len() as u64).map_err(err)?;
    Ok(Digest::of(&bytes))
}
fn append(out: &mut Vec<u8>, bytes: &[u8], b: &mut Budget) -> Result<(), String> {
    let size = u64::try_from(bytes.len()).map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    let allocation = size
        .checked_mul(2)
        .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
    b.charge(Resource::Work, size).map_err(err)?;
    b.charge(Resource::AllocationUnits, allocation)
        .map_err(err)?;
    out.try_reserve(bytes.len())
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    out.extend_from_slice(bytes);
    Ok(())
}
