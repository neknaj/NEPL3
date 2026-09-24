//! Complete native page composition before any filesystem output is created.
use super::{composition, discovery};
use crate::doc::{
    export::{Stage, StageMeasurement, checked_shell},
    source::{Compiled, err},
};
use nepl3_core::{
    budget::{Budget, StopReason},
    source::{Digest, SourceAdmission, SourceStore},
};
use nepl3_doc_core::{labels::namespace as labels, pages::namespace as scopes};
use nepl3_doc_html::pages::{PagesHtmlRequest, namespace as html};
use nepl3_sentence_core::lower::ForeignInlineForm;
use nepl3_wire::foundation::FoundationCodec;
use std::time::Instant;

pub(in crate::doc::export) struct SerializedPages {
    pub identity: Digest,
    pub pages: Vec<String>,
}

#[derive(Debug)]
enum AdapterError {
    Stopped(StopReason),
    Sentence {
        document: discovery::DocumentId,
        slot: nepl3_doc_core::model::EmbedRef,
        embed: nepl3_sentence_core::model::EmbedRef,
    },
    Document {
        document: discovery::DocumentId,
        embed: nepl3_doc_core::model::EmbedRef,
    },
}
impl From<StopReason> for AdapterError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl core::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Stopped(reason) => write!(f, "{reason:?}"),
            Self::Sentence {
                document,
                slot,
                embed,
            } => write!(
                f,
                "Sentence guest adapter required: {document:?}/{slot:?}/{embed:?}"
            ),
            Self::Document { document, embed } => {
                write!(f, "Doc guest adapter required: {document:?}/{embed:?}")
            }
        }
    }
}

/// This local export selects Doc Inline inside Sentence. Additional guest
/// operations require explicit host adapters; they never fall back to text.
/// Source owners and all placement tables remain live through final checking.
pub(super) fn render(
    request: &PagesHtmlRequest,
    compiled: &Compiled,
    b: &mut Budget,
) -> Result<SerializedPages, String> {
    render_observed(request, compiled, b, &mut |_| {})
}

pub(in crate::doc::export) fn render_observed(
    request: &PagesHtmlRequest,
    compiled: &Compiled,
    b: &mut Budget,
    observe: &mut impl FnMut(StageMeasurement),
) -> Result<SerializedPages, String> {
    b.poll().map_err(err)?;
    let prepare_start = Instant::now();
    let registry = &compiled.doc.registry;
    let sentence = registry
        .selected("nepl3.syntax.sentence", 1)
        .ok_or("Sentence schema")?;
    let forms = [ForeignInlineForm {
        kind: "Form:DocumentInline",
        guest_schema: &compiled.doc.package.schema,
        guest_category: "Inline",
    }];
    let store = SourceStore::default();
    let mut codec_admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(registry, &store, &mut codec_admission).map_err(err)?;
    let count = request.set.pages.len();
    let mut discovered = composition::array(count, b).map_err(err)?;
    for page in &request.set.pages {
        discovered.push(
            discovery::collect(
                &page.document,
                sentence,
                &compiled.doc.package.schema,
                &forms,
                registry,
                &mut codec,
                b,
            )
            .map_err(err)?,
        );
    }
    let mut admission = SourceAdmission::default();
    let mut plans = composition::array(count, b).map_err(err)?;
    for page in &discovered {
        plans.push(discovery::namespace::inspect(page, registry, b, &mut admission).map_err(err)?);
    }
    let mut members = composition::array(count, b).map_err(err)?;
    for plan in &plans {
        members.push(plan.member_refs(b).map_err(err)?);
    }
    let mut namespaces = composition::array(count, b).map_err(err)?;
    for members in &members {
        namespaces.push(labels::resolve(members, b).map_err(err)?);
    }
    let mut refs = composition::array(count, b).map_err(err)?;
    for namespace in &namespaces {
        refs.push(namespace);
    }
    let resolved = scopes::resolve(&request.set, &refs, registry, &mut codec, b).map_err(err)?;
    let prepared = html::prepare(&resolved, &request.options, b).map_err(err)?;
    observe(StageMeasurement {
        stage: Stage::Prepare,
        elapsed: prepare_start.elapsed(),
        usage: b.usage(),
    });
    let render_start = Instant::now();
    let mut outputs = composition::array(count, b).map_err(err)?;
    for (page, plan) in plans.iter().enumerate() {
        outputs.push(
            composition::render(
                plan,
                &prepared,
                page as u64,
                registry,
                &mut |document, slot, _, embed, _| {
                    Err(AdapterError::Sentence {
                        document,
                        slot,
                        embed,
                    })
                },
                &mut |document, _, embed, _| Err(AdapterError::Document { document, embed }),
                b,
                &mut admission,
            )
            .map_err(err)?,
        );
    }
    let mut requests = composition::array(count, b).map_err(err)?;
    for output in &outputs {
        requests.push(
            &output
                .members()
                .first()
                .ok_or("missing root")?
                .document()
                .output()
                .fragment
                .markup,
        );
    }
    let checked = html::output::check(&prepared, &requests, b).map_err(err)?;
    let mut pages = composition::array(count, b).map_err(err)?;
    for page in checked.pages() {
        pages.push(checked_shell(page, b)?);
    }
    b.poll().map_err(err)?;
    observe(StageMeasurement {
        stage: Stage::RenderAndSerialize,
        elapsed: render_start.elapsed(),
        usage: b.usage(),
    });
    Ok(SerializedPages {
        identity: checked.namespace_identity(),
        pages,
    })
}
