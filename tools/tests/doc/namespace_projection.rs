//! Measure the public composition operations with typed HTML observations.
use super::*;
use nepl3_doc_core::{labels::namespace as labels, pages::namespace as scopes};
use nepl3_doc_html::pages::{PagesHtmlRequest, namespace as html};
use nepl3_markup::html::{HtmlAttribute, HtmlHref, HtmlNode};
use nepl3_sentence_core::lower::ForeignInlineForm;
use nepl3_tools::doc::export::pages::{composition, discovery};

pub(super) struct Work {
    pub composition: u64,
    pub output_validation: u64,
}

pub(super) fn html_links(
    c: &Compiled,
    request: &PagesHtmlRequest,
    pool: &[nepl3_doc_core::pages::PageDocument],
) -> Result<(Vec<Vec<HtmlHref>>, Work), String> {
    let registry = &c.doc.registry;
    let surface = registry
        .selected("nepl3.syntax.sentence", 1)
        .ok_or("Sentence")?;
    let forms = [ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &c.doc.package.schema,
        guest_category: "Inline",
    }];
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut admission).map_err(err)?;
    let mut b = budget();
    let collected = request
        .set
        .pages
        .iter()
        .map(|page| {
            discovery::collect(
                &page.document,
                surface,
                &c.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                &mut b,
            )
            .map_err(err)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut admission = SourceAdmission::default();
    let plans = collected
        .iter()
        .map(|page| {
            discovery::namespace::inspect(page, registry, &mut b, &mut admission).map_err(err)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let members = plans
        .iter()
        .map(|plan| plan.member_refs(&mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let namespaces = members
        .iter()
        .map(|members| labels::resolve(members, &mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let refs = namespaces.iter().collect::<Vec<_>>();
    let resolved =
        scopes::resolve(&request.set, &refs, registry, &mut codec, &mut b).map_err(err)?;
    let prepared = html::prepare(&resolved, &request.options, &mut b).map_err(err)?;
    // Keep the source-admission population identical for every page-count run.
    // Admission charges a logarithmic size-derived search bound even for an
    // already admitted snapshot. Prewarming the complete fixture pool isolates
    // composition traversal from that independent source-count dimension.
    // All preparation and admission still use the operation's finite budget.
    for page in pool {
        page.document
            .validate_structure(registry, &mut b, &mut admission)
            .map_err(err)?;
    }
    let start = b.usage().work;
    let mut outputs = Vec::new();
    for (page, plan) in plans.iter().enumerate() {
        outputs.push(
            composition::render(
                plan.selection(),
                &prepared,
                page as u64,
                registry,
                &mut |_, _, _, _, _| Err(Error::NeedsResolution),
                &mut |_, _, _, _| Err(Error::NeedsResolution),
                &mut b,
                &mut admission,
            )
            .map_err(err)?,
        );
    }
    let requests = outputs
        .iter()
        .map(|output| {
            output
                .members()
                .first()
                .map(|member| &member.document().output().fragment.markup)
                .ok_or("root output")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let composed = b.usage().work;
    let checked = html::output::check(&prepared, &requests, &mut b).map_err(err)?;
    let work = Work {
        composition: composed
            .checked_sub(start)
            .ok_or("composition measurement")?,
        output_validation: b
            .usage()
            .work
            .checked_sub(composed)
            .ok_or("validation measurement")?,
    };
    // Observation is outside the measured operations and reads validated HTML.
    let links = checked
        .pages()
        .iter()
        .map(|page| {
            page.fragment()
                .nodes
                .iter()
                .filter_map(|node| match node {
                    HtmlNode::Element { attributes, .. } => Some(attributes),
                    _ => None,
                })
                .flatten()
                .filter_map(|attribute| match attribute {
                    HtmlAttribute::Href { value } => Some(value.clone()),
                    _ => None,
                })
                .collect()
        })
        .collect();
    Ok((links, work))
}
