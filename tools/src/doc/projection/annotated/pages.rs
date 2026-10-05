//! Explicit Markdown page-set projection. Routes name Markdown destinations,
//! never inferred HTML routes. Passive files have bytes but no semantic labels.
pub use super::images::ImageDependency;
use super::*;
use nepl3_doc_core::pages::{self as domain, PageDestination, PageSet};

#[derive(Debug)]
pub struct PagesArtifact {
    /// The input PageSet identity, excluding aliases and renderer settings.
    /// This is not a digest of the final distribution artifact.
    pub identity: Digest,
    pub pages: Vec<Artifact>,
    /// Link interfaces used by each page, in source traversal order. Target
    /// contents are not embedded by this viewing profile.
    pub dependencies: Vec<Vec<LinkDependency>>,
    pub image_dependencies: Vec<Vec<ImageDependency>>,
}

#[derive(Debug, serde::Serialize)]
pub struct LinkDependency {
    pub node: u64,
    pub target_kind: &'static str,
    pub target_id: String,
    pub route: String,
    pub fragment: Option<String>,
}

fn owned(value: &str, budget: &mut Budget) -> Result<String, Error> {
    budget.charge(Resource::Work, value.len() as u64)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(value.into())
}

/// Resolve the exact immutable set and render every page before returning any
/// output. A caller-supplied serialized link plan is not an admission proof.
/// Aliases are ordered like `set.pages`; passive files reject all fragments.
pub fn render<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    render_styles(set, registry, codec, budget, aliases, None)
}
pub(crate) fn render_styles<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
    styles: Option<&[NotesMode]>,
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    render_inner(set, registry, codec, budget, aliases, styles, false)
}

/// Explicit SVG image profile. Every file is validated and must be used as an
/// image; legacy passive-file rendering remains a separate operation.
pub fn render_footnotes_svg<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    render_inner(set, registry, codec, budget, aliases, None, true)
}

fn render_inner<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
    styles: Option<&[NotesMode]>,
    svg: bool,
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    if styles.is_some_and(|v| v.len() != set.pages.len()) {
        return Err(Error::Invalid("page style count mismatch".into()));
    }
    budget.poll()?;
    if aliases.len() != set.pages.len() {
        return Err(Error::Invalid("page alias count mismatch".into()));
    }
    // Bound image inputs before the portable PageSet admission can clone or
    // hash their byte arrays. Validation does not perform external I/O.
    let mut files = if svg {
        Some(images::Files::validate(set, budget)?)
    } else {
        None
    };
    let checked = domain::resolve(set, registry, codec, budget).map_err(|e| match e {
        domain::PageError::Stopped(s) => Error::Stopped(s),
        e => Error::Invalid(format!("{e:?}")),
    })?;
    let mut output = Vec::new();
    let mut dependencies = Vec::new();
    let mut image_dependencies = Vec::new();
    // CheckedPages is constructed by resolve in source-page order. Consume
    // each requirement/link once instead of filtering the whole set per page.
    let mut pending = checked.plan().remaining.as_slice();
    let mut page_links = checked.plan().links.as_slice();
    for (page, input) in set.pages.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let prepared = if svg {
            Some(
                nepl3_doc_core::text::prepare(&input.document, registry, codec, budget).map_err(
                    |error| match error {
                        nepl3_doc_core::portable::PortableError::Stopped(reason) => {
                            Error::Stopped(reason)
                        }
                        error => Error::Invalid(format!("{error:?}")),
                    },
                )?,
            )
        } else {
            None
        };
        let mut page_images = Vec::new();
        let mut used_images = Vec::new();
        while let Some((requirement, rest)) = pending.split_first() {
            budget.charge(Resource::Work, 1)?;
            if requirement.page != page as u64 {
                break;
            }
            if let (Some(files), Some(prepared), prepare::DocRequirement::Asset { node, asset }) =
                (&mut files, &prepared, &requirement.requirement)
            {
                let (image, dependency) = files.resolve(
                    &input.document,
                    prepared,
                    &input.registration.route,
                    *node,
                    asset,
                    budget,
                )?;
                push(&mut page_images, image, budget)?;
                push(&mut used_images, dependency, budget)?;
            } else {
                check_pending(&requirement.requirement, budget)?;
            }
            pending = rest;
        }
        let document_digest = checked
            .document_digest(page as u64)
            .ok_or_else(|| Error::Invalid("missing checked document digest".into()))?;
        let mut links = Vec::new();
        let mut used = Vec::new();
        while let Some((link, rest)) = page_links.split_first() {
            budget.charge(Resource::Work, 1)?;
            if link.page != page as u64 {
                break;
            }
            let (target_kind, registration) = match link.target {
                PageDestination::Page { index } => {
                    ("page", &set.pages[index as usize].registration)
                }
                PageDestination::File { index } => {
                    ("file", &set.files[index as usize].registration)
                }
            };
            let target = &registration.route;
            let dependency = LinkDependency {
                node: link.node,
                target_kind,
                target_id: owned(&registration.id, budget)?,
                route: owned(target, budget)?,
                fragment: link
                    .fragment
                    .as_deref()
                    .map(|v| owned(v, budget))
                    .transpose()?,
            };
            push(&mut used, dependency, budget)?;
            let href = relative(
                &input.registration.route,
                target,
                link.fragment.as_deref(),
                budget,
            )?;
            push(&mut links, (link.node, href), budget)?;
            page_links = rest;
        }
        let rendered = render_resolved_profile(
            &input.document,
            budget,
            aliases[page],
            &links,
            &page_images,
            document_digest,
            if svg {
                NotesMode::Footnotes
            } else {
                styles.map_or(NotesMode::Inline, |v| v[page])
            },
        )?;
        push(&mut output, rendered, budget)?;
        push(&mut dependencies, used, budget)?;
        push(&mut image_dependencies, used_images, budget)?;
    }
    if !pending.is_empty() || !page_links.is_empty() {
        return Err(Error::Invalid("checked page plan order mismatch".into()));
    }
    if let Some(files) = files {
        files.finish(budget)?;
    }
    // Semantic label existence is insufficient: the selected viewing profile
    // must actually emit the destination anchor (not an unreachable arena node).
    for link in &checked.plan().links {
        budget.charge(Resource::Work, 1)?;
        if let (PageDestination::Page { index }, Some(fragment)) =
            (link.target, link.fragment.as_deref())
        {
            let name = relative("x", "", Some(fragment), budget)?;
            let mut found = false;
            for emitted in &output[index as usize].1 {
                budget.charge(Resource::Work, (name.len() + emitted.len()) as u64 + 1)?;
                found |= name.strip_prefix('#') == Some(emitted.as_str());
            }
            if !found {
                return Err(Error::Invalid("page fragment not emitted".into()));
            }
        }
    }
    let mut pages = Vec::new();
    for (artifact, _) in output {
        push(&mut pages, artifact, budget)?;
    }
    Ok(PagesArtifact {
        identity: checked.plan().identity,
        pages,
        dependencies,
        image_dependencies,
    })
}

// Both routes have already passed PageSet's portable ASCII path validation.
// No filesystem normalization, URL guessing or platform path separator is used.
pub(crate) fn relative(
    source: &str,
    target: &str,
    fragment: Option<&str>,
    budget: &mut Budget,
) -> Result<String, Error> {
    let directory = source.rsplit_once('/').map_or("", |(d, _)| d);
    budget.charge(Resource::Work, (source.len() + target.len()) as u64 + 1)?;
    let mut common = 0;
    for (a, b) in directory.split('/').zip(target.split('/')) {
        if a.is_empty() || a != b {
            break;
        }
        common += a.len() + 1;
    }
    let rest = if common > directory.len() {
        ""
    } else {
        &directory[common..]
    };
    let parents = if rest.is_empty() {
        0
    } else {
        rest.split('/').count()
    };
    let suffix = &target[common..];
    let size = parents
        .checked_mul(3)
        .and_then(|n| n.checked_add(suffix.len()))
        .and_then(|n| {
            fragment.map_or(Some(n), |f| {
                f.len()
                    .checked_mul(2)
                    .and_then(|m| m.checked_add(3))
                    .and_then(|m| n.checked_add(m))
            })
        })
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
    budget.charge(Resource::Work, size as u64)?;
    budget.charge(Resource::AllocationUnits, size as u64)?;
    let mut href = String::with_capacity(size);
    for _ in 0..parents {
        href.push_str("../");
    }
    href.push_str(suffix);
    if let Some(id) = fragment {
        href.push_str("#n-");
        for byte in id.bytes() {
            href.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
            href.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
        }
    }
    Ok(href)
}
