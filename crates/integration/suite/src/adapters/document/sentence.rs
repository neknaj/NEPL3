//! Decode a selected Sentence slot without copying its meaning into Doc nodes.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::SyntaxError,
    value::{NdfValue, SchemaRef, TypedValue},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocContent, DocEmbed, EmbedKind};
use nepl3_sentence_core::{lower, model::Root, syntax::SentenceSyntax};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Role,
    Selection,
    Category,
    Closure(SyntaxError),
    Lower(lower::presentation::Error<E>),
    Value(nepl3_sentence_core::portable::Error<E>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// The caller supplies the selected Sentence surface and foreign-inline forms.
/// Its Budget depth is the owning Doc occurrence's depth. Closure validation,
/// literal/prefix lowering and semantic root checks share that Budget. This
/// operation preserves Sentence's local Source/Origin/View and evaluates no
/// nested guest. Printing, text projection and HTML remain separate operations.
pub fn lower<C: FoundationValueCodec>(
    input: &DocEmbed,
    surface: &SchemaRef,
    forms: &[lower::ForeignInlineForm<'_>],
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<SentenceSyntax, Error<C::Error>> {
    budget.poll()?;
    let result = (|| {
        let category = match input.kind {
            EmbedKind::Sentence => "Sentence",
            EmbedKind::SentenceInline => "Inline",
            _ => return Err(Error::Role),
        };
        let sentence = match &input.content {
            DocContent::Value { value } => {
                let value = match value.clone_with_budget(budget)? {
                    TypedValue::Record(v) => NdfValue::Record(v),
                    TypedValue::Variant(v) => NdfValue::Variant(v),
                };
                nepl3_sentence_core::portable::syntax::from_value(&value, registry, codec, budget)
                    .map_err(Error::Value)?
            }
            DocContent::Syntax { closure } => {
                budget.charge(
                    Resource::Work,
                    (closure.syntax.schema.package.len()
                        + surface.package.len()
                        + closure.syntax.category.len()
                        + category.len()) as u64
                        + 33,
                )?;
                if &closure.syntax.schema != surface {
                    return Err(Error::Selection);
                }
                if closure.syntax.category != category {
                    return Err(Error::Category);
                }
                let checked = closure
                    .validate(registry, budget, codec.source_admission())
                    .map_err(Error::Closure)?;
                lower::presentation::sentence_with_foreign(
                    checked.syntax(),
                    surface,
                    forms,
                    registry,
                    codec,
                    budget,
                )
                .map_err(Error::Lower)?
            }
        };
        if !matches!(
            (input.kind, sentence.value.root),
            (EmbedKind::Sentence, Root::Sentence(_)) | (EmbedKind::SentenceInline, Root::Inline(_))
        ) {
            return Err(Error::Category);
        }
        Ok(sentence)
    })();
    budget.poll()?;
    result
}
