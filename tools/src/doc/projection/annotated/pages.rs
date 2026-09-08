//! Explicit Markdown page-set projection. Routes name Markdown destinations,
//! never inferred HTML routes. Passive files have bytes but no semantic labels.
use super::*;
use nepl3_doc_core::pages::{self as domain, PageDestination, PageSet};

#[derive(Debug)]
pub struct PagesArtifact {
    /// The input PageSet identity, excluding aliases and renderer settings.
    /// This is not a digest of the final distribution artifact.
    pub identity: Digest,
    pub pages: Vec<Artifact>,
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
    budget.poll()?;
    if aliases.len() != set.pages.len() {
        return Err(Error::Invalid("page alias count mismatch".into()));
    }
    let checked = domain::resolve(set, registry, codec, budget).map_err(|e| match e {
        domain::PageError::Stopped(s) => Error::Stopped(s),
        e => Error::Invalid(format!("{e:?}")),
    })?;
    let mut output = Vec::new();
    for (page, input) in set.pages.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let mut links = Vec::new();
        for link in &checked.plan().links {
            budget.charge(Resource::Work, 1)?;
            if link.page != page as u64 {
                continue;
            }
            let target = match link.target {
                PageDestination::Page { index } => &set.pages[index as usize].registration.route,
                PageDestination::File { index } => &set.files[index as usize].registration.route,
            };
            let href = relative(
                &input.registration.route,
                target,
                link.fragment.as_deref(),
                budget,
            )?;
            push(&mut links, (link.node, href), budget)?;
        }
        let rendered = render_resolved(
            &input.document,
            registry,
            codec,
            budget,
            aliases[page],
            &links,
        )?;
        push(&mut output, rendered, budget)?;
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
    })
}

// Both routes have already passed PageSet's portable ASCII path validation.
// No filesystem normalization, URL guessing or platform path separator is used.
fn relative(
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
