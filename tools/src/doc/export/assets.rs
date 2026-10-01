//! Single-document SVG export prototype. Asset IDs never imply filesystem access.
use super::*;
use nepl3_doc_html::assets::{SvgInput, SvgMode};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetManifest {
    version: u64,
    assets: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source: String,
    mime: String,
}
fn read(path: &Path, limit: u64) -> crate::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("asset input size limit".into());
    }
    Ok(bytes)
}
pub fn write(
    input: &Path,
    manifest: &Path,
    output: &Path,
    css: CssMode,
    mode: &str,
) -> crate::Result<()> {
    let mode = match mode {
        "external" => SvgMode::External,
        "embedded" => SvgMode::Embedded,
        _ => return Err("expected SVG mode external or embedded".into()),
    };
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let manifest_bytes = read(manifest, 65_536)?;
    let spec: AssetManifest = serde_json::from_slice(&manifest_bytes)?;
    if spec.version != 1 || spec.assets.len() > 32 {
        return Err("invalid SVG manifest version or asset count".into());
    }
    let root = manifest
        .parent()
        .ok_or("missing manifest directory")?
        .canonicalize()?;
    let mut data = Vec::new();
    let mut total = 0usize;
    for entry in &spec.assets {
        if entry.id.is_empty()
            || entry.id.len() > 1024
            || entry.mime != "image/svg+xml"
            || entry.source.len() > 4096
            || entry
                .source
                .chars()
                .any(|c| c.is_control() || "\\:?#%".contains(c))
            || entry
                .source
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err("invalid SVG asset registration".into());
        }
        let path = root.join(&entry.source).canonicalize()?;
        if !path.starts_with(&root) {
            return Err("SVG asset escapes manifest directory".into());
        }
        let bytes = read(&path, nepl3_markup::html::svg::MAX_BYTES as u64)?;
        total += bytes.len();
        if total > 4 * nepl3_markup::html::svg::MAX_BYTES {
            return Err("total SVG bytes limit".into());
        }
        data.push(String::from_utf8(bytes)?);
    }
    let inputs: Vec<_> = spec
        .assets
        .iter()
        .zip(&data)
        .map(|(e, svg)| SvgInput { id: &e.id, svg })
        .collect();
    let source = read_source(input)?;
    let generated = generate_impl(
        &compiled()?,
        &source,
        css,
        Some((&inputs, mode)),
        &mut |_| {},
    )?;
    let mut files = BTreeMap::new();
    let mut resources = Vec::new();
    for (entry, svg) in spec.assets.iter().zip(&data) {
        let hash = Digest::of(svg.as_bytes())
            .0
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = format!("assets/{hash}.svg");
        resources.push(serde_json::json!({"id":entry.id,"mime":"image/svg+xml","sha256":hash,"bytes":svg.len(),"mode":mode_name(mode)}));
        if mode == SvgMode::External {
            files.entry(path).or_insert_with(|| svg.as_bytes().to_vec());
        }
    }
    let mut report: serde_json::Value = serde_json::from_str(&generated.manifest)?;
    report["scope"] = serde_json::json!(
        "single-document static SVG prototype; svg/g/path profile; pages and foreign requirements unsupported"
    );
    report["options"]["svg"] = serde_json::json!(mode_name(mode));
    report["resources"] = serde_json::json!(resources);
    if let Some(list) = report["files"].as_array_mut() {
        for (path, bytes) in &files {
            list.push(serde_json::json!({"path":path,"mime":"image/svg+xml","sha256":Digest::of(bytes).0.iter().map(|b| format!("{b:02x}")).collect::<String>()}));
        }
    } else {
        return Err("invalid generated manifest".into());
    }
    fs::create_dir(output)?;
    if css == CssMode::External || !files.is_empty() {
        fs::create_dir(output.join("assets"))?;
    }
    for (path, bytes) in files {
        fs::write(output.join(path), bytes)?;
    }
    if css == CssMode::External {
        fs::write(output.join("assets/doc.css"), &generated.stylesheet)?;
    }
    fs::write(output.join("document.html"), generated.html)?;
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
fn mode_name(mode: SvgMode) -> &'static str {
    match mode {
        SvgMode::External => "external",
        SvgMode::Embedded => "embedded",
    }
}

/// Emit only backend-owned numeric container rules, never source-authored CSS.
pub(super) fn document_css(
    document: &nepl3_doc_core::model::DocumentSyntax,
    inputs: Option<&[SvgInput<'_>]>,
    budget: &mut nepl3_core::budget::Budget,
) -> Result<String, String> {
    use nepl3_core::budget::Resource;
    budget
        .charge(Resource::AllocationUnits, CSS.len() as u64)
        .map_err(err)?;
    budget
        .charge(Resource::Work, CSS.len() as u64)
        .map_err(err)?;
    let mut css = String::from(CSS);
    let Some(inputs) = inputs else {
        return Ok(css);
    };
    for (node, entry) in document.value.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1).map_err(err)?;
        let nepl3_doc_core::model::DocKind::Image { asset, .. } = &entry.kind else {
            continue;
        };
        for input in inputs {
            budget
                .charge(Resource::Work, (asset.id.len() + input.id.len()) as u64 + 1)
                .map_err(err)?;
            if asset.id != input.id {
                continue;
            }
            if let Some((width, height)) =
                nepl3_markup::html::svg::intrinsic_size(input.svg, budget).map_err(err)?
                && height <= 360.0
            {
                // Finite bounded numbers and a numeric arena index cannot
                // introduce CSS delimiters, selectors or HTML raw-text states.
                budget
                    .charge(Resource::AllocationUnits, 1024)
                    .map_err(err)?;
                budget.charge(Resource::Work, 1024).map_err(err)?;
                css.push_str(&format!("\n@container nepl-image (min-width:{width}px){{[data-nepl-id=\"image-{node}\"]>.nepl-image-details{{display:none}}}}\n"));
            }
            break;
        }
    }
    Ok(css)
}
