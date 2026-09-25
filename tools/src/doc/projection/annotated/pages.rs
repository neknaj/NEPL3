//! Explicit Markdown page-set projection. Routes name Markdown destinations,
//! never inferred HTML routes. Passive files have bytes but no semantic labels.
use super::*;
use crate::doc::export::pages::discovery;
use nepl3_doc_core::pages::{PageDestination, PageSet, namespace as domain};
use nepl3_sentence_core::lower::ForeignInlineForm;

#[derive(Debug)]
pub struct PagesArtifact {
    /// The checked page-namespace identity, including selected guest members
    /// and passive file bytes, excluding aliases and renderer settings.
    /// This is not a digest of the final distribution artifact.
    pub identity: Digest,
    pub pages: Vec<Artifact>,
    /// Link interfaces used by each page, in source traversal order. Target
    /// contents are not embedded by this viewing profile.
    pub dependencies: Vec<Vec<LinkDependency>>,
}

#[derive(Debug, serde::Serialize)]
pub struct LinkDependency {
    pub member: u64,
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
    render_observed(set, registry, codec, budget, aliases, &mut |_| {})
}

/// Observe the cumulative usage after discovery and complete namespace
/// resolution. The callback receives no proof or mutable budget; rendering
/// continues in the same operation and retains all selected owners.
pub fn render_observed<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
    prepared: &mut impl FnMut(Usage),
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    render_profiled(
        set,
        registry,
        codec,
        budget,
        aliases,
        &mut |stage, usage| {
            if stage == Stage::Resolution {
                prepared(usage);
            }
        },
    )
}

/// Completed host stages. Usage is cumulative in one unchanged operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Discovery,
    Selection,
    Resolution,
    Projection,
}

/// Observe completed stages without exposing mutable state or validation proofs.
/// The host may measure elapsed time alongside each cumulative usage snapshot.
pub fn render_profiled<C: FoundationValueCodec>(
    set: &PageSet,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[&[Alias]],
    observe: &mut impl FnMut(Stage, Usage),
) -> Result<PagesArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    budget.poll()?;
    if aliases.len() != set.pages.len() {
        return Err(Error::Invalid("page alias count mismatch".into()));
    }
    let sentence = registry
        .selected("nepl3.syntax.sentence", 1)
        .ok_or_else(|| Error::Invalid("missing Sentence surface".into()))?;
    let doc = registry
        .selected("standard.doc", 1)
        .ok_or_else(|| Error::Invalid("missing Doc surface".into()))?;
    let mut forms = Vec::new();
    push(
        &mut forms,
        ForeignInlineForm {
            kind: "Form:DocumentInline",
            guest_schema: doc,
            guest_category: "Inline",
        },
        budget,
    )?;
    if let Some(math) = registry.selected("standard.math", 1) {
        push(
            &mut forms,
            ForeignInlineForm {
                kind: "Form:InlineMath",
                guest_schema: math,
                guest_category: "Expr",
            },
            budget,
        )?;
    }
    let mut discovered = Vec::new();
    for page in &set.pages {
        let value = discovery::collect(
            &page.document,
            sentence,
            doc,
            &forms,
            registry,
            codec,
            budget,
        );
        budget.poll()?;
        push(
            &mut discovered,
            value.map_err(|e| Error::Invalid(format!("{e:?}")))?,
            budget,
        )?;
    }
    observe(Stage::Discovery, budget.usage());
    let mut plans = Vec::new();
    let mut selected = Vec::new();
    for page in &discovered {
        let occurrences = discovery::namespace::select_occurrences(page, budget)?;
        let base = page.members()[0].depth();
        let mut documents = Vec::new();
        for occurrence in &occurrences {
            let owner = &page.members()[occurrence.document.index()];
            push(
                &mut documents,
                domain::NamespaceDocument {
                    document: owner.document(),
                    relative_depth: owner.depth().saturating_sub(base),
                },
                budget,
            )?;
        }
        push(&mut plans, occurrences, budget)?;
        push(&mut selected, documents, budget)?;
    }
    observe(Stage::Selection, budget.usage());
    let mut refs = Vec::new();
    for documents in &selected {
        push(&mut refs, documents.as_slice(), budget)?;
    }
    let value = domain::with_resolved(set, &refs, registry, codec, budget, |checked, _, budget| {
        observe(Stage::Resolution, budget.usage());
        let artifact = render_checked(set, &discovered, &plans, checked, budget, aliases)?;
        observe(Stage::Projection, budget.usage());
        Ok(artifact)
    });
    budget.poll()?;
    value.map_err(|error| match error {
        domain::ScopedError::Stopped(reason) => Error::Stopped(reason),
        domain::ScopedError::Output(error) => error,
        domain::ScopedError::Preparation(domain::Error::Namespace { error, .. }) => {
            Error::Invalid(format!("{error:?}"))
        }
        domain::ScopedError::Preparation(error) => Error::Invalid(format!("{error:?}")),
    })
}

fn render_checked(
    set: &PageSet,
    discovered: &[discovery::Collected<'_>],
    plans: &[Vec<discovery::namespace::Occurrence>],
    checked: &domain::CheckedPageNamespaces<'_, '_, '_>,
    budget: &mut Budget,
    aliases: &[&[Alias]],
) -> Result<PagesArtifact, Error> {
    let mut output = Vec::new();
    let mut dependencies = Vec::new();
    let mut pending = checked.members().iter().peekable();
    for (page, input) in set.pages.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        let mut context = composition::Context::new(&discovered[page], page as u64, budget)?;
        let mut used = Vec::new();
        let mut routed = Vec::new();
        for _ in discovered[page].members() {
            push(&mut routed, false, budget)?;
        }
        let mut document_digest = None;
        while let Some(plan) = pending.peek() {
            budget.charge(Resource::Work, 1)?;
            if plan.owner().page != page as u64 {
                break;
            }
            let member = plan.owner().member.0;
            let owner = plans[page][member as usize].document.index();
            if member == 0 {
                document_digest = Some(plan.document_digest());
            }
            for requirement in plan.remaining() {
                check_pending(requirement, budget)?;
            }
            for link in plan.links() {
                budget.charge(Resource::Work, 1)?;
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
                    member,
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
                if !routed[owner] {
                    let href = relative(
                        &input.registration.route,
                        target,
                        link.fragment.as_deref(),
                        budget,
                    )?;
                    context.links[owner][link.node as usize] = Some(href);
                }
            }
            routed[owner] = true;
            let _ = pending.next();
        }
        let document_digest =
            document_digest.ok_or_else(|| Error::Invalid("missing root member".into()))?;
        let contents = Contents::borrowed(discovered[page].members()[0].sentences());
        let rendered = render_resolved(
            &input.document,
            &contents,
            budget,
            aliases[page],
            Some(&context),
            document_digest,
        )?;
        push(&mut output, rendered, budget)?;
        push(&mut dependencies, used, budget)?;
    }
    if pending.next().is_some() {
        return Err(Error::Invalid("checked page plan order mismatch".into()));
    }
    // Semantic label existence is insufficient: the selected viewing profile
    // must actually emit the destination anchor (not an unreachable arena node).
    for link in checked.members().iter().flat_map(|m| m.links()) {
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
        identity: checked.identity(),
        pages,
        dependencies,
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
