//! One frozen canonical snapshot feeds independent compatibility and HTML
//! retry ledgers. Completed stages are retained; not durable recovery.
use super::*;
use crate::doc::export::pages::{self, interactive::RetryLimits, resources};
use crate::doc::source::err;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HtmlRetryLimits {
    #[serde(default)]
    compatibility: projection::retry::preparation::RetryLimits,
    #[serde(default)]
    html: RetryLimits,
}
impl HtmlRetryLimits {
    fn same_compatibility(self, other: Self) -> bool {
        self.compatibility.job_limits == other.compatibility.job_limits
            && self.compatibility.parse_limits == other.compatibility.parse_limits
            && self.compatibility.lower_limits == other.compatibility.lower_limits
            && self.compatibility.output_limits == other.compatibility.output_limits
    }
}
pub(super) fn defaults() -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&HtmlRetryLimits::default())?
    );
    Ok(())
}
pub(super) fn write(root: &Path, manifest: &str, limits: &Path, output: &Path) -> Result<()> {
    write_with_io(
        root,
        manifest,
        limits,
        output,
        &mut std::io::stdin().lock(),
        &mut std::io::stderr().lock(),
    )
}
fn write_with_io(
    root: &Path,
    manifest: &str,
    limits: &Path,
    output: &Path,
    input: &mut impl BufRead,
    messages: &mut impl Write,
) -> Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let mut selected: HtmlRetryLimits = resources::read_configuration(limits)?;
    let initial_compatibility = selected.compatibility;
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let registry = parse_registry(&raw)?;
    if registry
        .pages
        .iter()
        .any(|p| projection::is_footnotes(&p.renderer))
    {
        return Err("footnote-only registry requires Markdown export".into());
    }
    let registry_digest = references::hash(&raw);
    let prepared = projection::prepare_html_retry(
        root,
        manifest,
        raw,
        selected.compatibility,
        !registry.compatibility.is_empty(),
        |previous, stage, reason, usage| {
            writeln!(
                messages,
                "Paused canonical compatibility {stage:?}: {reason:?}; cumulative compatibility usage={usage:?}"
            )?;
            let next = resume_grant_with_message(
                input,
                messages,
                |next: &HtmlRetryLimits| {
                    next.compatibility.covers(previous) && next.html.covers(selected.html)
                },
                "Rejected: compatibility and HTML limits must be nondecreasing.",
            )?;
            Ok(next.map(|next| {
                selected = next;
                next.compatibility
            }))
        },
    )?;
    let compiled = crate::doc::source::compiled()?;
    let mut generated = pages::interactive::run(
        &compiled,
        prepared.inputs,
        prepared.references,
        prepared.aliases,
        selected.html,
        pages::DisplayOptions::default(),
        |previous, stage, reason, usage| {
            writeln!(messages,"Paused canonical HTML {stage}: {reason:?}; cumulative HTML usage={usage:?}. Compatibility settings must remain unchanged.").map_err(err)?;
            let next = resume_grant_with_message(input, messages, |next: &HtmlRetryLimits| {
                next.same_compatibility(selected) && next.html.covers(previous)
            }, "Rejected: compatibility limits must remain unchanged and all HTML limits must be nondecreasing.")
            .map_err(err)?;
            Ok(next.map(|n| n.html))
        },
    )?;
    let had_compatibility = prepared.markdown.is_some();
    if let Some(markdown) = prepared.markdown {
        compatibility::append(&registry, markdown, &mut generated)?;
    }
    if generated.manifest.len() > 8 * 1024 * 1024 {
        return Err("CanonicalHtmlRetryReceiptLimit".into());
    }
    let mut receipt: serde_json::Value = serde_json::from_str(&generated.manifest)?;
    let snapshot = serde_json::json!({
        "contract":"nepl3.canonical-html.retry/2","registry":manifest,"registry_sha256":registry_digest,
        "compatibility_stage":if had_compatibility{"completed-retry-job"}else{"not-required"},
        "initial_compatibility_limits":initial_compatibility,"compatibility_limits":selected.compatibility,
        "initial_html_limits":selected.html,
        "snapshot_limits":{"pages":128,"passive_files":128,"source_bytes":10_000_000,"alias_bytes":1_048_576,"passive_bytes":2_097_152,"combined_html_source_bytes":crate::doc::export::MAX_SOURCE_BYTES,"file_metadata_bytes":65_536},
        "scope":"Frozen registry, sources, aliases and passive files. Compatibility Markdown and HTML have independent cumulative retry ledgers, not a single whole-export cap. Completed compatibility is retained across HTML pauses. Bounded snapshot/receipt work and I/O are excluded. No cross-process recovery."
    });
    let mut identity = b"nepl3.canonical-html.retry/2\0".to_vec();
    for value in [
        &snapshot,
        &receipt["job_execution"],
        &receipt["compatibility_markdown"],
    ] {
        identity.extend_from_slice(&serde_json::to_vec(value)?);
    }
    receipt["canonical_html_retry"] = snapshot;
    receipt["canonical_html_retry"]["identity"] = references::hash(&identity).into();
    generated.manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    if generated.manifest.len() > 8 * 1024 * 1024 {
        return Err("CanonicalHtmlRetryReceiptLimit".into());
    }
    pages::write_generated(generated, output)
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
