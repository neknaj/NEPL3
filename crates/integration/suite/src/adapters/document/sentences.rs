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
    check::{ForeignOccurrence, RegistryValidatedDocumentSyntax, ShapeError, StructureError},
    model::{DocContent, DocumentSyntax, EmbedKind, EmbedRef},
};
use nepl3_sentence_core::{lower::ForeignInlineForm, syntax::SentenceSyntax};

#[cfg(feature = "sentence-html")]
pub mod html;

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
    /// Transfer the embed-indexed Sentence slots and structural occurrences.
    /// Empty slots belong to other guest roles retained by the original Doc.
    pub fn into_parts(self) -> (Vec<Option<SentenceSyntax>>, Vec<ForeignOccurrence>) {
        (self.slots, self.occurrences)
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
        let checked =
            RegistryValidatedDocumentSyntax::new(document, registry, b, codec.source_admission())
                .map_err(Error::Document)?;
        collect_checked(&checked, surface, forms, registry, codec, b)
    })();
    b.poll()?;
    result
}

/// Reuse a complete immutable Doc/guest proof after applying this operation's
/// registry, source admission and depth. Language-specific lowering remains
/// mandatory; this proof grants no Sentence semantics or guest selection.
pub fn collect_validated<'a, C: FoundationValueCodec>(
    checked: &RegistryValidatedDocumentSyntax<'a, '_>,
    surface: &SchemaRef,
    forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection<'a>, Error<C::Error>> {
    let result = (|| {
        checked
            .validate_for(registry, b, codec.source_admission())
            .map_err(Error::Document)?;
        collect_checked(checked, surface, forms, registry, codec, b)
    })();
    b.poll()?;
    result
}

fn collect_checked<'a, C: FoundationValueCodec>(
    checked: &RegistryValidatedDocumentSyntax<'a, '_>,
    surface: &SchemaRef,
    forms: &[ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection<'a>, Error<C::Error>> {
    let document = checked.structure().document();
    let result = (|| {
        let depths = checked
            .structure()
            .shape()
            .foreign_depths(b)
            .map_err(Error::Shape)?;
        let mut occurrences = checked
            .structure()
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
        let mut lowerer = sentence::Lowerer::new(registry);
        let mut syntax = checked.syntax().iter();
        for (index, slot) in document.value.embeds.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let proof = match slot.content {
                DocContent::Syntax { .. } => Some(syntax.next().ok_or(Error::Document(
                    StructureError::Syntax(nepl3_core::syntax::SyntaxError::Reference),
                ))?),
                DocContent::Value { .. } => None,
            };
            let input = if matches!(slot.kind, EmbedKind::Sentence | EmbedKind::SentenceInline) {
                Some(
                    b.with_depth_at_least(base.saturating_add(depths[index]), |b| {
                        lowerer
                            .lower_checked(slot, proof, surface, forms, codec, b)
                            .map_err(|error| Error::Sentence {
                                embed: EmbedRef(index as u64),
                                error,
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
