//! Select Doc Inline guests from an independent Sentence without copying its
//! meaning into the legacy Doc Sentence model. No renderer or host I/O is used.
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::SyntaxError,
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{check::Category, lower, model::DocumentSyntax};
use nepl3_sentence_core::{model::EmbedRef, syntax::SentenceSyntax};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Sentence(nepl3_sentence_core::syntax::Error),
    Shape(nepl3_sentence_core::check::Error),
    Closure(SyntaxError),
    Lower(lower::LowerError),
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Index in the selection's document table, independent of Sentence embed IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentId(usize);
impl DocumentId {
    pub fn index(self) -> usize {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub embed: EmbedRef,
    pub document: DocumentId,
}
pub struct Selection {
    documents: Vec<DocumentSyntax>,
    occurrences: Vec<Occurrence>,
}
impl Selection {
    pub fn documents(&self) -> &[DocumentSyntax] {
        &self.documents
    }
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }
    /// Transfer unique document ownership and the ordered occurrence mapping.
    /// Each DocumentId indexes this returned document table.
    pub fn into_parts(self) -> (Vec<DocumentSyntax>, Vec<Occurrence>) {
        (self.documents, self.occurrences)
    }
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        2 * core::mem::size_of::<T>() as u64,
    )?;
    items
        .try_reserve(1)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    items.push(item);
    Ok(())
}

/// Validate local Sentence syntax, then lower occurrences whose complete schema
/// identity and category match the explicitly selected Doc Inline surface.
/// Each embed is lowered once at its deepest occurrence; sharing and display
/// order are preserved. Other guests remain owned by Sentence and need their
/// own adapters. This operation performs no name resolution or guest evaluation.
/// All validation and lowering use the caller's cumulative Budget/admission.
pub fn collect<C: FoundationValueCodec>(
    sentence: &SentenceSyntax,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection, Error> {
    let result = collect_inner(sentence, surface, registry, codec, b);
    b.poll()?;
    result
}
fn collect_inner<C: FoundationValueCodec>(
    sentence: &SentenceSyntax,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<Selection, Error> {
    let checked = sentence
        .validate(registry, b, codec.source_admission())
        .map_err(|error| match error {
            nepl3_sentence_core::syntax::Error::Stopped(reason) => Error::Stopped(reason),
            error => Error::Sentence(error),
        })?;
    let shape_error = |error| match error {
        nepl3_sentence_core::check::Error::Stopped(reason) => Error::Stopped(reason),
        error => Error::Shape(error),
    };
    let occurrences = checked
        .shape()
        .foreign_occurrences(b)
        .map_err(shape_error)?;
    let depths = checked.shape().foreign_depths(b).map_err(shape_error)?;
    let mut mapping = Vec::new();
    let count = sentence.value.embeds.len();
    b.charge(Resource::Work, count as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(core::mem::size_of::<Option<DocumentId>>() as u64),
    )?;
    mapping
        .try_reserve_exact(count)
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    mapping.resize(count, None);
    let mut selected = Selection {
        documents: Vec::new(),
        occurrences: Vec::new(),
    };
    let base = b.current_depth();
    for occurrence in occurrences {
        let index = occurrence.embed.0 as usize;
        let closure = &sentence.value.embeds[index];
        b.charge(
            Resource::Work,
            (surface.package.len()
                + closure.syntax.schema.package.len()
                + closure.syntax.category.len()) as u64
                + 70,
        )?;
        if &closure.syntax.schema != surface || closure.syntax.category != "Inline" {
            continue;
        }
        let document = match mapping[index] {
            Some(id) => id,
            None => {
                let document = b.with_depth_at_least(base.saturating_add(depths[index]), |b| {
                    let checked = closure
                        .validate(registry, b, codec.source_admission())
                        .map_err(Error::Closure)?;
                    lower::document(
                        checked.syntax(),
                        surface,
                        Category::Inline,
                        registry,
                        b,
                        codec,
                    )
                    .map_err(Error::Lower)
                })?;
                let id = DocumentId(selected.documents.len());
                push(&mut selected.documents, document, b)?;
                mapping[index] = Some(id);
                id
            }
        };
        push(
            &mut selected.occurrences,
            Occurrence {
                embed: occurrence.embed,
                document,
            },
            b,
        )?;
    }
    Ok(selected)
}
