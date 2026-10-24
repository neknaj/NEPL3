//! One frozen canonical snapshot; finite compatibility preparation precedes
//! retained HTML preparation/render retries. Not durable or all-stage recovery.
use super::*;
use crate::doc::export::pages::{
    self,
    interactive::RetryLimits,
    resources::{self, ExecutionLimits},
};
use crate::doc::source::err;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct HtmlRetryLimits {
    #[serde(default)]
    compatibility: ExecutionLimits,
    #[serde(default)]
    html: RetryLimits,
}
impl HtmlRetryLimits {
    fn same_compatibility(self, other: Self) -> bool {
        self.compatibility.parse_limits == other.compatibility.parse_limits
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
    let selected: HtmlRetryLimits = resources::read_configuration(limits)?;
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
    let mut compatibility_budget = selected.compatibility.output_limits.budget();
    let prepared=projection::prepare_html_retry(root,manifest,raw,selected.compatibility.phases(),!registry.compatibility.is_empty(),&mut compatibility_budget)
        .map_err(|error|format!("CanonicalPreparationNonResumable: {error}; output usage={:?}; selected compatibility limits={:?}",compatibility_budget.usage(),selected.compatibility))?;
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
        "contract":"nepl3.canonical-html.retry/1","registry":manifest,"registry_sha256":registry_digest,
        "compatibility_stage":if had_compatibility{"completed-single-attempt"}else{"not-required"},
        "compatibility_limits":selected.compatibility,"compatibility_output_usage":resources::usage(compatibility_budget.usage()),
        "initial_html_limits":selected.html,
        "snapshot_limits":{"pages":128,"passive_files":128,"source_bytes":10_000_000,"alias_bytes":1_048_576,"passive_bytes":2_097_152,"combined_html_source_bytes":crate::doc::export::MAX_SOURCE_BYTES,"file_metadata_bytes":65_536},
        "scope":"Frozen registry, sources, aliases and passive files. HTML preparation/render retries only; compatibility Markdown is a finite single-attempt prerequisite. HTML job usage excludes compatibility preparation, bounded snapshot/receipt work and I/O. No cross-process recovery."
    });
    let mut identity = b"nepl3.canonical-html.retry/1\0".to_vec();
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
