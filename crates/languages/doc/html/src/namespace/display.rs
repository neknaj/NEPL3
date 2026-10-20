//! Display-capable members of an explicitly resolved namespace. This does not
//! select Sentence owners, infer visibility, or automatically insert members.
use super::*;
use nepl3_doc_core::model::{DocEmbedView, DocRoot, EmbedRef};

pub struct PreparedDisplayNamespace<'a>(PreparedNamespace<'a>);

/// Admit the same safe external URIs and explicit foreign kinds as Article
/// display rendering. Page/relative links, assets and unsupported guests remain
/// unresolved. This does not broaden the existing InlineMath-only entry point.
pub fn prepare<'a, C: FoundationValueCodec>(
    namespace: &CheckedNamespace<'_, 'a>,
    options: &'a RenderOptions,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<PreparedDisplayNamespace<'a>, LocalPreparationError<'a, C::Error>> {
    prepare_members(
        namespace,
        options,
        registry,
        codec,
        budget,
        Admission::Display,
    )
    .map(PreparedDisplayNamespace)
}

/// A structurally checked member with cross-member identities still pending.
/// The composing host must insert it and validate the complete destination.
pub fn render_part<E>(
    prepared: &PreparedDisplayNamespace<'_>,
    member: MemberId,
    adapter: &mut impl FnMut(&DocEmbedView<'_>, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<PendingForeignPart, ForeignPartError<E>> {
    render_pending(&prepared.0, member, adapter, budget)
}

#[derive(Debug)]
pub enum ArticleError<E> {
    Member(MemberId),
    ExpectedArticle(MemberId),
    Render(crate::ForeignRenderError<E>),
}

/// Render one Article root, not a concatenation of all namespace members.
/// Adapters insert selected pending parts; full output validation rejects
/// missing targets, duplicate IDs and invalid nesting before returning output.
pub fn render_article<E>(
    prepared: &PreparedDisplayNamespace<'_>,
    member: MemberId,
    adapter: &mut impl FnMut(&DocEmbedView<'_>, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    budget: &mut Budget,
) -> Result<crate::RenderedInlineWithForeign, ArticleError<E>> {
    budget
        .poll()
        .map_err(|reason| ArticleError::Render(reason.into()))?;
    let input = usize::try_from(member.0)
        .ok()
        .and_then(|index| prepared.0.members.get(index))
        .ok_or(ArticleError::Member(member))?;
    if !matches!(input.document.value.root, DocRoot::Article(_)) {
        return Err(ArticleError::ExpectedArticle(member));
    }
    build::render_prepared_with_foreign(input, &input.external_links, adapter, budget, true)
        .map_err(ArticleError::Render)
}
