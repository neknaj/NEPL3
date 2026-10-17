//! Decode the independent Sentence wire models at the Doc consumer boundary.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceSnapshot},
    syntax::{NodeRef, SyntaxBundle, SyntaxError, SyntaxNode, ValidatedSyntaxBundle},
    value::{NdfValue, SchemaRef},
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check::{Category, ShapeError, StructureError},
    lower::{LiteralDecoder, LowerError},
    model::DocumentSyntax,
};
use nepl3_sentence_core::{portable, syntax::SentenceSyntax};

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    Stopped(StopReason),
    Lower(LowerError),
    Payload {
        node: NodeRef,
        error: portable::Error<E>,
    },
    Adapter {
        node: NodeRef,
        error: super::Error,
    },
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl<E> From<LowerError> for Error<E> {
    fn from(error: LowerError) -> Self {
        match error {
            LowerError::Stopped(reason) => Self::Stopped(reason),
            error => Self::Lower(error),
        }
    }
}
impl<E> From<SyntaxError> for Error<E> {
    fn from(error: SyntaxError) -> Self {
        LowerError::from(error).into()
    }
}
impl<E> From<ShapeError> for Error<E> {
    fn from(error: ShapeError) -> Self {
        LowerError::from(error).into()
    }
}
impl<E> From<StructureError> for Error<E> {
    fn from(error: StructureError) -> Self {
        LowerError::from(error).into()
    }
}

// An owned immutable result of one complete Sentence codec validation, tied to
// the exact borrowed registry used for decoding. No mutable syntax access or
// public constructor is exposed; this is not a portable proof or Doc proof.
pub(super) struct DecodedSentence<'r> {
    syntax: SentenceSyntax,
    registry: &'r SchemaRegistry,
}
impl DecodedSentence<'_> {
    pub(super) fn syntax(&self) -> &SentenceSyntax {
        &self.syntax
    }
    pub(super) fn registry(&self) -> &SchemaRegistry {
        self.registry
    }
}
fn decode<'r, C: FoundationValueCodec>(
    payload: &NdfValue,
    owner: Option<&SourceSnapshot>,
    registry: &'r SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<DecodedSentence<'r>, portable::Error<C::Error>> {
    let syntax = match owner {
        Some(owner) => portable::literal::from_value(payload, owner, registry, codec, b)?,
        None => portable::syntax::from_value(payload, registry, codec, b)?,
    };
    b.poll()?;
    Ok(DecodedSentence { syntax, registry })
}

struct Decoder<'a, C>(&'a mut C);
impl<C: FoundationValueCodec> LiteralDecoder for Decoder<'_, C> {
    type Error = Error<C::Error>;
    fn admission(&mut self) -> &mut SourceAdmission {
        self.0.source_admission()
    }
    fn read(
        &mut self,
        id: NodeRef,
        node: &SyntaxNode,
        bundle: &SyntaxBundle,
        registry: &SchemaRegistry,
        b: &mut Budget,
    ) -> Result<DocumentSyntax, Self::Error> {
        let token = node
            .token
            .and_then(|t| bundle.tokens.get(t.0 as usize))
            .ok_or(LowerError::LiteralPayload { node: id })?;
        b.charge(Resource::Work, 1)?;
        let NdfValue::Record(record) = &token.payload else {
            return Err(LowerError::LiteralPayload { node: id }.into());
        };
        // Selection is explicit; a failed codec is never retried as another
        // payload. Each codec validates the complete independent schema ID.
        let referenced =
            nepl3_core::lexical::equal_with_budget(&record.kind, "SentenceLiteralPayload", b)?;
        let result = if referenced {
            let mut owner = None;
            for source in &bundle.sources {
                if source
                    .identity()
                    .compare_with_budget(token.head.snapshot_ref(), b)?
                    == core::cmp::Ordering::Equal
                {
                    owner = Some(source);
                    break;
                }
            }
            let owner = owner.ok_or(LowerError::LiteralPayload { node: id })?;
            decode(&token.payload, Some(owner), registry, self.0, b)
        } else {
            decode(&token.payload, None, registry, self.0, b)
        };
        let syntax = result.map_err(|error| match error {
            portable::Error::Stopped(reason) => Error::Stopped(reason),
            error => Error::Payload { node: id, error },
        })?;
        super::document::decoded_document(&syntax, b, self.0.source_admission()).map_err(|error| {
            match error {
                super::Error::Stopped(reason) => Error::Stopped(reason),
                error => Error::Adapter { node: id, error },
            }
        })
    }
}

/// Consume independent Sentence literal/syntax payloads without a Doc-specific
/// reader or wire projection. Doc retains final token and local closure checks.
pub fn lower<C: FoundationValueCodec>(
    input: &ValidatedSyntaxBundle<'_>,
    surface: &SchemaRef,
    category: Category,
    registry: &SchemaRegistry,
    b: &mut Budget,
    codec: &mut C,
) -> Result<DocumentSyntax, Error<C::Error>> {
    nepl3_doc_core::lower::document_with_decoder(
        input,
        surface,
        category,
        registry,
        b,
        &mut Decoder(codec),
    )
}

#[cfg(test)]
mod tests;
