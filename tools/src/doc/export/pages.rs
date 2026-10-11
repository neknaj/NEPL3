//! Bounded local multi-page export. Files are created only after the complete
//! page set renders; the final manifest is the completion marker, not a deploy.
use super::*;
use nepl3_doc_core::pages::{FileBytes, PageDocument, PageFile, PageRegistration, PageSet};
use nepl3_doc_html::pages::{PagesHtmlRequest, render_pages_with_display};
pub(crate) mod aliases;
mod code;
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
    pub files: Vec<Entry>,
    #[serde(default)]
    pub output_limits: resources::OutputLimits,
    #[serde(default)]
    pub parse_limits: resources::OperationLimits,
    #[serde(default)]
    pub lower_limits: resources::OperationLimits,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub source: String,
    pub route: String,
    /// Optional physical file relative to the manifest directory. `source`
    /// remains the logical namespace used by Doc links.
    #[serde(default)]
    pub input: Option<String>,
}
fn input_path(entry: &Entry) -> Result<&str, String> {
    let Some(path) = entry.input.as_deref() else {
        return Ok(&entry.source);
    };
    if path.len() > 4096
        || path.chars().any(|c| c.is_control() || "\\:?#%".contains(c))
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("InvalidInputPath".into());
    }
    Ok(path)
}
pub struct GeneratedPages {
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: String,
    pub provenance: Option<DisplayProvenance>,
}
pub struct DisplayProvenance {
    pub pages: PageSet,
    pub math: Vec<Vec<super::math::Occurrence>>,
    pub rendered: nepl3_doc_html::pages::RenderedPages,
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
    generate_with_phase_limits(
        compiled,
        inputs,
        resources::PhaseLimits::default(),
        output_budget,
    )
}
/// Parse/lower are separate operations per page. Only output accepts an existing
/// caller budget; its consumed usage and cancellation remain unchanged.
pub fn generate_with_phase_limits(
    compiled: &Compiled,
    inputs: &[(Entry, String)],
    phases: resources::PhaseLimits,
    output_budget: &mut Budget,
) -> Result<GeneratedPages, String> {
    generate_with_resources(compiled, inputs, &[], phases, output_budget)
}
/// File bytes are explicitly supplied by the host, never discovered by link
/// resolution. The same output budget covers their identity and serialization.
pub fn generate_with_resources(
    compiled: &Compiled,
    inputs: &[(Entry, String)],
    resources: &[(Entry, Vec<u8>)],
    phases: resources::PhaseLimits,
    output_budget: &mut Budget,
) -> Result<GeneratedPages, String> {
    generate_with_aliases(compiled, inputs, resources, phases, output_budget, &[])
}

/// Explicit native-host compatibility composition; ordinary page export is unchanged.
pub(crate) fn generate_with_aliases(
    compiled: &Compiled,
    inputs: &[(Entry, String)],
    resources: &[(Entry, Vec<u8>)],
    phases: resources::PhaseLimits,
    output_budget: &mut Budget,
    aliases: &[aliases::PageAliases],
) -> Result<GeneratedPages, String> {
    generate_selected(compiled, inputs, resources, phases, output_budget, aliases, super::MathRenderer::KaTeXPreferred)
}
pub fn generate_with_renderer(compiled: &Compiled, inputs: &[(Entry, String)], renderer: super::MathRenderer) -> Result<GeneratedPages, String> {
 generate_selected(compiled, inputs, &[], resources::PhaseLimits::default(), &mut budget(), &[], renderer)
}
fn generate_selected(compiled: &Compiled, inputs: &[(Entry, String)], resources: &[(Entry, Vec<u8>)],
 phases: resources::PhaseLimits, output_budget: &mut Budget, aliases: &[aliases::PageAliases], renderer: super::MathRenderer,
) -> Result<GeneratedPages, String> {
    output_budget.poll().map_err(err)?;
    let initial_usage = output_budget.usage();
    if inputs.is_empty() || inputs.len() > 128 {
        return Err("PageCountLimit".into());
    }
    aliases::check_binding(inputs, aliases, output_budget)?;
    let mut total = 0u64;
    let mut pages = Vec::new();
    let mut origins = Vec::new();
    let mut profiles = Vec::new();
    let mut page_code = Vec::new();
    if resources.len() > 128 {
        return Err("FileCountLimit".into());
    }
    let mut registered_files = Vec::new();
    let mut file_origins = Vec::new();
    for (entry, bytes) in resources {
        let physical = input_path(entry)?;
        total = total.checked_add(bytes.len() as u64).ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        output_budget
            .charge(nepl3_core::budget::Resource::Work, bytes.len() as u64)
            .map_err(err)?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (bytes.len()
                    + entry.id.len()
                    + entry.source.len()
                    + entry.route.len()
                    + core::mem::size_of::<PageFile>()) as u64,
            )
            .map_err(err)?;
        registered_files.push(PageFile {
            registration: PageRegistration {
                id: entry.id.clone(),
                source: entry.source.clone(),
                route: entry.route.clone(),
            },
            content: FileBytes(bytes.clone()),
        });
        file_origins.push(serde_json::json!({"id":entry.id,"source":entry.source,"route":entry.route,"input":physical,"sha256":digest_hex(Digest::of(bytes)),"bytes":bytes.len()}));
    }
    for (entry, input) in inputs {
        let physical = input_path(entry)?;
        total = total.checked_add(input.len() as u64).ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        let (document, profile, parse_usage, lower_usage, code) =
            crate::doc::source::with_named_input_limits(
                true,
                compiled,
                input,
                &entry.id,
                "Article",
                phases.parse,
                |tree, profile, b, _a| {
                    let checked = tree.syntax();
                    let empty = SourceStore::default();
                    let mut admission = SourceAdmission::default();
                    let mut c = FoundationCodec::new(profile.registry(), &empty, &mut admission)
                        .map_err(err)?;
                    let mut lower_budget = Budget::new(phases.lower);
                    let doc = lower::document(
                        checked,
                        &compiled.doc.package.schema,
                        Category::Article,
                        profile.registry(),
                        &mut lower_budget,
                        &mut c,
                    )
                    .map_err(|e| format!("lower: {e:?}; usage={:?}", lower_budget.usage()))?;
                    let code = code::PreparedPageCode::prepare(&doc, tree, profile, output_budget)?;
                    Ok((doc, profile.digest(), b.usage(), lower_budget.usage(), code))
                },
            )?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                (2 * core::mem::size_of::<code::PreparedPageCode>()) as u64,
            )
            .map_err(err)?;
        page_code.push(code);
        profiles.push(profile);
        pages.push(PageDocument {
            registration: PageRegistration {
                id: entry.id.clone(),
                source: entry.source.clone(),
                route: entry.route.clone(),
            },
            document,
        });
        origins.push(serde_json::json!({"id":entry.id,"source":entry.source,"route":entry.route,
            "input":physical,
            "source_sha256":digest_hex(Digest::of(input.as_bytes())),"profile_sha256":digest_hex(profile),
            "operation_limits":if phases.parse==phases.lower {resources::limits(phases.parse)} else {serde_json::Value::Null},
            "parse_limits":resources::limits(phases.parse),"lower_limits":resources::limits(phases.lower),
            "parse_initial_usage":resources::usage(Default::default()),"lower_initial_usage":resources::usage(Default::default()),
            "parse_and_validate_usage":resources::usage(parse_usage),"lower_usage":resources::usage(lower_usage)}));
    }
    let request = PagesHtmlRequest {
        set: PageSet {
            pages,
            files: registered_files,
        },
        options: RenderOptions {
            parallel: ParallelMode::Rows,
        },
    };
    let r = &compiled.doc.registry;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let mut page_math = Vec::new();
    for _ in &request.set.pages {
        output_budget.charge(nepl3_core::budget::Resource::AllocationUnits,
            core::mem::size_of::<Vec<super::math::Occurrence>>() as u64).map_err(err)?;
        page_math.try_reserve_exact(1).map_err(|_| err(output_budget.stop(nepl3_core::budget::StopReason::AllocationLimit)))?;
        page_math.push(Vec::new());
    }
    let displayed = render_pages_with_display(
        &request,
        r,
        &mut c,
        output_budget,
        &mut |page, embed, index, codec, b| {
            let code = usize::try_from(page)
                .ok()
                .and_then(|page| page_code.get(page))
                .ok_or("CodePageMissing")?;
            if embed.kind == nepl3_doc_core::model::EmbedKind::Code { code.render(embed, index, r, b) }
            else { let records = page_math.get_mut(page as usize).ok_or("MathPageMissing")?;
              super::math::DisplayHost { compiled, registry: r, codec, preference: renderer }.render(embed, index, records, b)
            }
        },
    )
    .map_err(|e| format!("resolve/render: {e:?}; usage={:?}", output_budget.usage()))?;
    for ((records, placements), page) in page_math.iter_mut().zip(&displayed.foreign).zip(&request.set.pages) {
      super::math::compose(records, placements, &page.document.value.embeds, output_budget)?;
    }
    let mut rendered = displayed.pages;
    for (page, fragment) in request.set.pages.iter().zip(&mut rendered.fragments) {
        for input in aliases {
            output_budget
                .charge(
                    nepl3_core::budget::Resource::Work,
                    (input.page.len() + page.registration.id.len() + 1) as u64,
                )
                .map_err(err)?;
            if input.page == page.registration.id {
                aliases::apply(fragment, &input.values, output_budget)?;
                break;
            }
        }
    }
    let rendered_usage = output_budget.usage();
    let mut files = BTreeMap::new();
    let mut file_kinds = BTreeMap::new();
    for file in &request.set.files {
        output_budget
            .charge(
                nepl3_core::budget::Resource::OutputBytes,
                file.content.0.len() as u64,
            )
            .map_err(err)?;
        output_budget
            .charge(
                nepl3_core::budget::Resource::AllocationUnits,
                file.content.0.len() as u64,
            )
            .map_err(err)?;
        insert(
            &mut files,
            file.registration.route.clone(),
            file.content.0.clone(),
        )?;
        file_kinds.insert(file.registration.route.clone(), "application/octet-stream");
    }
    for (page, fragment) in request.set.pages.iter().zip(&rendered.fragments) {
        let route = &page.registration.route;
        if !route.ends_with(".html") {
            return Err("HTML route must end in .html".into());
        }
        let html = shell(fragment, output_budget).map_err(|e| {
            format!(
                "page {route}: {e}; render completed usage={rendered_usage:?}; usage={:?}",
                output_budget.usage()
            )
        })?;
        insert(&mut files, route.clone(), html.into_bytes())?;
        file_kinds.insert(route.clone(), "text/html; charset=utf-8");
        let css = match route.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/assets/doc.css"),
            None => "assets/doc.css".into(),
        };
        if request
            .set
            .files
            .iter()
            .any(|file| conflict(&file.registration.route, &css))
        {
            return Err("registered file conflicts with generated stylesheet".into());
        }
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
    let output_identity =
        resources::execution_identity(rendered.identity, output_budget.limits(), initial_usage);
    let manifest = serde_json::to_string_pretty(&serde_json::json!({
        "format":"nepl3.local-doc-pages/1","identity":digest_hex(rendered.identity),"pages":origins,"files":records,
        "execution_identity":digest_hex(output_identity),
        "phase_execution":{"contract":"nepl3.local-doc-pages.phases/1","identity":digest_hex(resources::phase_identity(output_identity,&profiles,phases))},
        "output_budget":{"contract":"nepl3.local-doc-pages.execution/1","limits":resources::limits(output_budget.limits()),
            "initial_usage":resources::usage(initial_usage),"usage":resources::usage(output_budget.usage())},
        "registered_files":file_origins,
        "renderer":"nepl3-doc-html pages/4","options":{"parallel":"Rows","math_renderer":renderer.as_str()},
        "math_diagnostics":page_math.iter().map(|records| super::math::diagnostics(records)).collect::<Vec<_>>(),"viewer_scripts":false,
        "packages":"compiled checked bootstrap fixtures","scope":"Internal Doc page links and checked external http/https/mailto hrefs; no network or destination availability check. Retained Code uses shared syntax-only highlighting without guest evaluation; Math uses independent MathML with explicit renderer fallback diagnostics; assets and other foreign kinds remain unsupported. Not Pages deployment evidence.",
        "budget_scope":"Each parse/lower separately bounded; one shared resolve/render/serialize output budget",
        "output_usage":{"work":output_budget.usage().work,"allocation_units":output_budget.usage().allocation_units,"output_bytes":output_budget.usage().output_bytes}
    })).map_err(err)? + "\n";
    let mut generated = GeneratedPages { files, manifest, provenance: Some(DisplayProvenance { pages: request.set, math: page_math, rendered }) };
    aliases::record(&mut generated, aliases)?;
    Ok(generated)
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
 write_with_renderer(manifest, output, super::MathRenderer::KaTeXPreferred)
}
pub fn write_with_renderer(manifest: &Path, output: &Path, renderer: super::MathRenderer) -> crate::Result<()> {
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
        || manifest_data.files.len() > 128
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
        let path = crate::repository::local_path(&root, input_path(&entry)?)?;
        let input = read_source(&path)?;
        total = total.checked_add(input.len() as u64).ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        inputs.push((entry, input));
    }
    let mut resources = Vec::new();
    for entry in manifest_data.files {
        let path = crate::repository::local_path(&root, input_path(&entry)?)?;
        let mut content = Vec::new();
        fs::File::open(path)?
            .take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut content)?;
        total = total
            .checked_add(content.len() as u64)
            .ok_or("SourceLimit")?;
        if total > MAX_SOURCE_BYTES {
            return Err("SourceLimit".into());
        }
        resources.push((entry, content));
    }
    let generated = generate_selected(
        &compiled()?,
        &inputs,
        &resources,
        resources::PhaseLimits {
            parse: manifest_data.parse_limits.budget().limits(),
            lower: manifest_data.lower_limits.budget().limits(),
        },
        &mut manifest_data.output_limits.budget(),
        &[], renderer,
    )?;
    write_generated(generated, output)
}

// Only call with the complete output of this module's checked generator.
pub(crate) fn write_generated(generated: GeneratedPages, output: &Path) -> crate::Result<()> {
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
