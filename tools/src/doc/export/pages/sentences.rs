//! Insert prepared Sentence outputs into their exact page-namespace owner.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission},
};
use nepl3_doc_core::{
    model::{DocEmbed, EmbedRef},
    pages::namespace::Owner,
};
use nepl3_doc_html::{ForeignPlacement, RenderOptions, pages::namespace as pages};
use nepl3_markup::html::HtmlRequest;
use nepl3_sentence_core::model::{EmbedRef as SentenceEmbed, InlineContent};
use nepl3_suite::adapters::{
    document::sentences::{self, html::Prepared},
    sentence::html::PendingSentence,
};

pub struct PreparedMember<'n, 'a, 'd> {
    sentences: Prepared<'a, 'd>,
    namespace: Digest,
    owner: Owner,
    options: &'n RenderOptions,
}
/// Bind preparation to the checked page namespace and immutable display options.
/// The explicitly selected adapter is responsible for rendering guests in this
/// context. The resulting cache cannot be rebound to another owner or context.
pub fn prepare<'n, 'a, 'd, E: From<StopReason>>(
    input: &'a sentences::Selection<'d>,
    namespace: &pages::PreparedPages<'_, '_, '_, 'n>,
    owner: Owner,
    registry: &SchemaRegistry,
    adapter: &mut impl FnMut(
        EmbedRef,
        &InlineContent,
        SentenceEmbed,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<PreparedMember<'n, 'a, 'd>, Error<E>> {
    b.poll()?;
    if !namespace
        .checked()
        .document(owner)
        .is_some_and(|document| core::ptr::eq(document, input.document()))
    {
        return Err(Error::Selection(owner));
    }
    let sentences =
        sentences::html::prepare(input, registry, adapter, b, admission).map_err(Error::Prepare)?;
    Ok(PreparedMember {
        sentences,
        namespace: namespace.checked().identity(),
        owner,
        options: namespace.options(),
    })
}

pub struct Output<'p, 'n, 'a, 'd> {
    sentences: &'p PreparedMember<'n, 'a, 'd>,
    member: pages::PendingMember,
}
impl<'p, 'n, 'a, 'd> Output<'p, 'n, 'a, 'd> {
    pub fn member(&self) -> &pages::PendingMember {
        &self.member
    }
    /// Each occurrence retains its own destination offset. Sentence origin and
    /// nested-guest indices remain local to the borrowed prepared markup.
    pub fn sentences(&self) -> impl Iterator<Item = (&ForeignPlacement, &PendingSentence<'a>)> {
        self.member.output().foreign.iter().filter_map(|placement| {
            self.sentences
                .sentences
                .get(placement.embed)
                .map(|sentence| (placement, sentence))
        })
    }
    pub fn into_parts(self) -> (pages::PendingMember, &'p PreparedMember<'n, 'a, 'd>) {
        (self.member, self.sentences)
    }
}
#[derive(Debug)]
pub enum Error<E> {
    Selection(Owner),
    Stopped(StopReason),
    Prepare(sentences::html::Error<E>),
    Render(pages::MemberError<E>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Reuse preflight outputs, including their source owners, without invoking any
/// Sentence guest again. Other Doc roles use the caller's explicit adapter.
/// The returned member still requires complete page-output validation.
pub fn render_member<'p, 'n, 'a, 'd, E: From<StopReason>>(
    input: &'p PreparedMember<'n, 'a, 'd>,
    namespace: &pages::PreparedPages<'_, '_, '_, '_>,
    owner: Owner,
    other: &mut impl FnMut(&DocEmbed, EmbedRef, &mut Budget) -> Result<HtmlRequest, E>,
    b: &mut Budget,
) -> Result<Output<'p, 'n, 'a, 'd>, Error<E>> {
    b.poll()?;
    b.charge(Resource::Work, 33)?;
    if input.owner != owner
        || input.namespace != namespace.checked().identity()
        || !core::ptr::eq(input.options, namespace.options())
    {
        return Err(Error::Selection(owner));
    }
    let document = namespace
        .checked()
        .document(owner)
        .ok_or(Error::Selection(owner))?;
    if !core::ptr::eq(document, input.sentences.input().document()) {
        return Err(Error::Selection(owner));
    }
    let result = pages::render_member(
        namespace,
        owner,
        &mut |guest, embed, b| match input.sentences.get(embed) {
            Some(sentence) => sentence.markup().clone_with_budget(b).map_err(E::from),
            None => other(guest, embed, b),
        },
        b,
    );
    b.poll()?;
    Ok(Output {
        sentences: input,
        member: result.map_err(Error::Render)?,
    })
}
