//! Prepare independently owned Sentence slots over a checked Doc structure.
use super::sentence;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::{ForeignOccurrence, ShapeError, StructureError},
    model::{DocumentSyntax, EmbedKind, EmbedRef},
};
use nepl3_sentence_core::{lower::ForeignInlineForm, syntax::SentenceSyntax};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Document(StructureError),
    Shape(ShapeError),
    Sentence {
        embed: EmbedRef,
        error: sentence::Error<E>,
    },
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Borrow the immutable Doc owner; each Sentence is lowered once per embed.
/// Structural occurrences retain order and sharing separately from that table.
/// This value grants no name-resolution or rendering proof for nested guests.
pub struct Selection<'a> {
    document: &'a DocumentSyntax,
    slots: Vec<Option<SentenceSyntax>>,
    occurrences: Vec<ForeignOccurrence>,
}
impl<'a> Selection<'a> {
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub fn sentence(&self, embed: EmbedRef) -> Option<&SentenceSyntax> {
        usize::try_from(embed.0)
            .ok()
            .and_then(|i| self.slots.get(i))
            .and_then(Option::as_ref)
    }
    pub fn occurrences(&self) -> &[ForeignOccurrence] {
        &self.occurrences
    }
}
/// Validate Doc provenance and lower every Sentence/Inline label slot at its
/// deepest owner occurrence. All Parallel alternatives and optional content are
/// included. Other guest roles remain untouched. Inputs and cumulative resource
/// limits are shared with the existing single-slot adapter; no guest is executed.
pub fn collect<'a, C: FoundationValueCodec>(
    document: &'a DocumentSyntax,
    surface: &SchemaRef,
    forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection<'a>, Error<C::Error>> {
    b.poll()?;
    let result = (|| {
        let checked = document
            .validate_structure(registry, b, codec.source_admission())
            .map_err(Error::Document)?;
        let depths = checked.shape().foreign_depths(b).map_err(Error::Shape)?;
        let mut occurrences = checked
            .shape()
            .foreign_occurrences(b)
            .map_err(Error::Shape)?;
        b.charge(Resource::Work, occurrences.len() as u64)?;
        occurrences.retain(|o| matches!(o.kind, EmbedKind::Sentence | EmbedKind::SentenceInline));
        let count = document.value.embeds.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<Option<SentenceSyntax>>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let base = b.current_depth();
        for (index, slot) in document.value.embeds.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let input = if matches!(slot.kind, EmbedKind::Sentence | EmbedKind::SentenceInline) {
                Some(
                    b.with_depth_at_least(base.saturating_add(depths[index]), |b| {
                        sentence::lower(slot, surface, forms, registry, codec, b).map_err(|error| {
                            Error::Sentence {
                                embed: EmbedRef(index as u64),
                                error,
                            }
                        })
                    })?,
                )
            } else {
                None
            };
            slots.push(input);
        }
        Ok(Selection {
            document,
            slots,
            occurrences,
        })
    })();
    // Nested boundary errors retain the common sticky stop as the public cause.
    b.poll()?;
    result
}
