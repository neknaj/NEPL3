//! Explicit host selection of independent Sentence annotations for Math output.
//! Source closure validation precedes lowering and no guest operation executes.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::Digest,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};
use nepl3_sentence_core::{lower, portable, syntax::SentenceSyntax};
use nepl3_suite::adapters::sentence::html;

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Syntax(SyntaxError),
    Lower(lower::presentation::Error<E>),
    Portable(portable::Error<E>),
    Foundation(E),
    Render(html::Error),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

pub struct RenderedAnnotation {
    pub sentence: SentenceSyntax,
    pub sentence_digest: Digest,
    pub markup: nepl3_markup::html::HtmlRequest,
    pub origins: Vec<html::ElementOrigin>,
}

/// The full selected surface identity and Sentence entry must match. Element
/// indices refer to the returned markup and node indices to its owned Sentence.
/// The caller retains the original closure and remaps elements on composition.
pub struct SentenceAnnotationRenderer<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> SentenceAnnotationRenderer<'_, C> {
    pub fn render(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        let result = b.with_depth(|b| self.sentence(guest, b));
        b.poll()?;
        result
    }

    fn sentence(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        b.charge(
            Resource::Work,
            (guest.syntax.schema.package.len() as u64)
                .saturating_add(self.surface.package.len() as u64)
                .saturating_add(guest.syntax.category.len() as u64)
                .saturating_add(64),
        )?;
        if &guest.syntax.schema != self.surface || guest.syntax.category != "Sentence" {
            return Err(Error::Selection);
        }
        guest
            .validate(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let input = guest
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let sentence =
            lower::presentation::sentence(&input, self.surface, self.registry, self.codec, b)
                .map_err(Error::Lower)?;
        let raw = portable::syntax::to_value(&sentence, self.registry, self.codec, b)
            .map_err(Error::Portable)?;
        let sentence_digest = self
            .codec
            .canonical_value_digest(b"NEPL3.Math.Annotation.Sentence.v1\0", &raw, b)
            .map_err(|error| match error.stop_reason() {
                Some(reason) => Error::Stopped(reason),
                None => Error::Foundation(error),
            })?;
        let (_, markup, origins) =
            html::render(&sentence, self.registry, b, self.codec.source_admission())
                .map_err(Error::Render)?
                .into_parts();
        Ok(RenderedAnnotation {
            sentence,
            sentence_digest,
            markup,
            origins,
        })
    }
}
