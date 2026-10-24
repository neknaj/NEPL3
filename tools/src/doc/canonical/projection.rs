//! Frozen repository inputs and an explicitly selected page-context projection.
#[path = "receipt.rs"]
mod receipt;
#[path = "retry.rs"]
pub(super) mod retry;
use super::*;
use crate::doc::export::pages::resources::PhaseLimits;
use crate::doc::{
    projection::annotated,
    source::{Compiled, err},
};
use nepl3_core::{
    budget::{Budget, Resource},
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::{
    check::Category,
    pages::{FileBytes, PageDocument, PageFile, PageRegistration, PageSet},
};
use nepl3_wire::foundation::FoundationCodec;

pub(super) const RENDERER: &str = "nepl3-tools.markdown-annotated-pages/4";
pub(super) const FOOTNOTES_RENDERER: &str = "nepl3-tools.markdown-footnotes-pages/1";
pub(super) const COMPACT_RENDERER: &str = "nepl3-tools.markdown-footnotes-compact-pages/1";
pub(super) fn is_footnotes(renderer: &str) -> bool {
    matches!(renderer, FOOTNOTES_RENDERER | COMPACT_RENDERER)
}
const CONTEXT: &[u8] = b"nepl3.canonical-input-context/1\0";
const MAX_ALIASES: u64 = 1_048_576;
const MAX_OUTPUT: u64 = 2_097_152;

struct Input {
    page: Page,
    source: String,
    aliases: Vec<u8>,
}

pub(super) struct Generated {
    pub files: Vec<(String, String)>,
    pub manifest: String,
}

fn charge(budget: &mut Budget, resource: Resource, count: usize) -> Result<()> {
    budget.charge(resource, count as u64).map_err(err)?;
    Ok(())
}

// Length-framed fields; the caller fixes their order. Hashing never incorporates
// generated Markdown, so mutual links cannot create a recursive digest.
fn field(bytes: &mut Vec<u8>, value: &[u8], budget: &mut Budget) -> Result<()> {
    let length = value.len().checked_add(8).ok_or("ContextLimit")?;
    let new_length = bytes.len().checked_add(length).ok_or("ContextLimit")?;
    charge(budget, Resource::Work, length)?;
    if new_length > bytes.capacity() {
        let capacity = bytes
            .capacity()
            .checked_mul(2)
            .ok_or("ContextLimit")?
            .max(new_length);
        charge(
            budget,
            Resource::AllocationUnits,
            capacity - bytes.capacity(),
        )?;
        charge(budget, Resource::Work, bytes.len())?;
        bytes.try_reserve_exact(capacity - bytes.len())?;
    }
    bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
    bytes.extend_from_slice(value);
    Ok(())
}

fn hex(value: Digest) -> String {
    value.0.iter().map(|b| format!("{b:02x}")).collect()
}

fn page_context(
    input: &Input,
    dependencies: &[annotated::pages::LinkDependency],
    images: &[annotated::pages::ImageDependency],
    math: &[annotated::pages::MathDependency],
    budget: &mut Budget,
) -> Result<Digest> {
    let mut bytes = Vec::new();
    field(&mut bytes, b"nepl3.canonical-page-context/1\0", budget)?;
    let p = &input.page;
    for value in [&p.renderer, &p.id, &p.source, &p.projection, &p.aliases] {
        field(&mut bytes, value.as_bytes(), budget)?;
    }
    for value in [input.source.as_bytes(), input.aliases.as_slice()] {
        charge(budget, Resource::Work, value.len())?;
        field(&mut bytes, &Digest::of(value).0, budget)?;
    }
    field(
        &mut bytes,
        &(dependencies.len() as u64).to_be_bytes(),
        budget,
    )?;
    for dependency in dependencies {
        field(&mut bytes, &dependency.node.to_be_bytes(), budget)?;
        for value in [
            dependency.target_kind,
            &dependency.target_id,
            &dependency.route,
        ] {
            field(&mut bytes, value.as_bytes(), budget)?;
        }
        field(
            &mut bytes,
            &[u8::from(dependency.fragment.is_some())],
            budget,
        )?;
        if let Some(fragment) = &dependency.fragment {
            field(&mut bytes, fragment.as_bytes(), budget)?;
        }
    }
    if !images.is_empty() {
        field(&mut bytes, b"static-svg/1\0", budget)?;
        field(&mut bytes, &(images.len() as u64).to_be_bytes(), budget)?;
        for image in images {
            field(&mut bytes, &image.node.to_be_bytes(), budget)?;
            field(&mut bytes, image.asset_id.as_bytes(), budget)?;
            field(&mut bytes, image.route.as_bytes(), budget)?;
            field(&mut bytes, &image.digest.0, budget)?;
        }
    }
    if !math.is_empty() {
        field(&mut bytes, b"structural-math-tex/1\0", budget)?;
        field(&mut bytes, &(math.len() as u64).to_be_bytes(), budget)?;
        for dependency in math {
            field(&mut bytes, &dependency.embed.to_be_bytes(), budget)?;
            field(&mut bytes, &dependency.guest_digest.0, budget)?;
        }
    }
    charge(budget, Resource::Work, bytes.len())?;
    Ok(Digest::of(&bytes))
}

type ReferenceInputs = Vec<(super::super::export::pages::Entry, Vec<u8>)>;

struct Captured {
    raw: Vec<u8>,
    inputs: Vec<Input>,
    references: ReferenceInputs,
    needs_group: bool,
    phases: PhaseLimits,
}

fn capture(root: &Path, manifest: &str, raw: Vec<u8>) -> Result<Captured> {
    let registry = parse_registry(&raw)?;
    let phases = registry.phases();
    let grouped = registry.pages.iter().any(|p| {
        matches!(
            p.renderer.as_str(),
            RENDERER | FOOTNOTES_RENDERER | COMPACT_RENDERER
        )
    });
    let references = registry_reference_inputs(root, registry.files, manifest, &raw)?;
    if registry.pages.len() > 128 {
        return Err("PageCountLimit".into());
    }
    let mut inputs = Vec::new();
    let mut sources = 0u64;
    let mut aliases_total = 0u64;
    for page in registry.pages {
        let source = bounded(root, &page.source, super::super::export::MAX_SOURCE_BYTES)?;
        let aliases = bounded(root, &page.aliases, MAX_ALIASES)?;
        sources = sources
            .checked_add(source.len() as u64)
            .ok_or("SourceLimit")?;
        aliases_total = aliases_total
            .checked_add(aliases.len() as u64)
            .ok_or("AliasLimit")?;
        if sources > super::super::export::MAX_SOURCE_BYTES || aliases_total > MAX_ALIASES {
            return Err("canonical aggregate source/alias limit".into());
        }
        repository::json::validate(std::str::from_utf8(&aliases)?)?;
        inputs.push(Input {
            page,
            source: String::from_utf8(source)?,
            aliases,
        });
    }
    Ok(Captured {
        raw,
        inputs,
        references,
        needs_group: grouped,
        phases,
    })
}

fn prepare_group(
    compiled: &Compiled,
    inputs: &[Input],
    references: ReferenceInputs,
    phases: PhaseLimits,
    budget: &mut Budget,
) -> Result<PreparedGroup> {
    let footnotes = inputs
        .iter()
        .all(|input| is_footnotes(&input.page.renderer));
    let svg = !references.is_empty() && footnotes;
    charge(
        budget,
        Resource::AllocationUnits,
        inputs.len()
            * (core::mem::size_of::<PageDocument>()
                + core::mem::size_of::<Vec<annotated::Alias>>()),
    )?;
    let mut pages = Vec::with_capacity(inputs.len());
    let mut aliases = Vec::with_capacity(inputs.len());
    for input in inputs {
        budget.poll().map_err(err)?;
        let page = &input.page;
        let document = crate::doc::source::with_named_input_limits(
            true,
            compiled,
            &input.source,
            &page.id,
            "Article",
            phases.parse,
            |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let mut lower_budget = Budget::new(phases.lower);
                nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &compiled.doc.package.schema,
                    Category::Article,
                    profile.registry(),
                    &mut lower_budget,
                    &mut codec,
                )
                .map_err(|e| format!("lower: {e:?}; usage={:?}", lower_budget.usage()))
            },
        )
        .map_err(|e| format!("canonical page {}: {e}", page.source))?;
        let registration_bytes = page.id.len() + page.projection.len() * 2;
        charge(budget, Resource::Work, registration_bytes)?;
        charge(
            budget,
            Resource::AllocationUnits,
            registration_bytes + core::mem::size_of::<PageDocument>(),
        )?;
        pages.push(PageDocument {
            registration: PageRegistration {
                id: page.id.clone(),
                source: page.projection.clone(),
                route: page.projection.clone(),
            },
            document,
        });
        charge(budget, Resource::Work, input.aliases.len())?;
        // A conservative host JSON allocation allowance, separate from the
        // renderer's own accounting for semantic aliases and output strings.
        charge(budget, Resource::AllocationUnits, input.aliases.len() * 32)?;
        aliases.push(serde_json::from_slice::<Vec<annotated::Alias>>(
            &input.aliases,
        )?);
    }
    let mut files = Vec::new();
    for (entry, bytes) in references {
        charge(budget, Resource::Work, bytes.len())?;
        charge(
            budget,
            Resource::AllocationUnits,
            core::mem::size_of::<PageFile>() + entry.source.len(),
        )?;
        files.push(PageFile {
            registration: PageRegistration {
                id: entry.id,
                source: entry.source.clone(),
                route: if svg { entry.route } else { entry.source },
            },
            content: FileBytes(bytes),
        });
    }
    let set = PageSet { pages, files };
    Ok(PreparedGroup {
        set,
        aliases,
        footnotes,
        svg,
    })
}

struct PreparedGroup {
    set: PageSet,
    aliases: Vec<Vec<annotated::Alias>>,
    footnotes: bool,
    svg: bool,
}

fn render_group(
    compiled: &Compiled,
    inputs: &[Input],
    prepared: &PreparedGroup,
    budget: &mut Budget,
) -> Result<annotated::pages::PagesArtifact> {
    let PreparedGroup {
        set,
        aliases,
        footnotes,
        ..
    } = prepared;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(&compiled.doc.registry, &store, &mut admission).map_err(err)?;
    charge(
        budget,
        Resource::AllocationUnits,
        aliases.len() * core::mem::size_of::<&[annotated::Alias]>(),
    )?;
    let refs: Vec<_> = aliases.iter().map(Vec::as_slice).collect();
    let styles = if inputs.iter().any(|v| is_footnotes(&v.page.renderer)) {
        charge(budget, Resource::Work, inputs.len())?;
        charge(
            budget,
            Resource::AllocationUnits,
            inputs.len() * core::mem::size_of::<annotated::NotesMode>(),
        )?;
        Some(
            inputs
                .iter()
                .map(|v| {
                    if is_footnotes(&v.page.renderer) {
                        annotated::NotesMode::Footnotes
                    } else {
                        annotated::NotesMode::Inline
                    }
                })
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };
    let output = if *footnotes {
        let math = compiled
            .others
            .iter()
            .find(|p| p.schema.package == "standard.math")
            .ok_or("missing standard Math surface")?;
        let sentence = compiled
            .others
            .iter()
            .find(|p| p.schema.package == "nepl3.syntax.sentence")
            .ok_or("missing standard Sentence surface")?;
        let render = if inputs
            .iter()
            .all(|input| input.page.renderer == COMPACT_RENDERER)
        {
            annotated::pages::render_compact_footnotes_svg_math
        } else {
            annotated::pages::render_footnotes_svg_math
        };
        render(
            set,
            &compiled.doc.registry,
            &mut codec,
            budget,
            &refs,
            annotated::pages::MathSurfaces {
                math: &math.schema,
                sentence: Some(&sentence.schema),
                doc: Some(&compiled.doc.package.schema),
            },
        )
    } else {
        annotated::pages::render_styles(
            set,
            &compiled.doc.registry,
            &mut codec,
            budget,
            &refs,
            styles.as_deref(),
        )
    }
    .map_err(|e| format!("Markdown page set: {e:?}; usage={:?}", budget.usage()))?;
    Ok(output)
}

#[cfg(test)]
pub(super) fn generate(root: &Path, manifest: &str, budget: &mut Budget) -> Result<Generated> {
    budget.poll().map_err(err)?;
    generate_from_registry(
        root,
        manifest,
        bounded(root, manifest, MAX_REGISTRY)?,
        budget,
    )
}

pub(super) fn generate_from_registry(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    budget: &mut Budget,
) -> Result<Generated> {
    generate_batch(root, manifest, raw, budget).map_err(|e| {
        if budget.poll().is_err() {
            format!("canonical Markdown batch: {e}; usage={:?}", budget.usage()).into()
        } else {
            e
        }
    })
}

fn generate_batch(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    budget: &mut Budget,
) -> Result<Generated> {
    let initial_usage = budget.usage();
    let batch = prepare_batch(root, manifest, raw, budget)?;
    batch.render(initial_usage, budget)
}

pub(super) struct HtmlRetryPrepared {
    pub inputs: Vec<(super::super::export::pages::Entry, String)>,
    pub aliases: Vec<(String, Vec<u8>)>,
    pub references: ReferenceInputs,
    pub markdown: Option<Generated>,
}

/// Capture once for both output families. Only compatibility Markdown uses the
/// supplied finite phase settings; HTML has its separate retained job settings.
pub(super) fn prepare_html_retry(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    phases: PhaseLimits,
    compatibility: bool,
    budget: &mut Budget,
) -> Result<HtmlRetryPrepared> {
    let mut captured =
        capture(root, manifest, raw).map_err(|error| format!("SnapshotPreparation: {error}"))?;
    captured.phases = phases;
    if compatibility {
        let ready = compatibility_from_captured(captured, manifest, budget)
            .map_err(|error| format!("CompatibilityPreparation: {error}"))?;
        return Ok(HtmlRetryPrepared {
            inputs: ready.inputs,
            aliases: ready.aliases.into_iter().map(|a| (a.page, a.raw)).collect(),
            references: ready.references,
            markdown: Some(ready.markdown),
        });
    }
    let mut inputs = Vec::new();
    let mut aliases = Vec::new();
    for input in captured.inputs {
        aliases.push((input.page.id.clone(), input.aliases));
        inputs.push((
            super::super::export::pages::Entry {
                id: input.page.id,
                source: input.page.projection,
                input: Some(input.page.source),
                route: input.page.route,
            },
            input.source,
        ));
    }
    Ok(HtmlRetryPrepared {
        inputs,
        aliases,
        references: captured.references,
        markdown: None,
    })
}

pub(super) struct CompatibilityPrepared {
    pub inputs: Vec<(super::super::export::pages::Entry, String)>,
    pub aliases: Vec<super::super::export::pages::aliases::PageAliases>,
    pub references: ReferenceInputs,
    pub markdown: Generated,
}

/// One bounded immutable source snapshot feeds both projections. Generated
/// repository Markdown is never read as an input or adopted as an expectation.
pub(super) fn prepare_compatibility(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    budget: &mut Budget,
) -> Result<CompatibilityPrepared> {
    budget.poll().map_err(err)?;
    compatibility_from_captured(capture(root, manifest, raw)?, manifest, budget)
}

fn compatibility_from_captured(
    captured: Captured,
    manifest: &str,
    budget: &mut Budget,
) -> Result<CompatibilityPrepared> {
    let snapshot: Vec<_> = captured
        .inputs
        .iter()
        .map(|input| {
            serde_json::json!({
                "page": input.page.id, "source": input.page.source,
                "source_sha256": references::hash(input.source.as_bytes()),
                "aliases": input.page.aliases, "alias_sha256": references::hash(&input.aliases),
            })
        })
        .collect();
    let mut inputs = Vec::new();
    let mut aliases = Vec::new();
    for input in &captured.inputs {
        inputs.push((
            super::super::export::pages::Entry {
                id: input.page.id.clone(),
                source: input.page.projection.clone(),
                input: Some(input.page.source.clone()),
                route: input.page.route.clone(),
            },
            input.source.clone(),
        ));
        aliases.push(super::super::export::pages::aliases::PageAliases::parse(
            input.page.id.clone(),
            input.aliases.clone(),
        )?);
    }
    // capture bounds the source corpus to MAX_SOURCE_BYTES, aliases to 1 MiB,
    // passive data to 2 MiB and pages/files to 128 each. These host snapshot
    // copies are separate from each renderer's runtime Budget and receipts.
    let references = captured
        .references
        .iter()
        .map(|(entry, bytes)| {
            (
                super::super::export::pages::Entry {
                    id: entry.id.clone(),
                    source: entry.source.clone(),
                    route: entry.route.clone(),
                    input: entry.input.clone(),
                },
                bytes.clone(),
            )
        })
        .collect();
    let initial = budget.usage();
    let mut markdown = prepare_captured(captured, manifest, budget)?.render(initial, budget)?;
    if markdown.manifest.len() > 8 * 1024 * 1024 {
        return Err("CompatibilityReceiptLimit".into());
    }
    let mut receipt: serde_json::Value = serde_json::from_str(&markdown.manifest)?;
    receipt["compatibility_snapshot"] = snapshot.into();
    markdown.manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    if markdown.manifest.len() > 8 * 1024 * 1024 {
        return Err("CompatibilityReceiptLimit".into());
    }
    Ok(CompatibilityPrepared {
        inputs,
        aliases,
        references,
        markdown,
    })
}

fn prepare_batch(
    root: &Path,
    manifest: &str,
    raw: Vec<u8>,
    budget: &mut Budget,
) -> Result<PreparedBatch> {
    budget.poll().map_err(err)?;
    charge(budget, Resource::Work, 1)?;
    prepare_captured(capture(root, manifest, raw)?, manifest, budget)
}

fn prepare_captured(
    captured: Captured,
    manifest: &str,
    budget: &mut Budget,
) -> Result<PreparedBatch> {
    let Captured {
        raw,
        inputs,
        references,
        needs_group,
        phases,
    } = captured;
    let compiled = crate::doc::source::compiled()?;
    let prepared = if needs_group {
        Some(prepare_group(
            &compiled, &inputs, references, phases, budget,
        )?)
    } else {
        None
    };
    charge(budget, Resource::Work, manifest.len())?;
    charge(budget, Resource::AllocationUnits, manifest.len())?;
    Ok(PreparedBatch {
        compiled,
        manifest: manifest.to_owned(),
        raw,
        inputs,
        phases,
        prepared,
    })
}

/// Owns immutable inputs and lowered documents, never a CheckedPages proof.
/// Every render invocation performs fresh PageSet resolution in its own budget.
struct PreparedBatch {
    compiled: Compiled,
    manifest: String,
    raw: Vec<u8>,
    inputs: Vec<Input>,
    phases: PhaseLimits,
    prepared: Option<PreparedGroup>,
}
impl PreparedBatch {
    fn render(
        &self,
        initial_usage: nepl3_core::budget::Usage,
        budget: &mut Budget,
    ) -> Result<Generated> {
        let mut group = self
            .prepared
            .as_ref()
            .map(|p| render_group(&self.compiled, &self.inputs, p, budget))
            .transpose()?;
        let svg_files = self
            .prepared
            .as_ref()
            .filter(|p| p.svg)
            .map_or(&[][..], |p| p.set.files.as_slice());
        self.finish(&mut group, svg_files, initial_usage, budget)
    }
}

impl PreparedBatch {
    fn finish(
        &self,
        group: &mut Option<annotated::pages::PagesArtifact>,
        svg_files: &[PageFile],
        initial_usage: nepl3_core::budget::Usage,
        budget: &mut Budget,
    ) -> Result<Generated> {
        let Self {
            compiled,
            manifest,
            raw,
            inputs,
            phases,
            ..
        } = self;
        let phases = *phases;
        if let Some(group) = group.as_mut() {
            for (artifact, input) in group.pages.iter_mut().zip(inputs) {
                if is_footnotes(&input.page.renderer) {
                    continue;
                }
                let href = annotated::pages::relative(
                    &input.page.projection,
                    &input.page.source,
                    None,
                    budget,
                )
                .map_err(err)?;
                artifact.source_link(&href, budget).map_err(err)?;
            }
        }
        let mut context = Vec::new();
        let identity = if let Some(group) = &group {
            field(&mut context, CONTEXT, budget)?;
            field(&mut context, RENDERER.as_bytes(), budget)?;
            field(&mut context, manifest.as_bytes(), budget)?;
            field(&mut context, raw, budget)?;
            field(&mut context, &group.identity.0, budget)?;
            field(&mut context, &(inputs.len() as u64).to_be_bytes(), budget)?;
            for input in inputs {
                let p = &input.page;
                for value in [
                    &p.id,
                    &p.source,
                    &p.projection,
                    &p.aliases,
                    &p.route,
                    &p.renderer,
                ] {
                    field(&mut context, value.as_bytes(), budget)?;
                }
                for bytes in [input.source.as_bytes(), &input.aliases] {
                    charge(budget, Resource::Work, bytes.len())?;
                    field(&mut context, &Digest::of(bytes).0, budget)?;
                }
            }
            charge(budget, Resource::Work, context.len())?;
            Some(Digest::of(&context))
        } else {
            None
        };
        let file_capacity = inputs
            .len()
            .checked_add(svg_files.len())
            .ok_or("OutputLimit")?;
        let has_svg = !svg_files.is_empty();
        charge(
            budget,
            Resource::AllocationUnits,
            file_capacity * core::mem::size_of::<(String, String)>()
                + inputs.len() * core::mem::size_of::<Digest>(),
        )?;
        let mut files = Vec::with_capacity(file_capacity);
        let mut documents = Vec::with_capacity(inputs.len());
        for (index, input) in inputs.iter().enumerate() {
            budget.poll().map_err(err)?;
            let page = &input.page;
            let (text, document_digest) = if page.renderer == host::RENDERER {
                let href = annotated::pages::relative(&page.projection, &page.source, None, budget)
                    .map_err(err)?;
                let (legacy, document_digest) = host::generate_with_phase_limits(
                    compiled,
                    &page.source,
                    &input.source,
                    &input.aliases,
                    Some(&href),
                    budget,
                    phases,
                )?;
                if let Some(group) = &group {
                    charge(
                        budget,
                        Resource::Work,
                        legacy.len() + group.pages[index].markdown.len(),
                    )?;
                    let body = legacy
                        .split_once("\n\n")
                        .ok_or("legacy metadata missing")?
                        .1;
                    if body.trim_end_matches('\n')
                        != group.pages[index].markdown.trim_end_matches('\n')
                    {
                        return Err(
                            "legacy projection differs from resolved page body/anchors".into()
                        );
                    }
                }
                (legacy, document_digest)
            } else {
                let group = group.as_ref().ok_or("missing page context")?;
                let context = page_context(
                    input,
                    &group.dependencies[index],
                    &group.image_dependencies[index],
                    &group.math_dependencies[index],
                    budget,
                )?;
                let artifact = &group.pages[index];
                // Paths passed portable ASCII validation. Escaping hyphens prevents
                // a filename from closing the comment; none contains a newline.
                let reserve =
                    (page.source.len() * 5 + page.id.len() * 5 + 1024 + artifact.markdown.len())
                        * 8;
                charge(
                    budget,
                    Resource::Work,
                    input.source.len() + input.aliases.len(),
                )?;
                charge(budget, Resource::Work, reserve)?;
                charge(budget, Resource::AllocationUnits, reserve)?;
                (
                    format!(
                        "<!-- Generated from {}; renderer {}; page {}; source SHA-256 {}; alias input SHA-256 {}; page input SHA-256 {}. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->\n\n{}\n",
                        page.source.replace('-', "&#45;"),
                        page.renderer,
                        page.id.replace('-', "&#45;"),
                        hex(Digest::of(input.source.as_bytes())),
                        hex(Digest::of(&input.aliases)),
                        hex(context),
                        artifact.markdown.trim_end_matches('\n')
                    ),
                    artifact.document_digest,
                )
            };
            if text.len() as u64 > MAX_OUTPUT {
                return Err("canonical per-page OutputLimit".into());
            }
            charge(budget, Resource::OutputBytes, text.len())?;
            charge(budget, Resource::Work, page.projection.len())?;
            charge(
                budget,
                Resource::AllocationUnits,
                page.projection.len() + core::mem::size_of::<(String, String)>(),
            )?;
            files.push((page.projection.clone(), text));
            documents.push(document_digest);
        }
        charge(
            budget,
            Resource::AllocationUnits,
            file_capacity * core::mem::size_of::<serde_json::Value>(),
        )?;
        let mut records = Vec::with_capacity(file_capacity);
        for ((path, text), document_digest) in files.iter().zip(documents) {
            charge(budget, Resource::Work, text.len() + path.len())?;
            charge(budget, Resource::AllocationUnits, path.len() * 6 + 512)?;
            records.push(serde_json::json!({"path":path,"sha256":hex(Digest::of(text.as_bytes())),"bytes":text.len(),"document_digest":hex(document_digest)}));
        }
        for file in svg_files {
            let copy_bytes = file.content.0.len() + file.registration.route.len();
            charge(budget, Resource::Work, copy_bytes)?;
            charge(budget, Resource::AllocationUnits, copy_bytes)?;
            let path = file.registration.route.clone();
            let text = std::str::from_utf8(&file.content.0)?.to_owned();
            charge(budget, Resource::Work, text.len() + path.len())?;
            charge(budget, Resource::OutputBytes, text.len())?;
            charge(budget, Resource::AllocationUnits, path.len() * 6 + 512)?;
            records.push(serde_json::json!({"path":path,"sha256":hex(Digest::of(text.as_bytes())),"bytes":text.len(),"kind":"static-svg"}));
            files.push((path, text));
        }
        let manifest = receipt::render(
            inputs,
            group.as_ref(),
            has_svg,
            identity,
            &records,
            initial_usage,
            phases,
            budget,
        )?;
        Ok(Generated { files, manifest })
    }
}

#[cfg(test)]
mod retained_tests {
    use super::*;
    use crate::testing::Fixture;
    use nepl3_core::budget::{Limits, StopReason, Usage};
    #[test]
    fn prepared_inputs_survive_output_stop_without_rereading_files() -> Result<()> {
        let f = Fixture::new()?;
        f.write(
            "page.nepld",
            r#"article en "Frozen" body cons paragraph cons "{Before/note}" nil nil"#,
        )?;
        f.write("aliases.json", "[]")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page", "source":"page.nepld", "projection":"page.md", "route":"page.md",
            "aliases":"aliases.json", "renderer":FOOTNOTES_RENDERER
        }]}))?;
        let batch = prepare_batch(
            f.root(),
            "pages.json",
            raw,
            &mut crate::doc::source::budget(),
        )?;
        let expected = batch.render(Usage::default(), &mut crate::doc::source::budget())?;
        f.write("page.nepld", "invalid changed source")?;
        f.write("aliases.json", "invalid changed aliases")?;
        let mut short = Budget::new(Limits {
            work: 0,
            ..crate::doc::source::budget().limits()
        });
        assert!(batch.render(Usage::default(), &mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::WorkLimit));
        // A stopped Budget remains stopped; retaining inputs is not permission
        // to revive it or to carry its checked-admission proof into a new one.
        assert!(batch.render(Usage::default(), &mut short).is_err());
        let actual = batch.render(Usage::default(), &mut crate::doc::source::budget())?;
        assert_eq!(expected.files, actual.files);
        assert_eq!(expected.manifest, actual.manifest);
        assert!(actual.files[0].1.contains("Before"));
        assert!(actual.files[0].1.contains("[^"));
        Ok(())
    }
    #[test]
    fn compact_profile_preserves_body_and_rejects_mixed_or_html() -> Result<()> {
        let f = Fixture::new()?;
        f.write("page.nepld",r#"article en "Compact" body cons paragraph cons "{Before/note}" nil cons paragraph cons sentence cons link relative "page.md" none text "Self" nil nil cons display Math frac 3 7 cons image asset "figure" none "Image" none nil"#)?;
        f.write("aliases.json", r#"[{"section":null,"name":"entry"}]"#)?;
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><path d="M0 0 L10 10"/></svg>"#;
        f.write("image.svg", svg)?;
        let manifest = |renderer: &str| {
            serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
                "id":"page","source":"page.nepld","projection":"page.md","route":"page.md","aliases":"aliases.json","renderer":renderer
            }],"files":[{"id":"figure","source":"image.svg","route":"assets/image.svg"}]}))
        };
        let legacy = prepare_batch(
            f.root(),
            "pages.json",
            manifest(FOOTNOTES_RENDERER)?,
            &mut crate::doc::source::budget(),
        )?;
        let compact = prepare_batch(
            f.root(),
            "pages.json",
            manifest(COMPACT_RENDERER)?,
            &mut crate::doc::source::budget(),
        )?;
        let old = legacy.render(Usage::default(), &mut crate::doc::source::budget())?;
        let new = compact.render(Usage::default(), &mut crate::doc::source::budget())?;
        assert_eq!(
            old.files[0].1.lines().skip(1).collect::<Vec<_>>(),
            new.files[0].1.lines().skip(1).collect::<Vec<_>>()
        );
        assert!(
            new.files
                .iter()
                .any(|(path, text)| path == "assets/image.svg" && text == svg)
        );
        assert!(new.files[0].1.contains("entry"));
        assert!(new.files[0].1.contains("Self"));
        let old_receipt: serde_json::Value = serde_json::from_str(&old.manifest)?;
        let new_receipt: serde_json::Value = serde_json::from_str(&new.manifest)?;
        let group = compact.prepared.as_ref().ok_or("prepared group")?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&compact.compiled.doc.registry, &store, &mut admission)
                .map_err(err)?;
        let checked = nepl3_doc_core::pages::compact::resolve(
            &group.set,
            &compact.compiled.doc.registry,
            &mut codec,
            &mut crate::doc::source::budget(),
        )
        .map_err(err)?;
        assert_eq!(new_receipt["input_pageset"], hex(checked.plan().identity));
        assert_eq!(
            new_receipt["files"][0]["document_digest"],
            hex(checked.document_digest(0).ok_or("document digest")?)
        );
        let guest = checked
            .plan()
            .remaining
            .iter()
            .find_map(|requirement| match &requirement.requirement {
                nepl3_doc_core::prepare::DocRequirement::Foreign { guest_digest, .. } => {
                    Some(*guest_digest)
                }
                _ => None,
            })
            .ok_or("Math commitment")?;
        assert_eq!(
            new_receipt["page_dependencies"][0]["math"][0]["guest_sha256"],
            hex(guest)
        );
        assert_ne!(old_receipt["input_pageset"], new_receipt["input_pageset"]);
        assert_ne!(
            old_receipt["files"][0]["document_digest"],
            new_receipt["files"][0]["document_digest"]
        );
        assert_ne!(
            old_receipt["page_dependencies"][0]["math"],
            new_receipt["page_dependencies"][0]["math"]
        );
        f.write(
            "pages.json",
            String::from_utf8(manifest(COMPACT_RENDERER)?)?,
        )?;
        assert!(super::super::generate_html(f.root(), "pages.json").is_err());
        let mut mixed: serde_json::Value = serde_json::from_slice(&manifest(COMPACT_RENDERER)?)?;
        let mut second = mixed["pages"][0].clone();
        second["id"] = "second".into();
        second["source"] = "second.nepld".into();
        second["projection"] = "second.md".into();
        second["route"] = "second.md".into();
        second["renderer"] = FOOTNOTES_RENDERER.into();
        mixed["pages"].as_array_mut().ok_or("pages")?.push(second);
        let error = parse_registry(&serde_json::to_vec(&mixed)?)
            .err()
            .ok_or("mixed profile accepted")?;
        assert!(
            error
                .to_string()
                .contains("compact page identity cannot be mixed")
        );
        mixed["pages"][0]["renderer"] = FOOTNOTES_RENDERER.into();
        parse_registry(&serde_json::to_vec(&mixed)?)?;
        Ok(())
    }
    #[test]
    fn compact_inputs_survive_output_stop_without_rereading_files() -> Result<()> {
        let f = Fixture::new()?;
        f.write(
            "page.nepld",
            r#"article en "Frozen" body cons paragraph cons "{Before/note}" nil nil"#,
        )?;
        f.write("aliases.json", "[]")?;
        let raw = serde_json::to_vec(&serde_json::json!({"version":1,"pages":[{
            "id":"page", "source":"page.nepld", "projection":"page.md", "route":"page.md",
            "aliases":"aliases.json", "renderer":COMPACT_RENDERER
        }]}))?;
        let batch = prepare_batch(
            f.root(),
            "pages.json",
            raw,
            &mut crate::doc::source::budget(),
        )?;
        let expected = batch.render(Usage::default(), &mut crate::doc::source::budget())?;
        let mut measured = crate::doc::source::budget();
        batch.render(Usage::default(), &mut measured)?;
        let u = measured.usage();
        let mut limits = Limits {
            source_bytes: u.source_bytes,
            work: u.work,
            depth: u.depth,
            nodes: u.nodes,
            allocation_units: u.allocation_units,
            output_bytes: u.output_bytes,
            diagnostics: u.diagnostics,
            events: u.events,
        };
        // Receipts serialize the requested limits: reducing digit widths can
        // reduce their own metered size. Find a stable exact boundary first.
        loop {
            let mut exact = Budget::new(limits);
            batch.render(Usage::default(), &mut exact)?;
            let u = exact.usage();
            let next = Limits {
                source_bytes: u.source_bytes,
                work: u.work,
                depth: u.depth,
                nodes: u.nodes,
                allocation_units: u.allocation_units,
                output_bytes: u.output_bytes,
                diagnostics: u.diagnostics,
                events: u.events,
            };
            if next == limits {
                break;
            }
            assert!(
                next.source_bytes <= limits.source_bytes
                    && next.work <= limits.work
                    && next.depth <= limits.depth
                    && next.nodes <= limits.nodes
                    && next.allocation_units <= limits.allocation_units
                    && next.output_bytes <= limits.output_bytes
            );
            limits = next;
        }

        for (resource, reason) in [
            (0, StopReason::SourceLimit),
            (1, StopReason::WorkLimit),
            (2, StopReason::DepthLimit),
            (3, StopReason::NodeLimit),
            (4, StopReason::AllocationLimit),
            (5, StopReason::OutputLimit),
        ] {
            let mut short = limits;
            let limit = match resource {
                0 => &mut short.source_bytes,
                1 => &mut short.work,
                2 => &mut short.depth,
                3 => &mut short.nodes,
                4 => &mut short.allocation_units,
                _ => &mut short.output_bytes,
            };
            if *limit == 0 {
                continue;
            }
            *limit -= 1;
            let mut budget = Budget::new(short);
            assert!(
                batch.render(Usage::default(), &mut budget).is_err(),
                "resource {resource}"
            );
            assert_eq!(budget.poll(), Err(reason));
            let usage = budget.usage();
            assert!(batch.render(Usage::default(), &mut budget).is_err());
            assert_eq!(budget.usage(), usage);
        }
        f.write("page.nepld", "invalid changed source")?;
        f.write("aliases.json", "invalid changed aliases")?;
        let mut short = Budget::new(Limits {
            work: 0,
            ..crate::doc::source::budget().limits()
        });
        assert!(batch.render(Usage::default(), &mut short).is_err());
        assert_eq!(short.poll(), Err(StopReason::WorkLimit));
        // A stopped Budget remains stopped; retaining inputs is not permission
        // to revive it or to carry its checked-admission proof into a new one.
        assert!(batch.render(Usage::default(), &mut short).is_err());
        let actual = batch.render(Usage::default(), &mut crate::doc::source::budget())?;
        assert_eq!(expected.files, actual.files);
        assert_eq!(expected.manifest, actual.manifest);
        assert!(actual.files[0].1.contains("Before"));
        assert!(actual.files[0].1.contains("[^"));
        Ok(())
    }
    #[test]
    fn active_registry_reference_is_frozen_after_capture() -> Result<()> {
        let f = crate::testing::Fixture::new()?;
        let raw = serde_json::to_vec(&serde_json::json!({
            "version":1,
            "pages":[{"id":"sample","source":"doc/sample.nepld","projection":"doc/sample.md","aliases":"doc/aliases.json","route":"docs/sample.html","renderer":RENDERER}],
            "files":[{"id":"registry","source":"doc/canonical.json","route":"sources/canonical.json"}],
            "compatibility":[{"page":"sample","route":"sources/sample.md"}]
        }))?;
        f.write("doc/canonical.json", &raw)?;
        f.write("doc/sample.nepld", r#"article en "Before" body cons paragraph cons sentence cons link relative "canonical.json" none text "Registry" nil nil nil"#)?;
        f.write("doc/aliases.json", "[]")?;
        let captured = capture(f.root(), "doc/canonical.json", raw.clone())?;
        f.write("doc/canonical.json", "invalid replacement")?;
        let prepared = compatibility_from_captured(
            captured,
            "doc/canonical.json",
            &mut crate::doc::source::budget(),
        )?;
        assert_eq!(prepared.references[0].1, raw);
        assert!(prepared.markdown.files[0].1.contains("Before"));
        Ok(())
    }

    #[test]
    fn compatibility_snapshot_survives_repository_mutation() -> Result<()> {
        let f = crate::testing::Fixture::new()?;
        let raw = serde_json::to_vec(
            &serde_json::json!({"version":1,"pages":[{"id":"sample","source":"doc/sample.nepld","projection":"doc/sample.md","aliases":"doc/aliases.json","route":"docs/sample.html","renderer":RENDERER}],"files":[{"id":"guide","source":"doc/guide.md","route":"sources/guide.md"}],"compatibility":[{"page":"sample","route":"sources/sample.md"}]}),
        )?;
        f.write(
            "doc/sample.nepld",
            "article en \"Before\" body cons paragraph cons \"Frozen body.\" nil nil",
        )?;
        f.write("doc/aliases.json", "[]")?;
        f.write("doc/guide.md", "Frozen passive")?;
        let captured = capture(f.root(), "doc/canonical.json", raw)?;
        f.write("doc/guide.md", "Changed passive")?;
        f.write("doc/sample.nepld", "invalid replacement")?;
        f.write("doc/aliases.json", "invalid replacement")?;
        let prepared = compatibility_from_captured(
            captured,
            "doc/canonical.json",
            &mut crate::doc::source::budget(),
        )?;
        assert!(prepared.inputs[0].1.contains("Frozen body"));
        assert_eq!(prepared.aliases[0].raw, b"[]");
        assert_eq!(prepared.references[0].1, b"Frozen passive");
        assert!(prepared.markdown.files[0].1.contains(r"Frozen body\."));
        assert!(!prepared.markdown.files[0].1.contains("invalid replacement"));
        Ok(())
    }
}
