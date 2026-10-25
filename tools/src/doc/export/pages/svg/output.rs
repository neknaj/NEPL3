use super::*;
use crate::doc::export::{self, CssMode};
use nepl3_core::budget::{Resource, Usage};
use nepl3_doc_html::{ParallelMode, assets::SvgMode};
use serde::Serialize;
use std::io::Write;

fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn charge(b: &mut Budget, work: usize, allocation: usize) -> Result<(), String> {
    b.charge(Resource::Work, work as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, allocation as u64)
        .map_err(err)
}
fn insert(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    value: Vec<u8>,
    b: &mut Budget,
) -> Result<(), String> {
    charge(b, 2 * path.len() + 16, path.len() + 256)?;
    if super::super::conflict(&path, "manifest.json") {
        return Err("ReservedManifestPath".into());
    }
    for existing in files.keys() {
        charge(b, 2 * (existing.len() + path.len() + 1), 0)?;
        if super::super::conflict(existing, &path) {
            return Err("OutputPathCollision".into());
        }
    }
    // Reuse portable path checks, after accounting for their full scans.
    super::super::insert(files, path, value)
}
fn hex(d: Digest, b: &mut Budget) -> Result<String, String> {
    charge(b, 64, 64)?;
    Ok(crate::doc::export::digest_hex(d))
}

#[derive(Serialize)]
struct FileRecord<'a> {
    path: &'a str,
    bytes: usize,
    sha256: String,
}
#[derive(Serialize)]
struct Display<'a> {
    mode: &'static str,
    language: Option<&'a str>,
    fallbacks: &'a [String],
}
#[derive(Serialize)]
pub(super) struct Formula<'a> {
    pub root: u64,
    pub display: &'static str,
    pub owner_path: &'a [crate::doc::export::math::generation::OwnerStep],
    pub renderer: &'static str,
    pub selection_reason: &'static str,
    pub fallback_reason: Option<&'static str>,
}
#[derive(Serialize)]
pub(super) struct CapturedSvg<'a> {
    pub page: &'a str,
    pub id: &'a str,
    pub source: &'a str,
    pub bytes: usize,
    pub sha256: String,
}
#[derive(Serialize)]
pub(super) struct SourceEvidence<'a> {
    pub scope: &'static str,
    pub math_renderer: &'static str,
    pub sources: &'a [serde_json::Value],
    pub registered_files: &'a [serde_json::Value],
    pub svg_capture: Vec<CapturedSvg<'a>>,
    pub svg_total_byte_limit: usize,
    pub svg_total_count_limit: usize,
    pub math: Vec<Vec<Formula<'a>>>,
}
#[derive(Serialize)]
struct Receipt<'a> {
    format: &'static str,
    input_identity: String,
    identity_scope: &'static str,
    files: Vec<FileRecord<'a>>,
    display: Display<'a>,
    svg: &'static str,
    css: &'static str,
    katex_generation: bool,
    limits: serde_json::Value,
    initial_usage: serde_json::Value,
    pre_receipt_usage: serde_json::Value,
    scope: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_input: Option<&'a SourceEvidence<'a>>,
}
struct Writer<'a> {
    bytes: Vec<u8>,
    budget: &'a mut Budget,
}
impl Write for Writer<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let failure = |e| std::io::Error::other(format!("{e:?}"));
        self.budget
            .charge(Resource::Work, bytes.len() as u64)
            .map_err(failure)?;
        self.budget
            .charge(Resource::AllocationUnits, 2 * bytes.len() as u64)
            .map_err(failure)?;
        self.budget
            .charge(Resource::OutputBytes, bytes.len() as u64)
            .map_err(failure)?;
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| failure(self.budget.stop(StopReason::AllocationLimit)))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn package(
    set: &PageSet,
    options: &RenderOptions,
    inputs: &[RootAssets<'_>],
    rendered: &planned::Rendered<'_, '_>,
    identity: Digest,
    initial: Usage,
    source_input: Option<&SourceEvidence<'_>>,
    b: &mut Budget,
) -> Result<(BTreeMap<String, Vec<u8>>, String), String> {
    if rendered.pages.fragments.len() != set.pages.len() {
        return Err("PageCountMismatch".into());
    }
    let mut files = BTreeMap::new();
    for file in &set.files {
        charge(
            b,
            file.content.0.len() + file.registration.route.len(),
            file.content.0.len() + file.registration.route.len(),
        )?;
        b.charge(Resource::OutputBytes, file.content.0.len() as u64)
            .map_err(err)?;
        insert(
            &mut files,
            file.registration.route.clone(),
            file.content.0.clone(),
            b,
        )?;
    }
    for (index, (page, fragment)) in set.pages.iter().zip(&rendered.pages.fragments).enumerate() {
        let route = &page.registration.route;
        charge(b, route.len(), 0)?;
        if !route.ends_with(".html") {
            return Err("HtmlRouteRequired".into());
        }
        let mut assets = &[][..];
        for binding in inputs {
            charge(b, 1, 0)?;
            if binding.page == index as u64 {
                assets = binding.inputs;
                break;
            }
        }
        let mut css = export::assets::document_css(&page.document, Some(assets), b)?;
        let html = export::shell_with_assets(
            &fragment.fragment,
            CssMode::External,
            Some(SvgMode::Embedded),
            &mut css,
            None,
            b,
        )?;
        charge(b, route.len() + 72, 72)?;
        let css_name = format!("doc-{}.css", hex(Digest::of(route.as_bytes()), b)?);
        charge(
            b,
            html.len() + css_name.len(),
            html.len() + 2 * css_name.len() + 64,
        )?;
        b.charge(
            Resource::OutputBytes,
            (css_name.len() - "doc.css".len()) as u64,
        )
        .map_err(err)?;
        let html = html.replacen(
            "href=\"assets/doc.css\"",
            &format!("href=\"assets/{css_name}\""),
            1,
        );
        charge(
            b,
            route.len() + css_name.len() + 8,
            route.len() + css_name.len() + 8,
        )?;
        let css_path = match route.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/assets/{css_name}"),
            None => format!("assets/{css_name}"),
        };
        charge(b, route.len(), route.len())?;
        insert(&mut files, route.clone(), html.into_bytes(), b)?;
        insert(&mut files, css_path, css.into_bytes(), b)?;
    }
    let mut records = Vec::new();
    for (path, bytes) in &files {
        charge(
            b,
            bytes.len() + 1,
            2 * core::mem::size_of::<FileRecord<'_>>(),
        )?;
        records
            .try_reserve(1)
            .map_err(|_| err(b.stop(StopReason::AllocationLimit)))?;
        records.push(FileRecord {
            path,
            bytes: bytes.len(),
            sha256: hex(Digest::of(bytes), b)?,
        });
    }
    let display = match &options.parallel {
        ParallelMode::Rows => Display {
            mode: "Rows",
            language: None,
            fallbacks: &[],
        },
        ParallelMode::Columns => Display {
            mode: "Columns",
            language: None,
            fallbacks: &[],
        },
        ParallelMode::Single {
            language,
            fallbacks,
        } => Display {
            mode: "Single",
            language: Some(language),
            fallbacks,
        },
    };
    let input_identity = hex(identity, b)?;
    // Fixed-shape limits/usage records: no input-sized JSON allocation here.
    charge(b, 8192, 8192)?;
    let receipt = Receipt {
        source_input,
        format: "nepl3.local-doc-pages.root-svg-package/1",
        input_identity,
        identity_scope: "canonical PageSet, exact display and ordered raw root SVG bindings; not callback identity or portable proof",
        files: records,
        display,
        svg: "embedded",
        css: "external-page-specific",
        katex_generation: false,
        limits: super::super::resources::limits(b.limits()),
        initial_usage: super::super::resources::usage(initial),
        pre_receipt_usage: super::super::resources::usage(b.usage()),
        scope: "Externally retained plans and owners. Validated native fragments, root-only embedded SVG, page links, per-page CSS and CSP. Caller foreign display is not evaluated or mathematically proved. No CLI, captured retry state, external SVG packaging, KaTeX generation, deployment or portable-provider acceptance.",
    };
    let mut writer = Writer {
        bytes: Vec::new(),
        budget: b,
    };
    serde_json::to_writer_pretty(&mut writer, &receipt).map_err(err)?;
    writer.write_all(b"\n").map_err(err)?;
    let manifest = String::from_utf8(writer.bytes).map_err(err)?;
    Ok((files, manifest))
}
