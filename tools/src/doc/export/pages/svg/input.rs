use crate::doc::export::pages::{self, resources};
use serde::{Deserialize, Deserializer};
use std::{fs, io::Read, path::Path};

pub(super) const SVG_COUNT: usize = 32;
pub(super) const SVG_BYTES: usize = 4 * nepl3_markup::html::svg::MAX_BYTES;
fn present_input<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    source: String,
    route: String,
    #[serde(default, deserialize_with = "present_input")]
    input: Option<String>,
}
impl From<Entry> for pages::Entry {
    fn from(e: Entry) -> Self {
        Self {
            id: e.id,
            source: e.source,
            route: e.route,
            input: e.input,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    id: String,
    source: String,
    mime: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    page: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u64,
    pages: Vec<Entry>,
    #[serde(default)]
    files: Vec<Entry>,
    assets: Vec<Binding>,
    #[serde(default)]
    display: pages::DisplaySelection,
    #[serde(default)]
    output_limits: resources::OutputLimits,
    #[serde(default)]
    parse_limits: resources::OperationLimits,
    #[serde(default)]
    lower_limits: resources::OperationLimits,
}
pub(super) struct OwnedAsset {
    pub id: String,
    pub source: String,
    pub svg: String,
}
pub(super) struct OwnedBinding {
    pub page: usize,
    pub assets: Vec<OwnedAsset>,
}
pub(super) struct Captured {
    pub inputs: Vec<(pages::Entry, String)>,
    pub files: Vec<(pages::Entry, Vec<u8>)>,
    pub assets: Vec<OwnedBinding>,
    pub options: nepl3_doc_html::RenderOptions,
    pub phases: resources::PhaseLimits,
    pub output: resources::OutputLimits,
}
fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("CapturedInputLimit".into());
    }
    Ok(bytes)
}
fn local(root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    if name.is_empty()
        || name.len() > 4096
        || name.chars().any(|c| c.is_control() || "\\:?#%".contains(c))
        || name
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("InvalidCapturedPath".into());
    }
    let resolved = root.join(name).canonicalize().map_err(|e| e.to_string())?;
    if !resolved.starts_with(root) {
        return Err("CapturedPathEscape".into());
    }
    // Open the resolved path, not the original symlink spelling. This is a
    // containment check, not a race-free capability filesystem guarantee.
    Ok(resolved)
}
fn id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
}
pub(super) fn capture(path: &Path) -> Result<Captured, String> {
    let bytes = bounded(path, 65_536)?;
    let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
    crate::repository::json::validate(text).map_err(|e| e.to_string())?;
    let manifest: Manifest = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if manifest.version != 1
        || manifest.pages.is_empty()
        || manifest.pages.len() > 128
        || manifest.files.len() > 128
        || manifest.assets.len() > 128
    {
        return Err("InvalidSvgPageManifestCount".into());
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let root = canonical.parent().ok_or("ManifestRoot")?;
    let mut inputs = Vec::new();
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut names = std::collections::BTreeSet::new();
    for entry in manifest.pages {
        if !id(&entry.id) || !names.insert(entry.id.clone()) {
            return Err("DuplicateOrInvalidPageId".into());
        }
        let e: pages::Entry = entry.into();
        let bytes = bounded(
            &local(root, pages::input_path(&e)?)?,
            crate::doc::export::MAX_SOURCE_BYTES,
        )?;
        total = total.checked_add(bytes.len() as u64).ok_or("SourceLimit")?;
        if total > crate::doc::export::MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        inputs.push((e, String::from_utf8(bytes).map_err(|e| e.to_string())?));
    }
    for entry in manifest.files {
        let e: pages::Entry = entry.into();
        let bytes = bounded(
            &local(root, pages::input_path(&e)?)?,
            crate::doc::export::MAX_SOURCE_BYTES,
        )?;
        total = total.checked_add(bytes.len() as u64).ok_or("SourceLimit")?;
        if total > crate::doc::export::MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        files.push((e, bytes));
    }
    let mut assets = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut count = 0usize;
    let mut svg_bytes = 0usize;
    for binding in manifest.assets {
        let page = inputs
            .iter()
            .position(|(p, _)| p.id == binding.page)
            .ok_or("UnknownSvgPage")?;
        if !seen.insert(page) {
            return Err("DuplicateSvgPage".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut owned = Vec::new();
        for asset in binding.assets {
            count = count.checked_add(1).ok_or("SvgCountLimit")?;
            if count > SVG_COUNT {
                return Err("SvgCountLimit".into());
            }
            if !id(&asset.id) || !ids.insert(asset.id.clone()) || asset.mime != "image/svg+xml" {
                return Err("InvalidSvgBinding".into());
            }
            let bytes = bounded(
                &local(root, &asset.source)?,
                nepl3_markup::html::svg::MAX_BYTES as u64,
            )?;
            svg_bytes = svg_bytes.checked_add(bytes.len()).ok_or("SvgByteLimit")?;
            if svg_bytes > SVG_BYTES {
                return Err("SvgByteLimit".into());
            }
            owned.push(OwnedAsset {
                id: asset.id,
                source: asset.source,
                svg: String::from_utf8(bytes).map_err(|e| e.to_string())?,
            });
        }
        assets.push(OwnedBinding {
            page,
            assets: owned,
        });
    }
    Ok(Captured {
        inputs,
        files,
        assets,
        options: manifest.display.into(),
        phases: resources::PhaseLimits {
            parse: manifest.parse_limits.budget().limits(),
            lower: manifest.lower_limits.budget().limits(),
        },
        output: manifest.output_limits,
    })
}
