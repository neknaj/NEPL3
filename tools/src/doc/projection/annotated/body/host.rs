//! Explicit Body-only file adapter. Output is a Markdown fragment, not a page.
use super::*;
use crate::doc::fragment::{absent, publish, read_regular};
use crate::doc::{
    export::pages::resources::{ExecutionLimits, PhaseLimits},
    source::{Compiled, err, with_named_input_limits},
};
use nepl3_core::source::{SourceAdmission, SourceStore};
use nepl3_doc_core::check::Category;
use nepl3_wire::foundation::FoundationCodec;
use std::{io::Write, path::Path};
/// Read the same bounded strict phase-limit configuration as fragment exports.
pub fn read_limits(path: &Path) -> crate::Result<ExecutionLimits> {
    crate::doc::fragment::read_limits(path)
}

/// Parse/lower/prepare/render with separate explicit phase budgets. No aliases,
/// metadata, source links or final newline are added to the native artifact.
pub fn from_source(
    compiled: &Compiled,
    source: &str,
    limits: ExecutionLimits,
) -> Result<BodyArtifact, String> {
    from_source_profile(
        compiled,
        source,
        limits.phases(),
        &mut limits.output_limits.budget(),
    )
}
fn from_source_profile(
    compiled: &Compiled,
    source: &str,
    phases: PhaseLimits,
    output: &mut Budget,
) -> Result<BodyArtifact, String> {
    output.poll().map_err(err)?;
    if source.len() as u64 > crate::doc::export::MAX_SOURCE_BYTES.min(phases.parse.source_bytes) {
        return Err("SourceLimit".into());
    }
    with_named_input_limits(
        true,
        compiled,
        source,
        "doc-input",
        "Body",
        phases.parse,
        |tree, profile, _, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let document = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Body,
                profile.registry(),
                &mut Budget::new(phases.lower),
                &mut codec,
            )
            .map_err(err)?;
            render_footnotes(&document, profile.registry(), &mut codec, output).map_err(err)
        },
    )
}

/// Generate fully before touching the output. A complete sibling staging file
/// is atomically hard-linked without replacing any destination. Filesystems
/// without hard-link support fail; there is no unsafe rename/copy fallback.
/// This guarantees no partial final file, not power-loss durability or hostile
/// filesystem race protection. Cleanup failure after publication reports that
/// the complete output was published, rather than pretending it is absent.
pub fn write(input: &Path, output: &Path, limits: ExecutionLimits) -> crate::Result<()> {
    absent(output)?;
    let ceiling = crate::doc::export::MAX_SOURCE_BYTES.min(limits.parse_limits.source_bytes);
    let source = read_regular(input, ceiling)?;
    let artifact = from_source(&crate::doc::source::compiled()?, &source, limits)?;
    publish(output, |file| file.write_all(artifact.markdown.as_bytes()))
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
