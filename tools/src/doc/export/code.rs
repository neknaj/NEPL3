//! Host presentation of already parsed Code guests. No guest parser, lowerer,
//! evaluator or LSP server is invoked here.
use super::super::source::{budget, err};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceAdmission, SourceStore},
    view::FallbackRole,
};
use nepl3_doc_core::model::{DocEmbed, EmbedKind};
use nepl3_engine::{
    analysis::{
        BindingOptions,
        region::{self, RegionOutcome, RegionRequest, highlight},
    },
    portable,
    profile::ResolvedParseProfile,
    tree::ValidatedParseTree,
};
use nepl3_markup::html::*;
use nepl3_wire::foundation::FoundationCodec;

fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), String> {
    b.charge(Resource::Work, 1).map_err(err)?;
    b.charge(
        Resource::AllocationUnits,
        (2 * std::mem::size_of::<T>()) as u64,
    )
    .map_err(err)?;
    items.push(item);
    Ok(())
}
fn owned(text: &str, b: &mut Budget) -> Result<String, String> {
    b.charge(Resource::Work, text.len() as u64).map_err(err)?;
    b.charge(Resource::AllocationUnits, text.len() as u64)
        .map_err(err)?;
    Ok(text.into())
}
fn add(nodes: &mut Vec<HtmlNode>, node: HtmlNode, b: &mut Budget) -> Result<u64, String> {
    b.charge(Resource::Nodes, 1).map_err(err)?;
    let index = nodes.len() as u64;
    push(nodes, node, b)?;
    Ok(index)
}
fn class(role: FallbackRole) -> &'static str {
    match role {
        FallbackRole::Content => "nepl-code-content",
        FallbackRole::Marker => "nepl-code-marker",
        FallbackRole::Delimiter => "nepl-code-delimiter",
        FallbackRole::Name => "nepl-code-name",
        FallbackRole::Quantity => "nepl-code-quantity",
        FallbackRole::Annotation => "nepl-code-annotation",
    }
}
fn text(
    nodes: &mut Vec<HtmlNode>,
    children: &mut Vec<u64>,
    value: &str,
    b: &mut Budget,
) -> Result<(), String> {
    if !value.is_empty() {
        let value = owned(value, b)?;
        let id = add(nodes, HtmlNode::Text { text: value }, b)?;
        push(children, id, b)?;
    }
    Ok(())
}
/// The tree and closure are produced in one immutable parse/lower invocation by
/// generate_impl. The callback cannot accept unrelated external RegionReply data.
pub(super) fn render(
    embed: &DocEmbed,
    tree: &ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    b: &mut Budget,
) -> Result<HtmlRequest, String> {
    b.poll().map_err(err)?;
    if embed.kind != EmbedKind::Code {
        return Err("CodeKind".into());
    }
    let bundle = &embed.closure.syntax.bundle;
    let cover = bundle
        .node(bundle.root)
        .map_err(err)?
        .cover
        .as_ref()
        .ok_or("CodeSourceUnavailable")?;
    let mut found = None;
    for candidate in &bundle.sources {
        b.charge(
            Resource::Work,
            (candidate.identity().source.0.len() as u64)
                .saturating_add(cover.snapshot_ref().source.0.len() as u64)
                .saturating_add(64),
        )
        .map_err(err)?;
        if candidate.identity() == cover.snapshot_ref() {
            found = Some(candidate);
            break;
        }
    }
    let source = found.ok_or("CodeSourceMissing")?;
    source.slice(cover).map_err(err)?;
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
    let binding = portable::analysis::prepare(
        "doc-code",
        tree.tree(),
        BindingOptions,
        budget().limits(),
        profile,
        &mut codec,
        b,
    )
    .map_err(err)?;
    let input = portable::region::prepare(&binding, None, &mut codec, b).map_err(err)?;
    let key = input.key();
    let identity_cost = (source.identity().source.0.len() as u64).saturating_add(64);
    b.charge(Resource::Work, identity_cost).map_err(err)?;
    b.charge(Resource::AllocationUnits, identity_cost)
        .map_err(err)?;
    let request = RegionRequest {
        key,
        source: source.reference(),
        offset: cover.start(),
    };
    let reply = region::regions(&input, &request, b, &mut SourceAdmission::default());
    let spans = highlight::normalize(&reply, &key, source, b).map_err(err)?;
    let RegionOutcome::Complete { regions, .. } = &reply.outcome else {
        return Err("CodeAnalysisIncomplete".into());
    };
    let mut nodes = Vec::new();
    let mut children = Vec::new();
    let mut classes = Vec::new();
    let mut cursor = cover.start();
    for span in spans {
        b.charge(Resource::Work, 1).map_err(err)?;
        let start = span.byte_start.max(cover.start());
        let end = span.byte_end.min(cover.end());
        if start >= end {
            continue;
        }
        let gap = source
            .text()
            .get(cursor as usize..start as usize)
            .ok_or("CodeRange")?;
        text(&mut nodes, &mut children, gap, b)?;
        let presentation = &regions[span.region as usize].classes[span.class as usize];
        let category = class(presentation.fallback);
        b.charge(
            Resource::Work,
            (classes.len() as u64).saturating_mul(category.len() as u64 + 1),
        )
        .map_err(err)?;
        if !classes.iter().any(|v| v == category) {
            push(&mut classes, owned(category, b)?, b)?;
        }
        let mut attributes = Vec::new();
        let mut values = Vec::new();
        push(&mut values, owned(category, b)?, b)?;
        push(&mut attributes, HtmlAttribute::Class { values }, b)?;
        // Keep the complete schema-qualified class identity in escaped data,
        // separate from the fixed, safe stylesheet category key.
        let size = presentation
            .schema
            .package
            .len()
            .checked_add(presentation.name.len())
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(100))
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| err(b.stop(StopReason::AllocationLimit)))?;
        b.charge(Resource::Work, size as u64).map_err(err)?;
        b.charge(Resource::AllocationUnits, size as u64)
            .map_err(err)?;
        let mut identity = String::with_capacity(size);
        identity.push_str("pc-");
        let hex = |out: &mut String, bytes: &[u8]| {
            for byte in bytes {
                out.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
                out.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
            }
        };
        hex(&mut identity, presentation.schema.package.as_bytes());
        identity.push('-');
        hex(&mut identity, &presentation.schema.revision.to_be_bytes());
        identity.push('-');
        hex(&mut identity, &presentation.schema.digest.0);
        identity.push('-');
        hex(&mut identity, presentation.name.as_bytes());
        push(
            &mut attributes,
            HtmlAttribute::DataId { value: identity },
            b,
        )?;
        let mut inner = Vec::new();
        let value = source
            .text()
            .get(start as usize..end as usize)
            .ok_or("CodeRange")?;
        text(&mut nodes, &mut inner, value, b)?;
        let id = add(
            &mut nodes,
            HtmlNode::Element {
                tag: HtmlTag::Span,
                attributes,
                children: inner,
            },
            b,
        )?;
        push(&mut children, id, b)?;
        cursor = end;
    }
    text(
        &mut nodes,
        &mut children,
        source
            .text()
            .get(cursor as usize..cover.end() as usize)
            .ok_or("CodeRange")?,
        b,
    )?;
    let root = add(
        &mut nodes,
        HtmlNode::Element {
            tag: HtmlTag::Span,
            attributes: Vec::new(),
            children,
        },
        b,
    )?;
    Ok(HtmlRequest {
        fragment: HtmlFragment { root, nodes },
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::source;
    #[test]
    fn missing_source_stops_and_callback_errors_remain_explicit() -> Result<(), String> {
        let compiled = source::compiled()?;
        let text = "article en \"Host\" body cons paragraph cons code Doc article en \"Guest\" body nil nil nil";
        source::with_input(&compiled, text, "Article", |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let doc = nepl3_doc_core::lower::document(
                tree.syntax(),
                &compiled.doc.package.schema,
                nepl3_doc_core::check::Category::Article,
                profile.registry(),
                &mut budget(),
                &mut codec,
            )
            .map_err(err)?;
            let embed = doc.value.embeds.first().ok_or("missing embed")?;
            let mut cancelled = budget();
            cancelled.cancel();
            assert_eq!(
                render(embed, tree, profile, &mut cancelled),
                Err("Cancelled".into())
            );
            let mut limits = budget().limits();
            limits.work = 0;
            assert_eq!(
                render(embed, tree, profile, &mut Budget::new(limits)),
                Err("WorkLimit".into())
            );
            let mut absent = embed.clone();
            let root = absent.closure.syntax.bundle.root;
            absent.closure.syntax.bundle.nodes[root.0 as usize].cover = None;
            assert_eq!(
                render(&absent, tree, profile, &mut budget()),
                Err("CodeSourceUnavailable".into())
            );
            let options = nepl3_doc_html::RenderOptions {
                parallel: nepl3_doc_html::ParallelMode::Rows,
            };
            let mut stopped = budget();
            stopped.cancel();
            assert!(matches!(
                nepl3_doc_html::code::prepare_code(
                    &doc,
                    &options,
                    profile.registry(),
                    &mut codec,
                    &mut stopped
                ),
                Err(nepl3_doc_html::LocalPreparationError::Stopped(
                    StopReason::Cancelled
                ))
            ));
            let prepared = nepl3_doc_html::code::prepare_code(
                &doc,
                &options,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let failure = nepl3_doc_html::code::render_code(
                &prepared,
                &mut |_, _, _| Err("explicit adapter error"),
                &mut budget(),
            );
            assert!(matches!(
                failure,
                Err(nepl3_doc_html::ForeignRenderError::Foreign(
                    "explicit adapter error"
                ))
            ));
            Ok(())
        })
    }
}
