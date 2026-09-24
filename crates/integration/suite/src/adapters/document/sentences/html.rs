//! Preflight all Sentence slots before Doc display-language selection.
use super::*;
use crate::adapters::sentence::html::{self as sentence_html, PendingSentence, RenderFailure};
use nepl3_markup::html::HtmlRequest;
use nepl3_sentence_core::model::{EmbedRef as SentenceEmbed, InlineContent};

pub struct Prepared<'a, 'd> {
    input: &'a Selection<'d>,
    slots: Vec<Option<PendingSentence<'a>>>,
}
impl<'a, 'd> Prepared<'a, 'd> {
    pub fn input(&self) -> &'a Selection<'d> {
        self.input
    }
    pub fn get(&self, embed: EmbedRef) -> Option<&PendingSentence<'a>> {
        usize::try_from(embed.0)
            .ok()
            .and_then(|index| self.slots.get(index))
            .and_then(Option::as_ref)
    }
    /// Preserve embed-table indexing when transferring pending outputs. These
    /// parts require complete composition checks before serialization.
    pub fn into_parts(self) -> Vec<Option<PendingSentence<'a>>> {
        self.slots
    }
}
#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Sentence {
        embed: EmbedRef,
        error: RenderFailure<E>,
    },
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Build every collected slot once, including hidden variants. The adapter is
/// explicitly selected by the caller; both Doc slot and Sentence embed identify
/// each invocation. A failure exposes no partial table and stops subsequent
/// callbacks. Pending IDs and fragment targets belong to the complete document.
pub fn prepare<'a, 'd, E: From<StopReason>>(
    input: &'a Selection<'d>,
    registry: &SchemaRegistry,
    adapter: &mut impl FnMut(
        EmbedRef,
        &InlineContent,
        SentenceEmbed,
        &mut Budget,
    ) -> Result<HtmlRequest, E>,
    b: &mut Budget,
    admission: &mut nepl3_core::source::SourceAdmission,
) -> Result<Prepared<'a, 'd>, Error<E>> {
    b.poll()?;
    let result = (|| {
        let count = input.slots.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<Option<PendingSentence<'a>>>())
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let mut depths = Vec::new();
        let bytes = count
            .checked_mul(core::mem::size_of::<u64>())
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(Resource::Work, count as u64)?;
        depths
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        depths.resize(count, 0u64);
        for occurrence in input.occurrences() {
            b.charge(Resource::Work, 1)?;
            let depth = &mut depths[occurrence.embed.0 as usize];
            *depth = (*depth).max(occurrence.depth);
        }
        let base = b.current_depth();
        for (index, sentence) in input.slots.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let embed = EmbedRef(index as u64);
            let output = match sentence {
                Some(sentence) => Some(b.with_depth_at_least(
                    base.saturating_add(depths[index]),
                    |b| {
                        sentence_html::render_part_with_foreign(
                            sentence,
                            registry,
                            &mut |guest, guest_embed, b| adapter(embed, guest, guest_embed, b),
                            b,
                            admission,
                        )
                        .map_err(|error| Error::Sentence { embed, error })
                    },
                )?),
                None => None,
            };
            slots.push(output);
        }
        Ok(Prepared { input, slots })
    })();
    b.poll()?;
    result
}
