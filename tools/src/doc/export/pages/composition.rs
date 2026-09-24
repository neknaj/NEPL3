//! Bottom-up rendering of discovered owners in one complete page namespace.
//! Pending output retains every original owner and local source correspondence.
use super::discovery::{DocumentId, namespace::Plan};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};
use nepl3_doc_core::{
    labels::namespace::MemberId,
    model::{DocEmbed, EmbedRef},
    pages::namespace::Owner,
};
use nepl3_doc_html::pages::namespace as pages;
use nepl3_markup::html::HtmlRequest;
use nepl3_sentence_core::model::{EmbedRef as SentenceEmbed, InlineContent};
use nepl3_suite::adapters::sentence::html::{self, PendingSentence};

pub struct Member<'a> {
    document: pages::PendingMember,
    sentences: Vec<Option<PendingSentence<'a>>>,
}
impl<'a> Member<'a> {
    pub fn document(&self) -> &pages::PendingMember {
        &self.document
    }
    pub fn sentence(&self, slot: EmbedRef) -> Option<&PendingSentence<'a>> {
        usize::try_from(slot.0)
            .ok()
            .and_then(|i| self.sentences.get(i))
            .and_then(Option::as_ref)
    }
}
pub struct Output<'p, 'c, 'd> {
    plan: &'p Plan<'c, 'd>,
    // Dense discovery-owner order, independent of repeated display occurrences.
    members: Vec<Member<'p>>,
}
impl Output<'_, '_, '_> {
    pub fn plan(&self) -> &Plan<'_, '_> {
        self.plan
    }
    pub fn members(&self) -> &[Member<'_>] {
        &self.members
    }
    pub fn member(&self, id: DocumentId) -> Option<&Member<'_>> {
        self.members.get(id.index())
    }
}
#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection(Owner),
    InternalShape,
    Sentence {
        owner: Owner,
        slot: EmbedRef,
        error: html::RenderFailure<E>,
    },
    Document(pages::MemberError<E>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
fn array<T>(count: usize, b: &mut Budget) -> Result<Vec<T>, StopReason> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .filter(|bytes| *bytes <= isize::MAX as usize)
        .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
    b.charge(Resource::AllocationUnits, bytes as u64)?;
    b.charge(Resource::Work, count as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    Ok(values)
}
/// All Sentence slots are rendered, including unselected language variants.
/// Selected Doc guests use already rendered child owners; other roles require
/// explicit adapters. No callback is invoked until the complete occurrence
/// sequence has been matched to the prepared namespace. Final page validation
/// remains mandatory before serialization.
#[allow(clippy::too_many_arguments)]
pub fn render<'p, 'c, 'd, E: From<StopReason>>(
    plan: &'p Plan<'c, 'd>,
    namespace: &pages::PreparedPages<'_, '_, '_, '_>,
    page: u64,
    registry: &SchemaRegistry,
    sentence_guest: &mut impl FnMut(
        DocumentId,
        EmbedRef,
        &InlineContent,
        SentenceEmbed,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    document_guest: &mut impl FnMut(
        DocumentId,
        &DocEmbed,
        EmbedRef,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Output<'p, 'c, 'd>, Error<E>> {
    b.poll()?;
    let result = (|| {
        let input = plan.input().members();
        let mut owners = array(input.len(), b)?;
        owners.resize(input.len(), None);
        for (index, occurrence) in plan.occurrences().iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let owner = Owner {
                page,
                member: MemberId(index as u64),
            };
            let document = input
                .get(occurrence.document.index())
                .ok_or(Error::InternalShape)?;
            if !namespace
                .checked()
                .document(owner)
                .is_some_and(|d| core::ptr::eq(d, document.document()))
            {
                return Err(Error::Selection(owner));
            }
            owners[occurrence.document.index()].get_or_insert(owner);
        }
        let end = Owner {
            page,
            member: MemberId(plan.occurrences().len() as u64),
        };
        if namespace.checked().document(end).is_some() {
            return Err(Error::Selection(end));
        }
        let mut outputs: Vec<Member<'p>> = array(input.len(), b)?;
        let base = b.current_depth();
        let root_depth = input.first().ok_or(Error::InternalShape)?.depth();
        // Discovery admits a child after its parent. Reverse order therefore
        // renders arbitrary nesting without recursive host calls.
        for (index, document) in input.iter().enumerate().rev() {
            let owner = owners[index].ok_or(Error::InternalShape)?;
            let id = plan.occurrences()[owner.member.0 as usize].document;
            let depth = base.saturating_add(document.depth().saturating_sub(root_depth));
            let output = b.with_depth_at_least(depth, |b| {
                let mut slots = array(document.sentences().len(), b)?;
                let mut depths = array(document.sentences().len(), b)?;
                depths.resize(document.sentences().len(), 0u64);
                // Dense embed-indexed routing avoids scanning guest occurrences
                // at every callback; shared occurrences select the same owner.
                let mut routes = array(document.sentences().len(), b)?;
                for sentence in document.sentences() {
                    let count = sentence.as_ref().map_or(0, |s| s.value.embeds.len());
                    let mut row = array(count, b)?;
                    row.resize(count, None);
                    routes.push(row);
                }
                for occurrence in document.occurrences() {
                    b.charge(Resource::Work, 1)?;
                    let at = &mut depths[occurrence.embed.0 as usize];
                    *at = (*at).max(occurrence.depth);
                }
                for guest in document.guests() {
                    b.charge(Resource::Work, 1)?;
                    routes[guest.slot.0 as usize][guest.occurrence.embed.0 as usize] =
                        Some(guest.document);
                }
                for (slot_index, sentence) in document.sentences().iter().enumerate() {
                    b.charge(Resource::Work, 1)?;
                    let slot = EmbedRef(slot_index as u64);
                    let rendered =
                        match sentence {
                            None => None,
                            Some(sentence) => Some(b.with_depth_at_least(
                                depth.saturating_add(depths[slot_index]),
                                |b| {
                                    html::render_part_with_foreign(
                                        sentence,
                                        registry,
                                        &mut |guest, embed, b| {
                                            b.charge(Resource::Work, 1).map_err(E::from)?;
                                            match routes[slot_index][embed.0 as usize] {
                                                Some(child) => outputs
                                                    [input.len() - 1 - child.index()]
                                                .document
                                                .output()
                                                .fragment
                                                .markup
                                                .clone_with_budget(b)
                                                .map_err(E::from),
                                                None => sentence_guest(id, slot, guest, embed, b),
                                            }
                                        },
                                        b,
                                        admission,
                                    )
                                    .map_err(|error| Error::Sentence { owner, slot, error })
                                },
                            )?),
                        };
                    slots.push(rendered);
                }
                let rendered = pages::render_member(
                    namespace,
                    owner,
                    &mut |guest, embed, b| match slots[embed.0 as usize].as_ref() {
                        Some(sentence) => sentence.markup().clone_with_budget(b).map_err(E::from),
                        None => document_guest(id, guest, embed, b),
                    },
                    b,
                )
                .map_err(Error::Document)?;
                Ok::<_, Error<E>>(Member {
                    document: rendered,
                    sentences: slots,
                })
            })?;
            outputs.push(output);
        }
        b.charge(Resource::Work, outputs.len() as u64)?;
        outputs.reverse();
        Ok(Output {
            plan,
            members: outputs,
        })
    })();
    b.poll()?;
    result
}
