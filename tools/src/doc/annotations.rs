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
    Math(Box<super::math::Error<E>>),
    Projection(super::math::ProjectionError),
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
    pub foreign: Vec<ForeignMathRecord>,
}
pub struct ForeignMathRecord {
    pub embed: nepl3_sentence_core::model::EmbedRef,
    pub syntax: nepl3_math_core::model::MathSyntax,
    pub node_roots: Vec<u64>,
    pub annotation_roots: Vec<nepl3_math_mathml::AnnotationRoot>,
    pub annotations: Vec<super::math::AnnotationRecord>,
}
impl ForeignMathRecord {
    pub fn remap(
        &mut self,
        map: &mut impl FnMut(u64) -> Result<u64, super::math::ProjectionError>,
        b: &mut Budget,
    ) -> Result<(), super::math::ProjectionError> {
        b.with_depth(|b| {
            for root in &mut self.node_roots {
                b.charge(Resource::Work, 1)?;
                *root = map(*root)?;
            }
            for root in &mut self.annotation_roots {
                b.charge(Resource::Work, 1)?;
                root.markup = map(root.markup)?;
            }
            for annotation in &mut self.annotations {
                for origin in &mut annotation.origins {
                    b.charge(Resource::Work, 1)?;
                    if origin.node >= annotation.sentence.value.nodes.len() as u64 {
                        return Err(super::math::ProjectionError::Mapping(origin.node));
                    }
                    origin.element = map(origin.element)?;
                }
                for foreign in &mut annotation.foreign {
                    foreign.remap(map, b)?;
                }
            }
            Ok(())
        })
    }
}

/// The full selected surface identity and Sentence entry must match. Element
/// indices refer to the returned markup and node indices to its owned Sentence.
/// The caller retains the original closure and remaps elements on composition.
pub struct SentenceAnnotationRenderer<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub math_surface: Option<&'a SchemaRef>,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> SentenceAnnotationRenderer<'_, C> {
    pub fn render(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        let mut ceiling = b.limits();
        ceiling.depth = ceiling.depth.min(64);
        let result = b.with_ceiling(ceiling, |b| b.with_depth(|b| self.sentence(guest, b)));
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
        let forms = self.math_surface.map(|surface| lower::ForeignInlineForm {
            kind: "Form:InlineMath",
            guest_schema: surface,
            guest_category: "Expr",
        });
        let sentence = lower::presentation::sentence_with_foreign(
            &input,
            self.surface,
            forms.as_slice(),
            self.registry,
            self.codec,
            b,
        )
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
        let mut foreign = Vec::new();
        // Admission is local to this immutable HTML validation. Guest operations
        // retain the codec's original ledger and cumulative Budget.
        let mut admission = nepl3_core::source::SourceAdmission::default();
        let rendered = html::render_with_foreign(
            &sentence,
            self.registry,
            &mut |closure, embed, b| {
                let math_surface = self.math_surface.ok_or(Error::Selection)?;
                let mut host = super::math::MathDisplayHost {
                    registry: self.registry,
                    math_surface,
                    sentence_surface: Some(self.surface),
                    codec: self.codec,
                };
                let result = host
                    .render(closure, nepl3_markup::mathml::Display::Inline, b)
                    .map_err(|error| {
                        match b.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of_val(&error) as u64,
                        ) {
                            Ok(()) => Error::Math(Box::new(error)),
                            Err(reason) => Error::Stopped(reason),
                        }
                    })?
                    .into_html(b)
                    .map_err(Error::Projection)?;
                b.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<ForeignMathRecord>() as u64).saturating_mul(2),
                )?;
                foreign.push(ForeignMathRecord {
                    embed,
                    syntax: result.syntax,
                    node_roots: result.node_roots,
                    annotation_roots: result.annotation_roots,
                    annotations: result.annotations,
                });
                Ok::<_, Error<C::Error>>(result.markup)
            },
            b,
            &mut admission,
        )
        .map_err(|error| match error {
            html::RenderFailure::Sentence(error) => Error::Render(error),
            html::RenderFailure::Foreign(error) => error,
        })?;
        let (_, markup, origins, placements) = rendered.into_parts();
        if placements.len() != foreign.len() {
            return Err(Error::Selection);
        }
        for (record, placement) in foreign.iter_mut().zip(placements) {
            if record.embed != placement.embed {
                return Err(Error::Selection);
            }
            record
                .remap(
                    &mut |element| {
                        if element >= placement.elements {
                            return Err(super::math::ProjectionError::Mapping(element));
                        }
                        placement
                            .first_element
                            .checked_add(element)
                            .ok_or(super::math::ProjectionError::Mapping(element))
                    },
                    b,
                )
                .map_err(Error::Projection)?;
        }
        Ok(RenderedAnnotation {
            sentence,
            sentence_digest,
            markup,
            origins,
            foreign,
        })
    }
}
