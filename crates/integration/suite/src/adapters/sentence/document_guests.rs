//! Select Doc Inline guests from an independent Sentence without copying its
//! meaning into the legacy Doc Sentence model. No renderer or host I/O is used.
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::SyntaxError,
    value::{NdfValue, SchemaRef, TypedValue},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::Category,
    lower,
    model::{DocRoot, DocumentSyntax},
    portable,
};
use nepl3_sentence_core::{
    model::{EmbedRef, InlineContent},
    syntax::SentenceSyntax,
};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Sentence(nepl3_sentence_core::syntax::Error),
    Shape(nepl3_sentence_core::check::Error),
    Closure(SyntaxError),
    Lower(lower::LowerError),
    Value(portable::PortableError<E>),
    Selection,
    Category,
}
impl<E> From<StopReason> for Error<E> {
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
) -> Result<Selection, Error<C::Error>> {
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
) -> Result<Selection, Error<C::Error>> {
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
        let content = &sentence.value.embeds[index];
        if !self::selected(content, surface, registry, b)? {
            continue;
        }
        let document = match mapping[index] {
            Some(id) => id,
            None => {
                let document = b.with_depth_at_least(base.saturating_add(depths[index]), |b| {
                    decode(content, surface, registry, codec, b)
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

/// Match syntax against the supplied surface, or a typed value against the
/// registry's complete Doc meaning identity. Calling this adapter explicitly
/// selects Doc semantics. `surface` binds only Syntax inputs; typed values have
/// no surface grammar. The selected printer separately binds output spelling.
pub fn selected<E>(
    content: &InlineContent,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<bool, Error<E>> {
    b.poll()?;
    if !registry.is_finalized() {
        return Err(Error::Selection);
    }
    let expected = match content {
        InlineContent::Syntax { .. } => Some(surface),
        InlineContent::Value { .. } => registry.selected("nepl3.doc", 1),
    };
    let Some(expected) = expected else {
        return Ok(false);
    };
    b.charge(
        Resource::Work,
        (expected.package.len() + content.schema().package.len()) as u64 + 70,
    )?;
    if content.schema() != expected {
        return Ok(false);
    }
    match content {
        InlineContent::Syntax { closure } => {
            b.charge(Resource::Work, closure.syntax.category.len() as u64)?;
            Ok(closure.syntax.category == "Inline")
        }
        InlineContent::Value { .. } => Ok(true),
    }
}

/// Decode one explicitly selected Doc Inline. Nested language values retain
/// their own validation boundary and source closure.
pub fn decode<C: FoundationValueCodec>(
    content: &InlineContent,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<DocumentSyntax, Error<C::Error>> {
    b.poll()?;
    let result = (|| {
        if !selected(content, surface, registry, b)? {
            return Err(Error::Selection);
        }
        let document = match content {
            InlineContent::Syntax { closure } => {
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
                .map_err(Error::Lower)?
            }
            InlineContent::Value { value } => {
                let value = match value.clone_with_budget(b)? {
                    TypedValue::Record(value) => NdfValue::Record(value),
                    TypedValue::Variant(value) => NdfValue::Variant(value),
                };
                portable::from_value(&value, registry, codec, b).map_err(Error::Value)?
            }
        };
        if !matches!(document.value.root, DocRoot::Inline(_)) {
            return Err(Error::Category);
        }
        Ok(document)
    })();
    b.poll()?;
    result
}

/// Encode a typed Doc Inline as a Sentence foreign value. No source text or
/// source positions are synthesized. The Doc portable boundary validates the
/// document and retains the provenance of nested guests.
pub fn embed<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<InlineContent, Error<C::Error>> {
    b.poll()?;
    let result = (|| {
        if !matches!(document.value.root, DocRoot::Inline(_)) {
            return Err(Error::Category);
        }
        let value = portable::to_value(document, registry, codec, b).map_err(Error::Value)?;
        let NdfValue::Record(record) = &value else {
            return Err(Error::Value(portable::PortableError::Shape));
        };
        value.charge_clone(b)?;
        Ok(InlineContent::Value {
            value: TypedValue::Record(record.clone()),
        })
    })();
    b.poll()?;
    result
}
