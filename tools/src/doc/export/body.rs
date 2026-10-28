//! Explicit locally resolved Body HTML fragment. No Article, shell or resource
//! resolution capability is created. The fixed production CSS is not packaged.
pub mod page;
use crate::doc::{
    export::pages::resources::{ExecutionLimits, PhaseLimits},
    fragment,
    source::{Compiled, err, with_named_input_limits},
};
use nepl3_core::{
    budget::Budget,
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::check::Category;
use nepl3_doc_html::{ParallelMode, RenderOptions};
use nepl3_wire::foundation::FoundationCodec;
use std::{io::Write, path::Path};

/// Serialized checked block markup, without a page title, shell or newline.
#[derive(Debug)]
pub struct BodyHtmlArtifact {
    pub html: String,
    pub document_digest: Digest,
}

/// Explicit Body parse/lower followed by native local preparation and rendering.
/// Rows is fixed; every unresolved requirement, even in a hidden variant, fails.
pub fn from_source(
    compiled: &Compiled,
    source: &str,
    limits: ExecutionLimits,
) -> Result<BodyHtmlArtifact, String> {
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
) -> Result<BodyHtmlArtifact, String> {
    from_source_with(compiled, source, phases, output, |rendered, output| {
        let m = &rendered.markup;
        let checked =
            nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, output).map_err(err)?;
        let html = nepl3_markup::html::serialize(&checked, output).map_err(err)?;
        Ok(BodyHtmlArtifact {
            html,
            document_digest: rendered.document_digest,
        })
    })
}
fn from_source_with<T>(
    compiled: &Compiled,
    source: &str,
    phases: PhaseLimits,
    output: &mut Budget,
    finish: impl FnOnce(&nepl3_doc_html::RenderedFragment, &mut Budget) -> Result<T, String>,
) -> Result<T, String> {
    output.poll().map_err(err)?;
    if source.len() as u64 > super::MAX_SOURCE_BYTES.min(phases.parse.source_bytes) {
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
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let prepared = nepl3_doc_html::prepare_local_body(
                &document,
                &options,
                profile.registry(),
                &mut codec,
                output,
            )
            .map_err(err)?;
            let rendered = nepl3_doc_html::render_body(&prepared, output).map_err(err)?;
            finish(&rendered, output)
        },
    )
}

pub fn read_limits(path: &Path) -> crate::Result<ExecutionLimits> {
    fragment::read_limits(path)
}

/// Fully generate before publishing one new file. Shares bounded UTF-8 input,
/// staging, no-replace hard-link publication and cleanup semantics with Markdown.
/// No partial final file is exposed; power-loss/hostile-race durability is not promised.
pub fn write(input: &Path, output: &Path, limits: ExecutionLimits) -> crate::Result<()> {
    fragment::absent(output)?;
    let ceiling = super::MAX_SOURCE_BYTES.min(limits.parse_limits.source_bytes);
    let source = fragment::read_regular(input, ceiling)?;
    let artifact = from_source(&crate::doc::source::compiled()?, &source, limits)?;
    fragment::publish(output, |file| file.write_all(artifact.html.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialization_shares_exact_output_budget_and_cancellation() -> Result<(), String> {
        let c = crate::doc::source::compiled()?;
        let source = "body nil";
        let phases = PhaseLimits::default();
        let mut baseline = crate::doc::source::budget();
        let expected = from_source_profile(&c, source, phases, &mut baseline)?;
        assert_eq!(expected.html, "<div class=\"nepl-doc\"></div>");
        let mut limits = baseline.limits();
        limits.output_bytes = baseline.usage().output_bytes;
        assert_eq!(
            from_source_profile(&c, source, phases, &mut Budget::new(limits))?.html,
            expected.html
        );
        limits.output_bytes -= 1;
        assert!(
            from_source_profile(&c, source, phases, &mut Budget::new(limits))
                .err()
                .ok_or("expected failure")?
                .contains("OutputLimit")
        );
        let mut cancelled = crate::doc::source::budget();
        cancelled.cancel();
        assert!(
            from_source_profile(&c, source, phases, &mut cancelled)
                .err()
                .ok_or("expected failure")?
                .contains("Cancelled")
        );
        Ok(())
    }
}
