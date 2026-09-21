//! Frozen repository inputs and an explicitly selected page-context projection.
use super::*;
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
    lower,
    pages::{FileBytes, PageDocument, PageFile, PageRegistration, PageSet},
};
use nepl3_wire::foundation::FoundationCodec;

pub(super) const RENDERER: &str = "nepl3-tools.markdown-annotated-pages/4";
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
    charge(budget, Resource::Work, bytes.len())?;
    Ok(Digest::of(&bytes))
}

type ReferenceInputs = Vec<(super::super::export::pages::Entry, Vec<u8>)>;

fn capture(root: &Path, raw: Vec<u8>) -> Result<(Vec<u8>, Vec<Input>, ReferenceInputs, bool)> {
    let registry = parse_registry(&raw)?;
    let grouped = registry.pages.iter().any(|p| p.renderer == RENDERER);
    let references = reference_inputs(root, registry.files)?;
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
    Ok((raw, inputs, references, grouped))
}

fn grouped(
    compiled: &Compiled,
    inputs: &[Input],
    references: ReferenceInputs,
    budget: &mut Budget,
) -> Result<annotated::pages::PagesArtifact> {
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
            crate::doc::source::budget().limits(),
            |tree, profile, _, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let mut lower_budget = crate::doc::source::budget();
                lower::document(
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
                route: entry.source,
            },
            content: FileBytes(bytes),
        });
    }
    let set = PageSet { pages, files };
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
    Ok(
        annotated::pages::render(&set, &compiled.doc.registry, &mut codec, budget, &refs)
            .map_err(|e| format!("Markdown page set: {e:?}; usage={:?}", budget.usage()))?,
    )
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
    budget.poll().map_err(err)?;
    let initial_usage = budget.usage();
    let limits = budget.limits();
    charge(budget, Resource::Work, 1)?;
    let (raw, inputs, references, needs_group) = capture(root, raw)?;
    let compiled = crate::doc::source::compiled()?;
    let mut group = if needs_group {
        Some(grouped(&compiled, &inputs, references, budget)?)
    } else {
        None
    };
    if let Some(group) = &mut group {
        for (artifact, input) in group.pages.iter_mut().zip(&inputs) {
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
        field(&mut context, &raw, budget)?;
        field(&mut context, &group.identity.0, budget)?;
        field(&mut context, &(inputs.len() as u64).to_be_bytes(), budget)?;
        for input in &inputs {
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
    charge(
        budget,
        Resource::AllocationUnits,
        inputs.len() * (core::mem::size_of::<(String, String)>() + core::mem::size_of::<Digest>()),
    )?;
    let mut files = Vec::with_capacity(inputs.len());
    let mut documents = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        budget.poll().map_err(err)?;
        let page = &input.page;
        let (text, document_digest) = if page.renderer == host::RENDERER {
            let href = annotated::pages::relative(&page.projection, &page.source, None, budget)
                .map_err(err)?;
            let (legacy, document_digest) = host::generate_with_receipt(
                &compiled,
                &page.source,
                &input.source,
                &input.aliases,
                Some(&href),
                budget,
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
                if body.trim_end_matches('\n') != group.pages[index].markdown.trim_end_matches('\n')
                {
                    return Err("legacy projection differs from resolved page body/anchors".into());
                }
            }
            (legacy, document_digest)
        } else {
            let group = group.as_ref().ok_or("missing page context")?;
            let context = page_context(input, &group.dependencies[index], budget)?;
            let artifact = &group.pages[index];
            // Paths passed portable ASCII validation. Escaping hyphens prevents
            // a filename from closing the comment; none contains a newline.
            let reserve =
                (page.source.len() * 5 + page.id.len() * 5 + 1024 + artifact.markdown.len()) * 8;
            charge(
                budget,
                Resource::Work,
                input.source.len() + input.aliases.len(),
            )?;
            charge(budget, Resource::Work, reserve)?;
            charge(budget, Resource::AllocationUnits, reserve)?;
            (
                format!(
                    "<!-- Generated from {}; renderer {RENDERER}; page {}; source SHA-256 {}; alias input SHA-256 {}; page input SHA-256 {}. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->\n\n{}\n",
                    page.source.replace('-', "&#45;"),
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
        files.len() * core::mem::size_of::<serde_json::Value>(),
    )?;
    let mut records = Vec::with_capacity(files.len());
    for ((path, text), document_digest) in files.iter().zip(documents) {
        charge(budget, Resource::Work, text.len() + path.len())?;
        charge(budget, Resource::AllocationUnits, path.len() * 6 + 512)?;
        records.push(serde_json::json!({"path":path,"sha256":hex(Digest::of(text.as_bytes())),"bytes":text.len(),"document_digest":hex(document_digest)}));
    }
    // Reserve before json! copies the records and before serialization. Each
    // portable path is <=4096 bytes; 32 KiB covers escaped path, fields, digest
    // and pretty-print spacing. Both copies are included in this allowance.
    let dependency_allowance = group.as_ref().map_or(Ok(0usize), |g| {
        g.dependencies
            .iter()
            .flatten()
            .try_fold(0usize, |total, link| {
                let size = link
                    .target_id
                    .len()
                    .checked_add(link.route.len())
                    .and_then(|n| n.checked_add(link.fragment.as_ref().map_or(0, String::len)))
                    .and_then(|n| n.checked_mul(6))
                    .and_then(|n| n.checked_add(512))
                    .ok_or("ContextLimit")?;
                total.checked_add(size).ok_or("ContextLimit")
            })
    })?;
    let receipt_allowance = (files.len() * 32768 + 1024)
        .checked_add(dependency_allowance)
        .and_then(|n| n.checked_mul(2))
        .ok_or("ContextLimit")?;
    charge(budget, Resource::Work, receipt_allowance)?;
    charge(budget, Resource::AllocationUnits, receipt_allowance)?;
    // This final receipt is not an input to the next generation.
    let usage = |u: nepl3_core::budget::Usage| {
        serde_json::json!({
        "source_bytes":u.source_bytes,"work":u.work,"depth":u.depth,"nodes":u.nodes,
        "allocation_units":u.allocation_units,"output_bytes":u.output_bytes,
        "diagnostics":u.diagnostics,"events":u.events})
    };
    let receipt = serde_json::json!({"format":"nepl3.canonical-markdown-stage/2",
        "input_context":identity.map(hex),"input_pageset":group.as_ref().map(|g| hex(g.identity)),
        "page_dependencies":group.as_ref().map(|g| inputs.iter().zip(&g.dependencies).map(|(input, dependencies)|
            serde_json::json!({"path":input.page.projection,"links":dependencies})).collect::<Vec<_>>()),
        "files":records,
        "output_budget":{"limits":{"source_bytes":limits.source_bytes,"work":limits.work,
            "depth":limits.depth,"nodes":limits.nodes,"allocation_units":limits.allocation_units,
            "output_bytes":limits.output_bytes,"diagnostics":limits.diagnostics,"events":limits.events},
            "initial_usage":usage(initial_usage),"usage_before_receipt_serialization":usage(budget.usage())}});
    let manifest = serde_json::to_string_pretty(&receipt)? + "\n";
    charge(budget, Resource::Work, manifest.len())?;
    charge(budget, Resource::OutputBytes, manifest.len())?;
    Ok(Generated { files, manifest })
}
