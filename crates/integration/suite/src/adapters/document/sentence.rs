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

/// Validate and store an independent Sentence presentation in a Doc slot.
/// The root determines the slot role. Sources, locations and nested closures
/// retain their Sentence ownership; no source text or positions are invented.
/// Encoding and the owned record copy share the caller's resource budget.
pub fn embed<C: FoundationValueCodec>(
    input: &SentenceSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<DocEmbed, Error<C::Error>> {
    budget.poll()?;
    let result = (|| {
        let encoded =
            nepl3_sentence_core::portable::syntax::to_value(input, registry, codec, budget)
                .map_err(Error::Value)?;
        let NdfValue::Record(record) = &encoded else {
            return Err(Error::Value(nepl3_sentence_core::portable::Error::Shape));
        };
        encoded.charge_clone(budget)?;
        Ok(DocEmbed {
            kind: match input.value.root {
                Root::Sentence(_) => EmbedKind::Sentence,
                Root::Inline(_) => EmbedKind::SentenceInline,
            },
            content: DocContent::Value {
                value: TypedValue::Record(record.clone()),
            },
        })
    })();
    budget.poll()?;
    result
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
    Lowerer::new(registry).lower(input, surface, forms, codec, budget)
}

/// One immutable registry and the most recent owner proof. Scoped to one
/// collection operation; each selected guest still receives complete checks.
pub(super) struct Lowerer<'a> {
    registry: &'a SchemaRegistry,
    closures: lower::presentation::ClosureLowerer<'a>,
}
impl<'a> Lowerer<'a> {
    pub(super) fn new(registry: &'a SchemaRegistry) -> Self {
        Self {
            registry,
            closures: lower::presentation::ClosureLowerer::new(registry),
        }
    }
    pub(super) fn lower<C: FoundationValueCodec>(
        &mut self,
        input: &'a DocEmbed,
        surface: &SchemaRef,
        forms: &[lower::ForeignInlineForm<'_>],
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<SentenceSyntax, Error<C::Error>> {
        self.lower_checked(input, None, surface, forms, codec, budget)
    }

    pub(super) fn lower_checked<C: FoundationValueCodec>(
        &mut self,
        input: &'a DocEmbed,
        syntax: Option<&nepl3_core::syntax::RegistryValidatedSyntaxBundle<'_, '_>>,
        surface: &SchemaRef,
        forms: &[lower::ForeignInlineForm<'_>],
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<SentenceSyntax, Error<C::Error>> {
        let registry = self.registry;
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
                    nepl3_sentence_core::portable::syntax::from_value(
                        &value, registry, codec, budget,
                    )
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
                    let result = if let Some(proof) = syntax {
                        budget.charge(Resource::Work, 1)?;
                        if !core::ptr::eq(proof.bundle(), &closure.syntax.bundle) {
                            return Err(Error::Closure(SyntaxError::Reference));
                        }
                        lower::presentation::sentence_in_registry(
                            proof, surface, forms, registry, codec, budget,
                        )
                    } else {
                        self.closures.lower(closure, surface, forms, codec, budget)
                    };
                    result.map_err(|error| match error {
                        lower::presentation::Error::Closure(error) => Error::Closure(error),
                        lower::presentation::Error::Category => Error::Category,
                        error => Error::Lower(error),
                    })?
                }
            };
            if !matches!(
                (input.kind, sentence.value.root),
                (EmbedKind::Sentence, Root::Sentence(_))
                    | (EmbedKind::SentenceInline, Root::Inline(_))
            ) {
                return Err(Error::Category);
            }
            Ok(sentence)
        })();
        budget.poll()?;
        result
    }
}
