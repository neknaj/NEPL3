//! Immutable-source SVG PageSet export with explicit renderer provenance.
pub mod interactive;
use super::{generation as generate, input};
use crate::doc::export::MathRenderer;
use nepl3_doc_html::assets::SvgMode;
use std::path::Path;

/// Captures inputs once, completes all generation, then writes the receipt last.
/// This legacy embedded entry never prompts or implicitly downgrades renderers.
pub fn write(manifest: &Path, output: &Path, renderer: MathRenderer) -> crate::Result<()> {
    write_mode(manifest, output, renderer, SvgMode::Embedded)
}

/// Explicit external SVG source export. Inputs and finite capture limits are
/// unchanged; this is not an interactive or durably restorable job.
pub fn write_external(manifest: &Path, output: &Path, renderer: MathRenderer) -> crate::Result<()> {
    write_mode(manifest, output, renderer, SvgMode::External)
}
fn write_mode(
    manifest: &Path,
    output: &Path,
    renderer: MathRenderer,
    mode: SvgMode,
) -> crate::Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let captured = input::capture(manifest)?;
    let mut budget = captured.output.budget();
    let generated = {
        let compiled = crate::doc::source::compiled()?;
        match mode {
            SvgMode::Embedded => {
                generate::run_selected(&compiled, &captured, renderer, &mut budget)?
            }
            SvgMode::External => {
                generate::run_external_selected(&compiled, &captured, renderer, &mut budget)?
            }
        }
    };
    let _math_generation = generated.math_generation;
    let _native_generation = generated.native_generation;
    super::super::write_checked_files(generated.files, generated.manifest, output)
}

#[cfg(test)]
mod tests;
