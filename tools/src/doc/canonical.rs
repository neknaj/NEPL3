//! Repository document source selection and exact generated-projection checks.
//! This host adapter never infers migration approval from successful rendering.
use crate::{Result, doc::projection::annotated::host, repository};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

mod compatibility;
mod continuation;
mod projection;
pub(crate) mod references;
#[cfg(test)]
mod tests;

use crate::doc::projection::annotated::host::RENDERER;
const MAX_REGISTRY: u64 = 1_048_576;

#[derive(Default)]
struct OutputRoute {
    file: bool,
    children: BTreeMap<String, usize>,
}
#[derive(Default)]
struct OutputRoutes {
    // Flat ownership also bounds destruction stack depth on smaller-stack hosts.
    nodes: Vec<OutputRoute>,
}
impl OutputRoutes {
    fn insert(&mut self, path: &str) -> Result<()> {
        if self.nodes.is_empty() {
            self.nodes.push(OutputRoute::default());
        }
        let mut index = 0;
        for part in path.split('/') {
            if self.nodes[index].file {
                return Err("output file used as directory".into());
            }
            let key = part.to_ascii_lowercase();
            index = if let Some(&next) = self.nodes[index].children.get(&key) {
                next
            } else {
                let next = self.nodes.len();
                self.nodes.push(OutputRoute::default());
                self.nodes[index].children.insert(key, next);
                next
            };
        }
        let node = &mut self.nodes[index];
        if node.file || !node.children.is_empty() {
            return Err("duplicate or overlapping output route".into());
        }
        node.file = true;
        Ok(())
    }
}

fn portable_path(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 4096
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'/' | b'.'))
        && name.split('/').all(|part| {
            let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            !part.is_empty()
                && !part.ends_with('.')
                && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !((base.starts_with("COM") || base.starts_with("LPT"))
                    && base.len() == 4
                    && matches!(base.as_bytes()[3], b'1'..=b'9'))
        })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    /// Per-page parse and lower allowances, selected before starting work.
    #[serde(default)]
    pub parse_limits: super::export::pages::resources::OperationLimits,
    #[serde(default)]
    pub lower_limits: super::export::pages::resources::OperationLimits,
    pub pages: Vec<Page>,
    /// Passive Markdown, explicitly scoped JSON/example/test sources, or static
    /// SVG files in an all-footnotes batch. None becomes a semantic Doc page.
    #[serde(default)]
    pub files: Vec<ReferenceFile>,
    /// Output-only copies of freshly generated canonical Markdown.
    #[serde(default)]
    pub compatibility: Vec<compatibility::Copy>,
    /// Markdown batch allowance; old-only checks keep per-page limits.
    #[serde(default)]
    pub output_limits: super::export::pages::resources::OutputLimits,
    /// Separate HTML output allowance, selected before generating the PageSet.
    #[serde(default)]
    pub html_output_limits: super::export::pages::resources::OutputLimits,
}

impl Registry {
    fn phases(&self) -> super::export::pages::resources::PhaseLimits {
        super::export::pages::resources::PhaseLimits {
            parse: self.parse_limits.budget().limits(),
            lower: self.lower_limits.budget().limits(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub id: String,
    pub source: String,
    pub projection: String,
    pub aliases: String,
    pub route: String,
    pub renderer: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceFile {
    pub id: String,
    pub source: String,
    pub route: String,
}

fn passive_paths(file: &ReferenceFile, svg: bool) -> bool {
    if svg {
        return file.source.ends_with(".svg") && file.route.ends_with(".svg");
    }
    if !file.route.starts_with("sources/") {
        return false;
    }
    let markdown = file.source.starts_with("doc/")
        && file.source.ends_with(".md")
        && file.route.ends_with(".md");
    let json = (file.source.starts_with("design/")
        || file.source == "implementation-status.json"
        || file.source == "doc/canonical.json"
        || matches!(
            file.source.as_str(),
            "doc/history/web-tea-import.json"
                | "doc/history/source-manifest.json"
                | "doc/history/design-validation.json"
        ))
        && file.source.ends_with(".json")
        && file.route.ends_with(".json");
    // Exact inert examples/test source needed by the authoring guide, not a
    // general source-file export facility. No parser or compiler is invoked.
    let example = matches!(
        file.source.as_str(),
        "examples/document/line-break.nepld" | "examples/document/linear-combination.nepld"
    ) && file.route.ends_with(".nepld");
    let rust_test = file.source == "tools/tests/doc/print.rs" && file.route.ends_with(".rs");
    markdown || json || example || rust_test
}

fn reference_inputs(
    root: &Path,
    files: Vec<ReferenceFile>,
) -> Result<Vec<(super::export::pages::Entry, Vec<u8>)>> {
    let mut inputs = Vec::new();
    let mut total = 0usize;
    for file in files {
        let bytes = bounded(root, &file.source, 262_144)?;
        std::str::from_utf8(&bytes)?;
        total = total.checked_add(bytes.len()).ok_or("ReferenceLimit")?;
        if total > 2_097_152 {
            return Err("ReferenceLimit".into());
        }
        inputs.push((
            super::export::pages::Entry {
                id: file.id,
                source: file.source,
                route: file.route,
                input: None,
            },
            bytes,
        ));
    }
    Ok(inputs)
}

/// If the active registry is also an exported reference, its passive bytes
/// must describe the exact selection already admitted for this operation.
/// Keep normal path and resource checks, and fail rather than mix snapshots.
fn registry_reference_inputs(
    root: &Path,
    files: Vec<ReferenceFile>,
    manifest: &str,
    raw: &[u8],
) -> Result<Vec<(super::export::pages::Entry, Vec<u8>)>> {
    let inputs = reference_inputs(root, files)?;
    let selected = Path::new(manifest);
    if inputs.iter().any(|(entry, bytes)| {
        // Path components normalize repeated separators and interior '.'.
        // Fold ASCII case like registry collision checks so a Windows
        // spelling cannot evade the snapshot check. Ambiguous spellings
        // fail closed on case-sensitive hosts as well.
        Path::new(&entry.source)
            .components()
            .map(|part| part.as_os_str().as_encoded_bytes().to_ascii_lowercase())
            .eq(selected
                .components()
                .map(|part| part.as_os_str().as_encoded_bytes().to_ascii_lowercase()))
            && bytes != raw
    }) {
        return Err("active registry reference differs from captured registry".into());
    }
    Ok(inputs)
}

pub(crate) fn bounded(root: &Path, relative: &str, limit: u64) -> Result<Vec<u8>> {
    // Require real regular repository files; aliases or platform links cannot
    // turn a canonical record into a write/read outside its declared path.
    let path = repository::local_path(root, relative)?;
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(format!("canonical path contains a symlink: {relative}").into());
        }
    }
    if !fs::symlink_metadata(&path)?.is_file() {
        return Err(format!("canonical path is not a regular file: {relative}").into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("canonical input exceeds limit: {relative}").into());
    }
    Ok(bytes)
}

/// Validate the source-selection manifest, without claiming semantic approval.
pub fn load(root: &Path, manifest: &str) -> Result<Registry> {
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    parse_registry(&raw)
}

pub(crate) fn parse_registry(raw: &[u8]) -> Result<Registry> {
    let text = std::str::from_utf8(raw)?;
    repository::json::validate(text)?;
    let registry: Registry = serde_json::from_str(text)?;
    if registry.version != 1 || registry.pages.is_empty() || registry.pages.len() > 1000 {
        return Err("unsupported or empty canonical registry".into());
    }
    if registry
        .pages
        .iter()
        .any(|p| p.renderer == projection::COMPACT_RENDERER)
        && !registry
            .pages
            .iter()
            .all(|p| p.renderer == projection::COMPACT_RENDERER)
    {
        return Err("compact page identity cannot be mixed with legacy renderers".into());
    }
    let svg = registry
        .pages
        .iter()
        .all(|p| projection::is_footnotes(&p.renderer));
    if (!svg
        || registry
            .files
            .iter()
            .any(|file| !file.source.ends_with(".svg") || !file.route.ends_with(".svg")))
        && !registry.files.is_empty()
        && registry
            .pages
            .iter()
            .any(|p| projection::is_footnotes(&p.renderer))
    {
        return Err("footnote page sets cannot depend on unexported passive files".into());
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut shared_aliases = BTreeSet::new();
    let mut routes = BTreeSet::new();
    for page in &registry.pages {
        if page.id.is_empty()
            || !page
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !ids.insert(&page.id)
            || !matches!(
                page.renderer.as_str(),
                RENDERER
                    | projection::RENDERER
                    | projection::FOOTNOTES_RENDERER
                    | projection::COMPACT_RENDERER
            )
        {
            return Err("invalid canonical page identity or renderer".into());
        }
        let footnotes = projection::is_footnotes(&page.renderer);
        // Exact paired migration exceptions; arbitrary root/crate/language/example/tool files remain rejected.
        let sentence_readme = page.source == "crates/languages/sentence/core/README.nepld"
            && page.projection == "crates/languages/sentence/core/README.md";
        let sentence_surface = page.source == "languages/Sentence/README.nepld"
            && page.projection == "languages/Sentence/README.md";
        let math_example = page.source == "examples/document/math/README.nepld"
            && page.projection == "examples/document/math/README.md";
        let math_renderer_tests = page.source == "tools/audit/math/README.nepld"
            && page.projection == "tools/audit/math/README.md";
        let math_mathml = page.source == "crates/languages/math/mathml/README.nepld"
            && page.projection == "crates/languages/math/mathml/README.md";
        let math_tex = page.source == "crates/languages/math/tex/README.nepld"
            && page.projection == "crates/languages/math/tex/README.md";
        let security = page.source == "SECURITY.nepld" && page.projection == "SECURITY.md";
        let conformance_history = page.source == "conformance/history.nepld"
            && page.projection == "conformance/history.md";
        for (name, suffix) in [
            (&page.source, ".nepld"),
            (&page.projection, ".md"),
            (&page.aliases, ".json"),
        ] {
            let reused_alias = footnotes && suffix == ".json" && shared_aliases.contains(name);
            if !portable_path(name)
                || (!footnotes
                    && !name.starts_with("doc/")
                    && !((sentence_readme
                        || sentence_surface
                        || math_mathml
                        || math_tex
                        || math_renderer_tests
                        || math_example
                        || security
                        || conformance_history)
                        && suffix != ".json"))
                || !name.ends_with(suffix)
                || (!reused_alias && !paths.insert(name.to_ascii_lowercase()))
            {
                return Err(format!("invalid or repeated canonical file path: {name}").into());
            }
            if footnotes && suffix == ".json" {
                shared_aliases.insert(name.clone());
            }
        }
        if footnotes
            && page
                .projection
                .split('/')
                .next()
                .is_some_and(|part| part.eq_ignore_ascii_case("manifest.json"))
        {
            return Err("projection overlaps the reserved manifest.json receipt".into());
        }
        // Routes are portable URL paths, not host filesystem paths.
        if !portable_path(&page.route)
            || if footnotes {
                page.route != page.projection
            } else {
                !page.route.ends_with(".html")
            }
            || !routes.insert(page.route.to_ascii_lowercase())
        {
            return Err("invalid or repeated canonical page route".into());
        }
    }
    if registry.files.len() > 128 {
        return Err("ReferenceCountLimit".into());
    }
    for file in &registry.files {
        if file.id.is_empty()
            || !file
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || !ids.insert(&file.id)
            || !portable_path(&file.source)
            || !passive_paths(file, svg)
            || !paths.insert(file.source.to_ascii_lowercase())
            || !portable_path(&file.route)
            || !routes.insert(file.route.to_ascii_lowercase())
        {
            return Err("invalid or colliding registered file".into());
        }
    }
    if svg {
        if registry.pages.len() > 128 {
            return Err("PageCountLimit".into());
        }
        let mut output = OutputRoutes::default();
        output.insert("manifest.json")?;
        for page in &registry.pages {
            output.insert(&page.projection)?;
        }
        for file in &registry.files {
            output.insert(&file.route)?;
        }
    }
    // Every registered source, alias and projection is a file. Reject a known
    // file/directory conflict before staging any output, including old-only
    // registries that do not enter the PageSet resolver. Testing every ancestor
    // avoids missing a parent when another lexicographic name falls between.
    for path in &paths {
        for (end, _) in path.match_indices('/') {
            if paths.contains(&path[..end]) {
                return Err(
                    format!("canonical file path used as directory: {}", &path[..end]).into(),
                );
            }
        }
    }
    compatibility::validate(&registry)?;
    Ok(registry)
}

/// Regenerate from the Doc source and compare exact bytes; never adopt or write
/// a changed Markdown projection as a new expectation.
pub fn check(root: &Path, manifest: &str) -> Result<()> {
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let registry = parse_registry(&raw)?;
    let phases = registry.phases();
    // Keep the existing per-page operation and memory policy for old-only
    // registries. A page-context profile explicitly opts into the batch limit.
    if registry.pages.iter().all(|p| p.renderer == RENDERER) {
        registry_reference_inputs(root, registry.files, manifest, &raw)?;
        let compiled = super::source::compiled()?;
        for page in &registry.pages {
            let source = bounded(root, &page.source, super::export::MAX_SOURCE_BYTES)?;
            let aliases = bounded(root, &page.aliases, MAX_REGISTRY)?;
            repository::json::validate(std::str::from_utf8(&aliases)?)?;
            let mut budget = super::source::budget();
            let href = super::projection::annotated::pages::relative(
                &page.projection,
                &page.source,
                None,
                &mut budget,
            )
            .map_err(super::source::err)?;
            let expected = host::generate_with_phase_limits(
                &compiled,
                &page.source,
                std::str::from_utf8(&source)?,
                &aliases,
                Some(&href),
                &mut budget,
                phases,
            )?
            .0;
            if bounded(root, &page.projection, 2_097_152)? != expected.as_bytes() {
                return Err(format!(
                    "stale generated Markdown: {} (edit {})",
                    page.projection, page.source
                )
                .into());
            }
        }
        for page in registry.pages {
            println!("Canonical Doc projection is current: {}", page.id);
        }
        return Ok(());
    }
    let generated = projection::generate_from_registry(
        root,
        manifest,
        raw,
        &mut registry.output_limits.budget(),
    )?;
    for (path, expected) in &generated.files {
        let actual = bounded(root, path, 2_097_152)?;
        if actual != expected.as_bytes() {
            return Err(format!(
                "stale generated Markdown: {path} (edit its registered Doc source)"
            )
            .into());
        }
    }
    for (path, _) in &generated.files {
        println!("Canonical Doc projection is current: {path}");
    }
    Ok(())
}

/// Stage the complete checked Markdown set without replacing repository files.
pub fn markdown(root: &Path, manifest: &str, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let registry = parse_registry(&raw)?;
    let generated = projection::generate_from_registry(
        root,
        manifest,
        raw,
        &mut registry.output_limits.budget(),
    )?;
    super::export::pages::write_generated(
        super::export::pages::GeneratedPages {
            provenance: None,
            files: generated
                .files
                .into_iter()
                .map(|(p, s)| (p, s.into_bytes()))
                .collect(),
            manifest: generated.manifest,
        },
        output,
    )
}

/// Opt-in continuation of output attempts after immutable preparation succeeds.
/// Parse/lower/preparation stops remain ordinary failures on this entry.
pub fn markdown_interactive_output(
    root: &Path,
    manifest: &str,
    limits_path: &Path,
    output: &Path,
) -> Result<()> {
    markdown_interactive_output_with_io(
        root,
        manifest,
        limits_path,
        output,
        &mut std::io::stdin().lock(),
        &mut std::io::stderr().lock(),
    )
}
fn markdown_interactive_output_with_io(
    root: &Path,
    manifest: &str,
    limits_path: &Path,
    output: &Path,
    mut input: &mut impl std::io::BufRead,
    messages: &mut impl std::io::Write,
) -> Result<()> {
    use super::export::pages::resources::{job::covers, read_configuration};
    use projection::retry::OutputRetryLimits;
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let selected: OutputRetryLimits = read_configuration(limits_path)?;
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let generated = projection::retry::generate(
        root,
        manifest,
        raw,
        selected,
        |reason, usage, job, phase| {
            writeln!(
                messages,
                "Paused Markdown output: {reason:?}; cumulative render usage={usage:?}"
            )?;
            resume_grant(&mut input, messages, |next: &OutputRetryLimits| {
                covers(next.render_job_limits.budget().limits(), job)
                    && covers(next.output_limits.budget().limits(), phase)
            })
            .map(|next| {
                next.map(|n| {
                    (
                        n.render_job_limits.budget().limits(),
                        n.output_limits.budget().limits(),
                    )
                })
            })
        },
    )?;
    super::export::pages::write_generated(
        super::export::pages::GeneratedPages {
            provenance: None,
            files: generated
                .files
                .into_iter()
                .map(|(p, s)| (p, s.into_bytes()))
                .collect(),
            manifest: generated.manifest,
        },
        output,
    )
}

fn resume_grant<T: serde::de::DeserializeOwned>(
    input: &mut impl std::io::BufRead,
    messages: &mut impl std::io::Write,
    accepts: impl Fn(&T) -> bool,
) -> Result<Option<T>> {
    resume_grant_with_message(
        input,
        messages,
        accepts,
        "Rejected: all selected limits must be nondecreasing.",
    )
}
fn resume_grant_with_message<T: serde::de::DeserializeOwned>(
    mut input: &mut impl std::io::BufRead,
    messages: &mut impl std::io::Write,
    accepts: impl Fn(&T) -> bool,
    rejection: &str,
) -> Result<Option<T>> {
    use std::io::BufRead;
    loop {
        writeln!(messages, "resume <limits.json> or cancel (EOF cancels):")?;
        messages.flush()?;
        let mut line = String::new();
        let count = std::io::Read::take(&mut input, 4097).read_line(&mut line)?;
        if line.len() > 4096 {
            return Err("ResumeCommandLimit".into());
        }
        if count == 0 || line.trim() == "cancel" {
            return Ok(None);
        }
        let Some(path) = line
            .trim()
            .strip_prefix("resume ")
            .filter(|s| !s.trim().is_empty())
        else {
            writeln!(messages, "Expected resume <limits.json> or cancel.")?;
            continue;
        };
        match super::export::pages::resources::read_configuration::<T>(Path::new(path.trim())) {
            Ok(next) if accepts(&next) => return Ok(Some(next)),
            Ok(_) => writeln!(messages, "{rejection}")?,
            Err(error) => writeln!(messages, "Cannot read limits: {error}")?,
        }
    }
}

/// Explicit whole-job retry with complete-page parse/lower checkpoints.
pub fn markdown_interactive(
    root: &Path,
    manifest: &str,
    limits: &Path,
    output: &Path,
) -> Result<()> {
    markdown_interactive_with_io(
        root,
        manifest,
        limits,
        output,
        &mut std::io::stdin().lock(),
        &mut std::io::stderr().lock(),
    )
}
fn markdown_interactive_with_io(
    root: &Path,
    manifest: &str,
    limits: &Path,
    output: &Path,
    input: &mut impl std::io::BufRead,
    messages: &mut impl std::io::Write,
) -> Result<()> {
    use projection::retry::preparation::{self, RetryLimits};
    if output.exists() {
        return Err("output directory already exists".into());
    }
    let selected: RetryLimits = super::export::pages::resources::read_configuration(limits)?;
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let generated = preparation::generate(
        root,
        manifest,
        raw,
        selected,
        |previous, phase, reason, usage| {
            let stage = match phase {
                preparation::Phase::Preparation { page, role } => {
                    format!("preparation page {page} representation {role:?}")
                }
                preparation::Phase::Output => "output".to_owned(),
            };
            writeln!(
                messages,
                "Paused Markdown {stage}: {reason:?}; cumulative job usage={usage:?}"
            )?;
            resume_grant(input, messages, |next: &RetryLimits| next.covers(previous))
        },
    )?;
    super::export::pages::write_generated(
        super::export::pages::GeneratedPages {
            provenance: None,
            files: generated
                .files
                .into_iter()
                .map(|(p, s)| (p, s.into_bytes()))
                .collect(),
            manifest: generated.manifest,
        },
        output,
    )
}
pub fn retry_limits() -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&projection::retry::preparation::RetryLimits::default())?
    );
    Ok(())
}

pub fn html_retry_limits() -> Result<()> {
    continuation::defaults()
}
pub fn html_interactive(root: &Path, manifest: &str, limits: &Path, output: &Path) -> Result<()> {
    continuation::write(root, manifest, limits, output)
}

/// Export the registered Doc sources through the same prepared page-set backend.
/// The existing Markdown path remains the logical namespace for relative links;
/// the physical source and its digest are recorded separately by the generator.
pub fn html(root: &Path, manifest: &str, output: &Path) -> Result<()> {
    if output.exists() {
        return Err("output directory already exists".into());
    }
    super::export::pages::write_generated(generate_html(root, manifest)?, output)
}

/// Generate the complete checked set for host composition before any writes.
pub(crate) fn generate_html(
    root: &Path,
    manifest: &str,
) -> Result<super::export::pages::GeneratedPages> {
    generate_html_with_projections(root, manifest, &[])
}

/// A composing host may replace explicitly registered passive sources with
/// already rendered projections. Standalone export continues to deliver sources.
pub(crate) fn generate_html_with_projections(
    root: &Path,
    manifest: &str,
    projections: &[references::Projection],
) -> Result<super::export::pages::GeneratedPages> {
    let raw = bounded(root, manifest, MAX_REGISTRY)?;
    let registry = parse_registry(&raw)?;
    let phases = registry.phases();
    if registry
        .pages
        .iter()
        .any(|p| projection::is_footnotes(&p.renderer))
    {
        return Err("footnote-only registry requires Markdown export".into());
    }
    let mut output_budget = registry.html_output_limits.budget();
    let (inputs, aliases, mut resources, markdown) = if registry.compatibility.is_empty() {
        let resources = registry_reference_inputs(
            root,
            registry
                .files
                .iter()
                .map(|f| ReferenceFile {
                    id: f.id.clone(),
                    source: f.source.clone(),
                    route: f.route.clone(),
                })
                .collect(),
            manifest,
            &raw,
        )?;
        let mut inputs = Vec::new();
        let mut aliases = Vec::new();
        let mut alias_bytes = 0usize;
        let mut total = 0u64;
        for page in &registry.pages {
            let raw = bounded(root, &page.aliases, MAX_REGISTRY)?;
            alias_bytes = alias_bytes
                .checked_add(raw.len())
                .ok_or("AliasInputLimit")?;
            if alias_bytes > 10_000_000 {
                return Err("AliasInputLimit".into());
            }
            aliases.push(super::export::pages::aliases::PageAliases::parse(
                page.id.clone(),
                raw,
            )?);
            let bytes = bounded(root, &page.source, super::export::MAX_SOURCE_BYTES)?;
            total = total.checked_add(bytes.len() as u64).ok_or("SourceLimit")?;
            if total > super::export::MAX_SOURCE_BYTES {
                return Err("SourceLimit".into());
            }
            let source = String::from_utf8(bytes)?;
            inputs.push((
                super::export::pages::Entry {
                    id: page.id.clone(),
                    source: page.projection.clone(),
                    input: Some(page.source.clone()),
                    route: page.route.clone(),
                },
                source,
            ));
        }
        (inputs, aliases, resources, None)
    } else {
        let prepared = projection::prepare_compatibility(
            root,
            manifest,
            raw,
            &mut registry.output_limits.budget(),
        )?;
        (
            prepared.inputs,
            prepared.aliases,
            prepared.references,
            Some(prepared.markdown),
        )
    };
    // Site-only passive HTML substitutions never contaminate Markdown inputs.
    references::apply(&mut resources, projections)?;
    let mut generated = super::export::pages::generate_with_aliases(
        &super::source::compiled()?,
        &inputs,
        &resources,
        phases,
        &mut output_budget,
        &aliases,
    )?;
    references::record(&mut generated, projections)?;
    if let Some(markdown) = markdown {
        compatibility::append(&registry, markdown, &mut generated)?;
    }
    Ok(generated)
}

/// Export a separately owned registry using the explicit footnote viewing profile.
pub fn footnotes_manifest(manifest: &Path, output: &Path) -> Result<()> {
    let parent = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = manifest
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("invalid registry filename")?;
    let registry = load(parent, name)?;
    if registry
        .pages
        .iter()
        .any(|p| !projection::is_footnotes(&p.renderer))
    {
        return Err("footnotes-pages requires the footnote renderer on every page".into());
    }
    markdown(parent, name, output)
}
