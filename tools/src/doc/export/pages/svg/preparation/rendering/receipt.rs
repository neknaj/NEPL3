//! The outer native job receipt is host bookkeeping, not another render phase.
//! Its identity is separate from the package's immutable-input identity.
use super::*;
use crate::doc::{export::digest_hex, source::err};
use nepl3_core::source::Digest;
const DOMAIN: &str = "nepl3.local-doc-pages.svg-render-job/1";

pub(super) fn finish(
    package: &str,
    preparation: &serde_json::Value,
    attempts: &[RenderAttempt],
    limits: Limits,
    usage: Usage,
) -> Result<String, String> {
    let package: serde_json::Value = serde_json::from_str(package).map_err(err)?;
    let final_attempt = attempts
        .len()
        .checked_sub(1)
        .ok_or("MissingRenderAttempt")?;
    let job = serde_json::json!({
        "limits":resources::limits(limits),"usage":resources::usage(usage),
        "preparation":preparation,
        "render_attempts":attempts.iter().map(|a|serde_json::json!({
            "requested":resources::limits(a.requested),"effective":resources::limits(a.effective),
            "usage":resources::usage(a.usage),"stop":a.stop.map(|s|format!("{s:?}")),"failed":a.failed,
        })).collect::<Vec<_>>(),
        "successful_render_attempt":final_attempt,
        "budget_scope":"Cumulative measured parse/lower/Code and atomic render attempts, including original package receipt serialization; capture I/O, host containers, this outer receipt encoding/identity and KaTeX internal work/allocation are unmeasured.",
        "native_evidence_scope":"Package native evidence belongs only to the final successful attempt. Native outcomes of earlier interrupted attempts are not retained; no invocation history is inferred.",
        "host_configuration":"Trusted host configuration is not captured document input; if generation is reached it observes current availability/configuration and validates actual responses. No failed-attempt visual/font cache is reused.",
    });
    // The native host contract uses compact serde_json encoding of the ordered
    // two-element array, before adding the outer identity. This is not a portable
    // source/renderer proof or a claim of measured host bookkeeping.
    let payload = serde_json::to_vec(&(&package, &job)).map_err(err)?;
    let mut bytes = Vec::from(DOMAIN.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&payload);
    let identity = digest_hex(Digest::of(&bytes));
    serde_json::to_string_pretty(&serde_json::json!({
        "format":DOMAIN,"identity":identity,"package":package,"job_execution":job,
    }))
    .map(|text| text + "\n")
    .map_err(err)
}
