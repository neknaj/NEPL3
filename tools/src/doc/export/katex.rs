//! Native KaTeX generation capability; configured independently of Doc source.
mod transport;
use super::{err, math::Occurrence};
use crate::doc::math::display::TexPreparation;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::Digest,
};
use nepl3_markup::{katex::fragment, mathml::Display};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
const EXECUTION: &str = include_str!("../../../math/katex/execution.json");
const ASSETS: &str = include_str!("../../../math/katex/assets.json");
pub struct Visual {
    pub root: u64,
    pub scope: String,
    pub fragment: fragment::Fragment,
}
pub struct File {
    pub path: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}
pub struct Generated {
    pub visuals: Vec<Visual>,
    pub files: Arc<Vec<File>>,
    pub classes: Arc<Vec<String>>,
    pub node_version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    source: String,
    path: String,
    mime: String,
    bytes: usize,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetPins {
    version: String,
    #[serde(rename = "packageIntegrity")]
    integrity: String,
    files: Vec<Pin>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostFile {
    path: String,
    mime: String,
    sha256: String,
    hex: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostRecord {
    id: usize,
    result: Value,
    #[serde(default)]
    diagnostics: Vec<Diagnostic>,
    #[serde(default, rename = "diagnosticBytes")]
    diagnostic_bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    level: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum HostResult {
    Generated {
        results: Vec<HostRecord>,
        files: Vec<HostFile>,
        execution_sha256: String,
        node_version: String,
    },
    Unavailable {
        #[serde(default)]
        path: Option<String>,
    },
    Stopped {
        reason: String,
    },
    ProviderViolation {
        reason: String,
        #[serde(default)]
        path: Option<String>,
    },
    InvalidRequest,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn digest(bytes: &[u8]) -> String {
    hex(&Digest::of(bytes).0)
}
fn decode_hex(text: &str, b: &mut Budget) -> Result<Vec<u8>, String> {
    b.charge(Resource::Work, text.len() as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, (text.len() / 2) as u64)
        .map_err(err)?;
    if !text.len().is_multiple_of(2) {
        return Err("KaTeXAssetHex".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(text.len() / 2)
        .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
    for pair in text.as_bytes().chunks_exact(2) {
        let digit = |x: u8| match x {
            b'0'..=b'9' => Some(x - b'0'),
            b'a'..=b'f' => Some(x - b'a' + 10),
            _ => None,
        };
        bytes.push(
            digit(pair[0]).ok_or("KaTeXAssetHex")? * 16 + digit(pair[1]).ok_or("KaTeXAssetHex")?,
        );
    }
    Ok(bytes)
}
/// A synchronous immutable-source batch. Identity includes source/profile/options
/// supplied by the export owner and the exact generated TeX requests. Worker
/// resource stops and violations are fatal; optional absence preserves MathML.
pub fn generate(
    records: &mut [Occurrence],
    source_identity: &str,
    b: &mut Budget,
) -> Result<Option<Generated>, String> {
    generate_cached(records, source_identity, None, b)
}
pub fn generate_cached(
    records: &mut [Occurrence],
    source_identity: &str,
    cached: Option<&Generated>,
    b: &mut Budget,
) -> Result<Option<Generated>, String> {
    b.poll().map_err(err)?;
    if std::env::var_os("NEPL3_KATEX_NODE_MODULES").is_none() {
        return Ok(None);
    }
    let mut requests = Vec::new();
    let mut selected = Vec::new();
    for (index, record) in records.iter().enumerate() {
        b.charge(Resource::Work, 1).map_err(err)?;
        if let TexPreparation::Ready { tex, .. } = &record.tex {
            if requests.len() == 256 {
                return Err(err(b.stop(StopReason::NodeLimit)));
            }
            b.charge(Resource::AllocationUnits, tex.len() as u64 * 32 + 1024)
                .map_err(err)?;
            requests.push(serde_json::json!({"id":requests.len(),"tex":tex,"displayMode":record.display == Display::Block}));
            selected.push(index);
        }
    }
    if requests.is_empty() {
        return Ok(None);
    }
    let source = serde_json::to_vec(&(
        source_identity,
        &requests,
        transport::identity(),
        digest(EXECUTION.as_bytes()),
        digest(ASSETS.as_bytes()),
        converter_identity(),
    ))
    .map_err(err)?;
    b.charge(Resource::Work, source.len() as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, source.len() as u64)
        .map_err(err)?;
    let identity = digest(&source);
    let request = serde_json::to_vec(&serde_json::json!({"identity":identity,"requests":requests,"include_assets":cached.is_none()}))
        .map_err(err)?;
    let Some(response) = transport::invoke(&request, b)? else {
        return Ok(None);
    };
    let host = decode_response(&response, &identity, b)?;
    let (results, files, execution, node_version) = match host {
        HostResult::Generated {
            results,
            files,
            execution_sha256,
            node_version,
        } => (results, files, execution_sha256, node_version),
        HostResult::Unavailable { path } => {
            if let Some(path) = path {
                if path.len() > 4096 {
                    return Err("KaTeXUnavailableShape".into());
                }
                for index in &selected {
                    records[*index].fallback = Some(super::math::Fallback::ResourceUnavailable);
                    b.charge(Resource::AllocationUnits, path.len() as u64)
                        .map_err(err)?;
                    records[*index].fallback_detail = Some(path.clone());
                }
            }
            return Ok(None);
        }
        HostResult::Stopped { reason } => {
            b.cancel();
            return Err(format!("KaTeXHostStopped({reason}); parent cancelled"));
        }
        HostResult::ProviderViolation { reason, path } => {
            return Err(format!("KaTeXHostViolation({reason}, {path:?})"));
        }
        HostResult::InvalidRequest => return Err("KaTeXHostInvalidRequest".into()),
    };
    if node_version.len() > 32
        || !node_version
            .bytes()
            .all(|c| c.is_ascii_digit() || c == b'.')
    {
        return Err("KaTeXHostNodeVersion".into());
    }
    if execution != digest(EXECUTION.as_bytes()) || results.len() != selected.len() {
        return Err("KaTeXHostIdentity".into());
    }
    let no_visuals = results.iter().all(|r| {
        matches!(
            r.result.get("kind").and_then(Value::as_str),
            Some("render-error" | "unavailable")
        )
    });
    let (owned, classes) = if no_visuals && files.is_empty() {
        (Arc::new(Vec::new()), Arc::new(Vec::new()))
    } else if let Some(cache) = cached {
        if !files.is_empty() {
            return Err("KaTeXUnexpectedAssets".into());
        }
        (Arc::clone(&cache.files), Arc::clone(&cache.classes))
    } else {
        b.charge(
            Resource::AllocationUnits,
            (ASSETS.len() + EXECUTION.len()) as u64 * 64,
        )
        .map_err(err)?;
        b.charge(Resource::Work, (ASSETS.len() + EXECUTION.len()) as u64 * 16)
            .map_err(err)?;
        let pins: AssetPins = serde_json::from_str(ASSETS).map_err(err)?;
        if pins.version != "0.18.7" || pins.integrity.is_empty() || files.len() != pins.files.len()
        {
            return Err("KaTeXAssets".into());
        }
        let mut owned = Vec::new();
        for (file, pin) in files.into_iter().zip(pins.files) {
            if pin.source.is_empty()
                || file.path != pin.path
                || file.mime != pin.mime
                || file.sha256 != pin.sha256
                || file.hex.len() != 2 * pin.bytes
            {
                return Err("KaTeXAssetIdentity".into());
            }
            let bytes = decode_hex(&file.hex, b)?;
            b.charge(Resource::Work, bytes.len() as u64).map_err(err)?;
            if digest(&bytes) != pin.sha256 {
                return Err("KaTeXAssetIdentity".into());
            }
            owned.push(File {
                path: file.path,
                mime: file.mime,
                bytes,
            });
        }
        let execution: Value = serde_json::from_str(EXECUTION).map_err(err)?;
        let classes: Vec<String> =
            serde_json::from_value(execution.get("classes").ok_or("KaTeXClasses")?.clone())
                .map_err(err)?;
        (Arc::new(owned), Arc::new(classes))
    };
    let class_refs: Vec<_> = classes.iter().map(String::as_str).collect();
    let mut visuals = Vec::new();
    for (id, (host, index)) in results.into_iter().zip(selected).enumerate() {
        if host.id != id || host.diagnostic_bytes > 8192 {
            return Err("KaTeXHostIdentity".into());
        }
        let mut diagnostic_bytes = 0u64;
        for diagnostic in &host.diagnostics {
            if !["log", "info", "warn", "error", "debug"].contains(&diagnostic.level.as_str()) {
                return Err("KaTeXDiagnostic".into());
            }
            diagnostic_bytes += diagnostic.text.len() as u64 + 1;
            b.charge(Resource::Diagnostics, 1).map_err(err)?;
        }
        if diagnostic_bytes != host.diagnostic_bytes {
            return Err("KaTeXDiagnostic".into());
        }
        // A warning may signal unsupported glyphs or clipped metrics; do not
        // admit it as faithful output merely because rendering returned HTML.
        if !host.diagnostics.is_empty() {
            return Err("KaTeXRendererDiagnostics".into());
        }
        let Value::Object(mut result) = host.result else {
            return Err("KaTeXHostShape".into());
        };
        match result.get("kind").and_then(Value::as_str) {
            Some("unavailable") if result.len() == 1 => {
                records[index].fallback_detail =
                    Some("pinned renderer/parser module unavailable".into());
                continue;
            }
            Some("render-error")
                if result.len() == 2
                    && result.get("reason").and_then(Value::as_str) == Some("parse") =>
            {
                records[index].fallback = Some(super::math::Fallback::RendererParseError);
                continue;
            }
            Some("parsed-unchecked") => {}
            _ => return Err("KaTeXHostOutcome".into()),
        }
        if result
            .remove("version")
            .and_then(|v| v.as_str().map(str::to_owned))
            .as_deref()
            != Some("0.18.7")
        {
            return Err("KaTeXHostVersion".into());
        }
        let input_bytes = result
            .remove("inputBytes")
            .and_then(|v| v.as_u64())
            .ok_or("KaTeXHostShape")?;
        let output_bytes = result
            .remove("outputBytes")
            .and_then(|v| v.as_u64())
            .ok_or("KaTeXHostShape")?;
        let TexPreparation::Ready { tex, .. } = &records[index].tex else {
            return Err("KaTeXHostIdentity".into());
        };
        if input_bytes != tex.len() as u64 || output_bytes > 262144 {
            return Err("KaTeXHostIdentity".into());
        }
        let fragment = crate::doc::math::katex::decode(Value::Object(result), b).map_err(err)?;
        let scope = format!("nepl-math-{identity}-k{id}");
        fragment::validate(
            &fragment,
            &fragment::Policy {
                classes: &class_refs,
                scope: &scope,
            },
            b,
        )
        .map_err(err)?;
        records[index].fallback = None;
        visuals.push(Visual {
            root: records[index].root,
            scope,
            fragment,
        });
    }
    if visuals.is_empty() {
        return Ok(None);
    }
    Ok(Some(Generated {
        visuals,
        files: owned,
        classes,
        node_version,
    }))
}

impl Generated {
    /// Preserve all fixed fonts and their media choices, replacing only each
    /// reviewed relative font URL with the same verified bytes as a data URL.
    /// The complete original license is retained in the generated stylesheet.
    pub fn stylesheet(&self, b: &mut Budget) -> Result<String, String> {
        let original = std::str::from_utf8(
            &self
                .files
                .iter()
                .find(|f| f.path == "katex.min.css")
                .ok_or("KaTeXCss")?
                .bytes,
        )
        .map_err(err)?;
        b.charge(Resource::Work, original.len() as u64)
            .map_err(err)?;
        b.charge(Resource::AllocationUnits, original.len() as u64)
            .map_err(err)?;
        let mut css = String::new();
        let mut remaining = original;
        let mut used = std::collections::BTreeSet::new();
        while let Some(start) = remaining.find("url(") {
            css.push_str(&remaining[..start + 4]);
            remaining = &remaining[start + 4..];
            let end = remaining.find(')').ok_or("KaTeXFontClosure")?;
            let path = &remaining[..end];
            b.charge(Resource::Work, self.files.len() as u64)
                .map_err(err)?;
            let file = self
                .files
                .iter()
                .find(|f| f.path == path && path.starts_with("fonts/"))
                .ok_or("KaTeXFontClosure")?;
            b.charge(Resource::AllocationUnits, path.len() as u64 + 64)
                .map_err(err)?;
            used.insert(path);
            let data = super::stylesheet::font_data(&file.bytes, &file.mime, b)?;
            b.charge(Resource::AllocationUnits, data.len() as u64)
                .map_err(err)?;
            css.push_str(&data);
            css.push(')');
            remaining = &remaining[end + 1..];
        }
        css.push_str(remaining);
        if self
            .files
            .iter()
            .filter(|f| f.path.starts_with("fonts/"))
            .any(|f| !used.contains(f.path.as_str()))
        {
            return Err("KaTeXFontClosure".into());
        }
        let license = std::str::from_utf8(
            &self
                .files
                .iter()
                .find(|f| f.path == "LICENSE")
                .ok_or("KaTeXLicense")?
                .bytes,
        )
        .map_err(err)?;
        if license.contains("*/") {
            return Err("KaTeXLicenseComment".into());
        }
        b.charge(Resource::AllocationUnits, license.len() as u64 + 32)
            .map_err(err)?;
        css.push_str("\n/* KaTeX license:\n");
        css.push_str(license);
        css.push_str("\n*/\n");
        Ok(css)
    }
    pub fn compose(
        &self,
        checked: &nepl3_markup::html::ValidatedHtml<'_>,
        b: &mut Budget,
    ) -> Result<nepl3_markup::html::MathHtml, String> {
        let classes: Vec<_> = self.classes.iter().map(String::as_str).collect();
        let mut proofs = Vec::new();
        for visual in &self.visuals {
            proofs.push(
                fragment::validate(
                    &visual.fragment,
                    &fragment::Policy {
                        classes: &classes,
                        scope: &visual.scope,
                    },
                    b,
                )
                .map_err(err)?,
            );
        }
        let bindings: Vec<_> = self
            .visuals
            .iter()
            .zip(&proofs)
            .map(|(v, p)| nepl3_markup::html::MathBinding {
                root: v.root,
                visual: p,
            })
            .collect();
        let (result, depth) = b
            .measure_depth(|b| nepl3_markup::html::serialize_with_math(checked, &bindings, 3, b))
            .map_err(err)?;
        if depth > 256 {
            return Err("KaTeXComposedShellDepth".into());
        }
        Ok(result)
    }
}

pub fn converter_identity() -> String {
    digest(include_bytes!(
        "../../../../crates/languages/math/tex/src/lib.rs"
    ))
}
impl Generated {
    pub fn identity(&self) -> Value {
        serde_json::json!({"adapter_sha256":transport::identity(),"execution_sha256":digest(EXECUTION.as_bytes()),"assets_sha256":digest(ASSETS.as_bytes()),"tex_converter_sha256":converter_identity(),"node_version":self.node_version,"unicode_fonts":"system fallback for supported scripts"})
    }
}

fn decode_response(response: &[u8], identity: &str, b: &mut Budget) -> Result<HostResult, String> {
    b.poll().map_err(err)?;
    // Conservative logical reservation for bounded serde trees/maps and
    // decoding temporaries; not a measurement of allocator RSS.
    b.charge(Resource::AllocationUnits, response.len() as u64 * 64)
        .map_err(err)?;
    b.charge(Resource::Work, response.len() as u64 * 16)
        .map_err(err)?;
    let Value::Object(mut value) = serde_json::from_slice::<Value>(response).map_err(err)? else {
        return Err("KaTeXHostShape".into());
    };
    if value.remove("identity") != Some(Value::String(identity.to_owned())) {
        return Err("KaTeXHostIdentity".into());
    }
    serde_json::from_value(Value::Object(value)).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::source::budget;
    #[test]
    fn composition_preserves_prior_depth_and_exact_output_limit() -> Result<(), String> {
        use nepl3_markup::{html::*, mathml};
        let tree = HtmlFragment {
            root: 1,
            nodes: vec![
                HtmlNode::Text { text: "x".into() },
                HtmlNode::MathElement {
                    tag: mathml::Tag::Math,
                    attributes: vec![mathml::Attribute::Display(Display::Inline)],
                    children: vec![2],
                },
                HtmlNode::MathElement {
                    tag: mathml::Tag::Identifier,
                    attributes: vec![],
                    children: vec![0],
                },
            ],
        };
        let policy = HtmlPolicy { classes: vec![] };
        let proof = validate(&tree, HtmlSlot::Phrasing, &policy, &mut budget()).map_err(err)?;
        let generated = Generated {
            files: Arc::new(vec![]),
            classes: Arc::new(vec!["katex".into()]),
            node_version: "24.0.0".into(),
            visuals: vec![Visual {
                root: 1,
                scope: "nepl-math-boundary".into(),
                fragment: fragment::Fragment {
                    nodes: vec![fragment::Node::Span {
                        classes: "katex".into(),
                        style: "height:1em;".into(),
                        aria_hidden: None,
                        children: vec![],
                    }],
                },
            }],
        };
        let mut first = budget();
        first.observe_depth(300).map_err(err)?;
        let output = generated.compose(&proof, &mut first)?;
        assert_eq!(first.usage().depth, 300);
        let size = (output.html.len() + output.stylesheet.len()) as u64;
        assert_eq!(first.usage().output_bytes, size);
        for (limit, succeeds) in [(size, true), (size - 1, false)] {
            let mut limits = budget().limits();
            limits.output_bytes = limit;
            let mut b = Budget::new(limits);
            let result = generated.compose(&proof, &mut b);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert_eq!(b.poll(), Err(StopReason::OutputLimit));
            }
        }
        Ok(())
    }
    #[test]
    fn response_identity_shape_and_sticky_budget() -> Result<(), String> {
        let response = br#"{"identity":"one","kind":"unavailable"}"#;
        assert!(matches!(
            decode_response(response, "one", &mut budget())?,
            HostResult::Unavailable { path: None }
        ));
        assert!(decode_response(response, "two", &mut budget()).is_err());
        for invalid in [
            b"null".as_slice(),
            b"{",
            br#"{"identity":"one","kind":"unavailable","extra":0}"#,
        ] {
            assert!(decode_response(invalid, "one", &mut budget()).is_err());
        }
        let mut stopped = budget();
        stopped.cancel();
        assert!(decode_response(response, "one", &mut stopped).is_err());
        assert_eq!(stopped.usage(), Default::default());
        let mut limits = budget().limits();
        limits.allocation_units = 1;
        let mut small = Budget::new(limits);
        assert!(decode_response(response, "one", &mut small).is_err());
        assert_eq!(small.poll(), Err(StopReason::AllocationLimit));
        Ok(())
    }
}
