//! Bounded local multi-page export. Files are created only after the complete
//! page set renders; the final manifest is the completion marker, not a deploy.
use super::*;
use nepl3_doc_core::pages::{PageDocument, PageRegistration, PageSet};
use nepl3_doc_html::pages::{PagesHtmlRequest, render_pages};
use serde::Deserialize;
use std::{collections::BTreeMap, io::Write};
pub mod resources;
use nepl3_core::budget::Budget;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u64,
    pub pages: Vec<Entry>,
    #[serde(default)]
    pub output_limits: resources::OutputLimits,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub source: String,
    pub route: String,
}
pub struct GeneratedPages {
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: String,
}

/// In-memory host input pairs. Paths are logical names, not filesystem access.
pub fn generate(compiled: &Compiled, inputs: &[(Entry, String)]) -> Result<GeneratedPages, String> {
    generate_with_output_budget(compiled, inputs, &mut budget())
}

/// The caller selects the finite output allowance before generation. The same
/// sticky budget covers the entire PageSet; parse/lower retain separate limits.
pub fn generate_with_output_budget(
    compiled: &Compiled,
    inputs: &[(Entry, String)],
    output_budget: &mut Budget,
) -> Result<GeneratedPages, String> {
    output_budget.poll().map_err(err)?;
    let initial_usage = output_budget.usage();
    if inputs.is_empty() || inputs.len() > 128 {
        return Err("PageCountLimit".into());
    }
    let mut total = 0u64;
    let mut pages = Vec::new();
    let mut origins = Vec::new();
    for (entry, input) in inputs {
        total = total.checked_add(input.len() as u64).ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        let (document, profile, parse_usage, lower_usage) = crate::doc::source::with_named_input(
            true,
            compiled,
            input,
            &entry.id,
            "Article",
            |tree, profile, b, _a| {
                let checked = tree.syntax();
                let empty = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                    .map_err(err)?;
                let mut lower_budget = budget();
                let doc = lower::document(
                    checked,
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut lower_budget,
                    &mut c,
                )
                .map_err(err)?;
                Ok((doc, profile.digest(), b.usage(), lower_budget.usage()))
            },
        )?;
        pages.push(PageDocument {
            registration: PageRegistration {
                id: entry.id.clone(),
                source: entry.source.clone(),
                route: entry.route.clone(),
            },
            document,
        });
        origins.push(serde_json::json!({"id":entry.id,"source":entry.source,"route":entry.route,
            "source_sha256":digest_hex(Digest::of(input.as_bytes())),"profile_sha256":digest_hex(profile),
            "operation_limits":resources::limits(budget().limits()),
            "parse_and_validate_usage":resources::usage(parse_usage),"lower_usage":resources::usage(lower_usage)}));
    }
    let request = PagesHtmlRequest {
        set: PageSet { pages },
        options: RenderOptions {
            parallel: ParallelMode::Rows,
        },
    };
    let r = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let rendered = render_pages(&request, r, &mut c, output_budget)
        .map_err(|e| format!("resolve/render: {e:?}; usage={:?}", output_budget.usage()))?;
    let mut files = BTreeMap::new();
    let mut file_kinds = BTreeMap::new();
    for (page, fragment) in request.set.pages.iter().zip(&rendered.fragments) {
        let route = &page.registration.route;
        if !route.ends_with(".html") {
            return Err("HTML route must end in .html".into());
        }
        let html = shell(fragment, output_budget)
            .map_err(|e| format!("serialize: {e}; usage={:?}", output_budget.usage()))?;
        insert(&mut files, route.clone(), html.into_bytes())?;
        file_kinds.insert(route.clone(), "text/html; charset=utf-8");
        let css = match route.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/assets/doc.css"),
            None => "assets/doc.css".into(),
        };
        insert(&mut files, css.clone(), CSS.as_bytes().to_vec())?;
        file_kinds.insert(css, "text/css; charset=utf-8");
    }
    // Reserve the completion marker before writing anything to disk.
    if files.keys().any(|path| conflict(path, "manifest.json")) {
        return Err("reserved manifest path".into());
    }
    let records = files
        .iter()
        .map(|(path, bytes)| {
            serde_json::json!({"path":path,"sha256":digest_hex(Digest::of(bytes)),
        "mime":file_kinds[path],"license":if path.ends_with(".css") {Some("MIT")} else {None}})
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_string_pretty(&serde_json::json!({
        "format":"nepl3.local-doc-pages/1","identity":digest_hex(rendered.identity),"pages":origins,"files":records,
        "execution_identity":digest_hex(resources::execution_identity(rendered.identity,output_budget.limits(),initial_usage)),
        "output_budget":{"contract":"nepl3.local-doc-pages.execution/1","limits":resources::limits(output_budget.limits()),
            "initial_usage":resources::usage(initial_usage),"usage":resources::usage(output_budget.usage())},
        "renderer":"nepl3-doc-html pages/2","options":{"parallel":"Rows"},"viewer_scripts":false,
        "packages":"compiled checked bootstrap fixtures","scope":"Internal Doc page links and checked external http/https/mailto hrefs; no network or destination availability check. Assets and foreign rendering remain unsupported. Not Pages deployment evidence.",
        "budget_scope":"Each parse/lower separately bounded; one shared resolve/render/serialize output budget",
        "output_usage":{"work":output_budget.usage().work,"allocation_units":output_budget.usage().allocation_units,"output_bytes":output_budget.usage().output_bytes}
    })).map_err(err)? + "\n";
    Ok(GeneratedPages { files, manifest })
}
fn conflict(a: &str, b: &str) -> bool {
    let mut left = a.split('/');
    let mut right = b.split('/');
    loop {
        match (left.next(), right.next()) {
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => {
                if a != b {
                    return true;
                }
            }
            (Some(_), Some(_)) => return false,
            _ => return true,
        }
    }
}
fn insert(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    for segment in path.split('/') {
        let base = segment
            .split('.')
            .next()
            .ok_or("empty path segment")?
            .to_ascii_uppercase();
        if segment.ends_with('.')
            || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((base.starts_with("COM") || base.starts_with("LPT"))
                && base.len() == 4
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(format!("nonportable output path: {path}"));
        }
    }
    for (existing, old) in files.iter() {
        if existing == &path && old == &bytes {
            return Ok(());
        }
        if conflict(existing, &path) {
            return Err(format!("output path collision: {path}"));
        }
    }
    files.insert(path, bytes);
    Ok(())
}
pub fn write(manifest: &Path, output: &Path) -> crate::Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(manifest)?
        .take(65_537)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 65_536 {
        return Err("ManifestLimit".into());
    }
    let manifest_data: Manifest = serde_json::from_slice(&bytes)?;
    if manifest_data.version != 1
        || manifest_data.pages.is_empty()
        || manifest_data.pages.len() > 128
    {
        return Err("invalid page manifest version/count".into());
    }
    let root = manifest
        .canonicalize()?
        .parent()
        .ok_or("manifest directory")?
        .to_path_buf();
    let mut inputs = Vec::new();
    let mut total = 0u64;
    for entry in manifest_data.pages {
        let path = crate::repository::local_path(&root, &entry.source)?;
        let input = read_source(&path)?;
        total = total.checked_add(input.len() as u64).ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        inputs.push((entry, input));
    }
    let generated = generate_with_output_budget(
        &compiled()?,
        &inputs,
        &mut manifest_data.output_limits.budget(),
    )?;
    fs::create_dir(output)?;
    for (path, bytes) in generated.files {
        let path = output.join(path);
        fs::create_dir_all(path.parent().ok_or("output parent")?)?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?
            .write_all(&bytes)?;
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("manifest.json"))?
        .write_all(generated.manifest.as_bytes())?;
    Ok(())
}
