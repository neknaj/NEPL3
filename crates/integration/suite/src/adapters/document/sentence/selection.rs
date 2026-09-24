//! Independent Sentence ownership for every structural Doc occurrence.
use super::*;
use alloc::vec::Vec;
use nepl3_doc_core::{
    check::{ForeignOccurrence, ShapeError, StructureError},
    model::DocumentSyntax,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentenceId(usize);
impl SentenceId {
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub owner: ForeignOccurrence,
    pub sentence: SentenceId,
}
pub struct Selection {
    sentences: Vec<SentenceSyntax>,
    occurrences: Vec<Occurrence>,
}
impl Selection {
    pub fn sentences(&self) -> &[SentenceSyntax] {
        &self.sentences
    }
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }
    pub fn into_parts(self) -> (Vec<SentenceSyntax>, Vec<Occurrence>) {
        (self.sentences, self.occurrences)
    }
}
#[derive(Debug)]
pub enum SelectionError<E> {
    Stopped(StopReason),
    Document(StructureError),
    Shape(ShapeError),
    Sentence {
        owner: ForeignOccurrence,
        error: Error<E>,
    },
}
impl<E> From<StopReason> for SelectionError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
fn push<T>(values: &mut Vec<T>, value: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        (core::mem::size_of::<T>() as u64).saturating_mul(2),
    )?;
    values
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    values.push(value);
    Ok(())
}
/// Validate the immutable Doc boundary and lower Sentence/SentenceInline slots
/// once per embed, at the deepest owner depth. All structural variants are
/// included, before output-language selection. Other guest roles stay in Doc
/// and require their own adapters. No labels are resolved or guests evaluated.
pub fn collect<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    surface: &SchemaRef,
    forms: &[lower::ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection, SelectionError<C::Error>> {
    let result = (|| {
        let checked = document
            .validate_structure(registry, b, codec.source_admission())
            .map_err(SelectionError::Document)?;
        let occurrences = checked
            .shape()
            .foreign_occurrences(b)
            .map_err(SelectionError::Shape)?;
        let depths = checked
            .shape()
            .foreign_depths(b)
            .map_err(SelectionError::Shape)?;
        let mut mapping = Vec::new();
        let count = document.value.embeds.len();
        let bytes = count
            .checked_mul(core::mem::size_of::<Option<SentenceId>>())
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, bytes as u64)?;
        b.charge(Resource::Work, count as u64)?;
        mapping
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        mapping.resize(count, None);
        let mut selection = Selection {
            sentences: Vec::new(),
            occurrences: Vec::new(),
        };
        let base = b.current_depth();
        for owner in occurrences {
            b.charge(Resource::Work, 1)?;
            if !matches!(owner.kind, EmbedKind::Sentence | EmbedKind::SentenceInline) {
                continue;
            }
            let index = owner.embed.0 as usize;
            let sentence = match mapping[index] {
                Some(id) => id,
                None => {
                    let input = b
                        .with_depth_at_least(base.saturating_add(depths[index]), |b| {
                            super::lower(
                                &document.value.embeds[index],
                                surface,
                                forms,
                                registry,
                                codec,
                                b,
                            )
                        })
                        .map_err(|error| SelectionError::Sentence { owner, error })?;
                    let id = SentenceId(selection.sentences.len());
                    push(&mut selection.sentences, input, b)?;
                    mapping[index] = Some(id);
                    id
                }
            };
            push(
                &mut selection.occurrences,
                Occurrence { owner, sentence },
                b,
            )?;
        }
        Ok(selection)
    })();
    b.poll()?;
    result
}
