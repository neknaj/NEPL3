//! Immutable-source SVG PageSet export with explicit renderer provenance.
mod generate;
mod input;
use crate::doc::export::MathRenderer;
use std::path::Path;

/// Captures inputs once, completes all generation, then writes the receipt last.
/// No interactive retry, external SVG or implicit renderer downgrade is supported.
pub fn write(manifest: &Path, output: &Path, renderer: MathRenderer) -> crate::Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let captured = input::capture(manifest)?;
    let mut budget = captured.output.budget();
    let generated = generate::run_selected(
        &crate::doc::source::compiled()?,
        &captured,
        renderer,
        &mut budget,
    )?;
    let _math_generation = generated.math_generation;
    let _native_generation = generated.native_generation;
    super::super::write_checked_files(generated.files, generated.manifest, output)
}

#[cfg(test)]
mod tests;
