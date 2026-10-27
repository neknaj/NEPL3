//! Standalone Body Markdown fragment. No page registration, metadata, source
//! link boundary or legacy Article capability is created by this operation.
use super::*;

/// A fragment with authored headings only. Unlike Article artifacts, this has
/// no title boundary and cannot receive canonical Article provenance insertion.
#[derive(Debug)]
pub struct BodyArtifact {
    pub markdown: String,
    pub document_digest: Digest,
}

/// Render one locally resolved Body with ordered Anno footnotes. Top-level
/// Sections become peer H1 headings; no title or language is synthesized.
/// Local References may target Sections, whose emitted anchors are checked.
/// Inline Anchor nodes remain unsupported, even if no Reference targets them.
/// Empty Body yields an empty fragment. External URIs are checked; unresolved
/// page links, assets and foreign operations (including hidden variants) fail.
/// This entry accepts no compatibility aliases or selected foreign fragments.
pub fn render_footnotes<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<BodyArtifact, Error>
where
    C::Error: core::fmt::Debug,
{
    let plan = prepare::inspect_body(document, registry, codec, budget).map_err(|e| match e {
        prepare::PreparationError::Stopped(s) => Error::Stopped(s),
        e => Error::Invalid(format!("{e:?}")),
    })?;
    for requirement in &plan.requirements {
        check_pending(requirement, budget)?;
    }
    let DocRoot::Body(root) = document.value.root else {
        return Err(Error::Unsupported { node: 0 });
    };
    // Only Section destinations have emitted anchors in this fragment profile.
    // Local label validity alone does not prove output-anchor availability.
    let mut links = Vec::new();
    for (node, entry) in document.value.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if let DocKind::Reference { target, .. } = &entry.kind {
            let mut section = false;
            for definition in &document.value.nodes {
                budget.charge(Resource::Work, target.len() as u64 + 1)?;
                if matches!(&definition.kind, DocKind::Section { id, .. } if id == target) {
                    section = true;
                }
            }
            if !section {
                return Err(Error::Unsupported { node: node as u64 });
            }
            let capacity = target
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(3))
                .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
            budget.charge(Resource::Work, capacity as u64)?;
            budget.charge(Resource::AllocationUnits, capacity as u64)?;
            let mut href = String::new();
            href.try_reserve_exact(capacity)
                .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
            href.push_str("#n-");
            for byte in target.bytes() {
                href.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
                href.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
            }
            push(&mut links, (node as u64, href), budget)?;
        }
    }
    let mut writer = Annotated {
        plain: Writer {
            doc: document,
            budget,
            output: String::new(),
        },
        aliases: &[],
        links: &links,
        images: &[],
        math: &[],
        emitted_math: Vec::new(),
        emitted: Vec::new(),
        mode: NotesMode::Footnotes,
        footnotes: Vec::new(),
    };
    writer.body(root.0, 0)?;
    writer.finish_footnotes()?;
    for (_, href) in &links {
        let mut found = false;
        for anchor in &writer.emitted {
            writer
                .plain
                .budget
                .charge(Resource::Work, href.len() as u64 + 1)?;
            found |= href.strip_prefix('#') == Some(anchor.as_str());
        }
        if !found {
            return Err(Error::Invalid("Body reference anchor not emitted".into()));
        }
    }
    Ok(BodyArtifact {
        markdown: writer.plain.output,
        document_digest: plan.document_digest,
    })
}
