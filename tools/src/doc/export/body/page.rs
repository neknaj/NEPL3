//! Offline single-file Body document shell with explicit host-owned metadata.
//! This does not create an Article, a PageSet or a foreign/resource capability.
use super::*;
use nepl3_core::budget::{Resource, StopReason};
use nepl3_markup::text::{TextContext, escape};
use serde::{Deserialize, Serialize};

pub const MAX_METADATA_BYTES: u64 = 65_536;

/// Required explicit host metadata, not synthesized from Body headings.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub title: String,
    pub language: String,
}
#[derive(Debug)]
pub struct BodyPageArtifact {
    pub html: String,
    /// Identity of the Body, not an identity binding metadata or packaged bytes.
    pub document_digest: Digest,
}

pub fn read_metadata(path: &Path) -> crate::Result<Metadata> {
    let text = fragment::read_regular(path, MAX_METADATA_BYTES)?;
    crate::repository::json::validate(&text)?;
    Ok(serde_json::from_str(&text)?)
}

struct EscapedMetadata {
    title: String,
    language: String,
}
fn prepare_metadata(metadata: &Metadata, b: &mut Budget) -> Result<EscapedMetadata, String> {
    b.poll().map_err(err)?;
    let size = metadata
        .title
        .len()
        .checked_add(metadata.language.len())
        .ok_or("MetadataLimit")?;
    if size as u64 > MAX_METADATA_BYTES {
        return Err("MetadataLimit".into());
    }
    b.charge(Resource::Work, size as u64).map_err(err)?;
    if metadata.title.trim().is_empty() {
        return Err("EmptyPageTitle".into());
    }
    if !nepl3_core::lexical::language::well_formed(&metadata.language, b).map_err(err)? {
        return Err("InvalidPageLanguage".into());
    }
    Ok(EscapedMetadata {
        title: escape(&metadata.title, TextContext::Content, b).map_err(err)?,
        language: escape(&metadata.language, TextContext::Attribute, b).map_err(err)?,
    })
}

pub fn from_source(
    compiled: &Compiled,
    source: &str,
    metadata: &Metadata,
    limits: ExecutionLimits,
) -> Result<BodyPageArtifact, String> {
    from_source_profile(
        compiled,
        source,
        metadata,
        limits.phases(),
        &mut limits.output_limits.budget(),
    )
}
fn from_source_profile(
    compiled: &Compiled,
    source: &str,
    metadata: &Metadata,
    phases: PhaseLimits,
    b: &mut Budget,
) -> Result<BodyPageArtifact, String> {
    let metadata = prepare_metadata(metadata, b)?;
    super::from_source_with(compiled, source, phases, b, |rendered, b| {
        let m = &rendered.markup;
        let checked =
            nepl3_markup::html::validate(&m.fragment, m.slot, &m.policy, b).map_err(err)?;
        super::super::check_shell_depth(&m.fragment, b)?;
        let fragment = nepl3_markup::html::serialize(&checked, b).map_err(err)?;
        Ok(BodyPageArtifact {
            html: shell(&fragment, &metadata, b)?,
            document_digest: rendered.document_digest,
        })
    })
}

fn shell(fragment: &str, metadata: &EscapedMetadata, b: &mut Budget) -> Result<String, String> {
    // Account for the fixed head independently of Body preparation depth.
    b.observe_depth(4).map_err(err)?;
    let css = super::super::CSS;
    super::super::stylesheet::check_inline_css(css)?;
    b.charge(Resource::Work, css.len() as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, 44).map_err(err)?;
    let hash = super::super::stylesheet::csp_digest(Digest::of(css.as_bytes()));
    // No font link, scripts, resource requests or local/remote URL permissions.
    // Production CSS uses its installed-font fallbacks when Klee One is absent.
    let parts = [
        "<!DOCTYPE html><html lang=\"",
        metadata.language.as_str(),
        "\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'sha256-",
        hash.as_str(),
        "'; base-uri 'none'; form-action 'none'\"><title>",
        metadata.title.as_str(),
        "</title><style>",
        css,
        "</style></head><body>",
        fragment,
        "</body></html>",
    ];
    let size = parts
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.len()))
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
    // Escaping metadata and serializing fragment already charged their bytes.
    let added = size - metadata.title.len() - metadata.language.len() - fragment.len();
    // Doctype plus ten fixed shell element/text nodes; no whitespace nodes.
    b.charge(Resource::Nodes, 11).map_err(err)?;
    b.charge(Resource::OutputBytes, added as u64).map_err(err)?;
    b.charge(Resource::Work, size as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, size as u64)
        .map_err(err)?;
    let mut html = String::new();
    html.try_reserve_exact(size)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    for part in parts {
        html.push_str(part);
    }
    Ok(html)
}

/// Fully generate before the shared no-replace single-file publication step.
pub fn write(
    input: &Path,
    output: &Path,
    metadata: &Metadata,
    limits: ExecutionLimits,
) -> crate::Result<()> {
    fragment::absent(output)?;
    let ceiling = super::super::MAX_SOURCE_BYTES.min(limits.parse_limits.source_bytes);
    let source = fragment::read_regular(input, ceiling)?;
    let artifact = from_source(&crate::doc::source::compiled()?, &source, metadata, limits)?;
    fragment::publish(output, |file| file.write_all(artifact.html.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn metadata() -> Metadata {
        Metadata {
            title: "Body <& title".into(),
            language: "ja-JP".into(),
        }
    }
    #[test]
    fn page_budget_includes_shell_and_escaped_metadata_exactly() -> Result<(), String> {
        let c = crate::doc::source::compiled()?;
        let m = metadata();
        let mut b = crate::doc::source::budget();
        let page = from_source_profile(&c, "body nil", &m, PhaseLimits::default(), &mut b)?;
        assert!(
            page.html
                .starts_with("<!DOCTYPE html><html lang=\"ja-JP\">")
        );
        assert!(page.html.contains("<title>Body &lt;&amp; title</title>"));
        assert!(
            page.html
                .ends_with("<div class=\"nepl-doc\"></div></body></html>")
        );
        assert!(
            page.html
                .contains(&format!("<style>{}</style>", super::super::super::CSS))
        );
        for forbidden in [
            "<script",
            "<link",
            "<article",
            "https://fonts",
            "assets/doc.css",
        ] {
            assert!(!page.html.contains(forbidden), "{forbidden}");
        }
        // Preserve native preparation/serialization intermediate charges; only
        // the additional shell and escaped metadata are charged exactly once.
        let mut raw_budget = crate::doc::source::budget();
        let raw = super::super::from_source_profile(
            &c,
            "body nil",
            PhaseLimits::default(),
            &mut raw_budget,
        )?;
        assert_eq!(
            b.usage().output_bytes - raw_budget.usage().output_bytes,
            (page.html.len() - raw.html.len()) as u64
        );
        let mut limits = b.limits();
        limits.output_bytes = b.usage().output_bytes;
        assert_eq!(
            from_source_profile(
                &c,
                "body nil",
                &m,
                PhaseLimits::default(),
                &mut Budget::new(limits)
            )?
            .html,
            page.html
        );
        limits.output_bytes -= 1;
        assert!(
            from_source_profile(
                &c,
                "body nil",
                &m,
                PhaseLimits::default(),
                &mut Budget::new(limits)
            )
            .err()
            .ok_or("expected limit")?
            .contains("OutputLimit")
        );
        let mut cancelled = crate::doc::source::budget();
        cancelled.cancel();
        assert!(
            from_source_profile(&c, "body nil", &m, PhaseLimits::default(), &mut cancelled)
                .err()
                .ok_or("expected cancellation")?
                .contains("Cancelled")
        );
        Ok(())
    }
    #[test]
    fn fixed_head_depth_and_metadata_size_boundaries_are_enforced() -> Result<(), String> {
        // The full native preparation has additional depth costs. Isolate the
        // fixed shell boundary rather than asserting its depth is sufficient
        // for the entire Body pipeline.
        let m = prepare_metadata(&metadata(), &mut crate::doc::source::budget())?;
        let mut limits = crate::doc::source::budget().limits();
        limits.depth = 4;
        shell("<div></div>", &m, &mut Budget::new(limits))?;
        limits.depth = 3;
        assert!(
            shell("<div></div>", &m, &mut Budget::new(limits))
                .err()
                .ok_or("accepted shallow shell budget")?
                .contains("DepthLimit")
        );
        let exact = Metadata {
            title: "t".repeat(MAX_METADATA_BYTES as usize - 2),
            language: "en".into(),
        };
        prepare_metadata(&exact, &mut crate::doc::source::budget())?;
        let over = Metadata {
            title: format!("{}t", exact.title),
            language: exact.language,
        };
        assert!(
            prepare_metadata(&over, &mut crate::doc::source::budget())
                .err()
                .ok_or("accepted oversized metadata")?
                .contains("MetadataLimit")
        );
        Ok(())
    }
    #[test]
    fn metadata_rejects_invalid_values_without_normalization() -> Result<(), String> {
        for (title, language, expected) in [
            (" ", "en", "EmptyPageTitle"),
            ("title", "en_US", "InvalidPageLanguage"),
            ("bad\0", "en", "InvalidCharacter"),
        ] {
            let result = prepare_metadata(
                &Metadata {
                    title: title.into(),
                    language: language.into(),
                },
                &mut crate::doc::source::budget(),
            );
            assert!(result.err().ok_or("accepted invalid")?.contains(expected));
        }
        let mut huge = metadata();
        huge.title = "x".repeat(MAX_METADATA_BYTES as usize);
        assert!(
            prepare_metadata(&huge, &mut crate::doc::source::budget())
                .err()
                .ok_or("accepted oversized")?
                .contains("MetadataLimit")
        );
        Ok(())
    }
}
