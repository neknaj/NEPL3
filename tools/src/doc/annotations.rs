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
pub mod article;
pub mod document;
mod foreign;
pub mod paragraph;

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Placed(Box<nepl3_suite::adapters::sentence::document_guests::PlacedError<E>>),
    Syntax(SyntaxError),
    Lower(lower::presentation::Error<E>),
    Portable(portable::Error<E>),
    Foundation(E),
    Render(html::Error),
    Math(Box<super::math::Error<E>>),
    Projection(super::math::ProjectionError),
    Document(Box<document::Error<E>>),
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
    pub foreign: Vec<ForeignRecord>,
}
pub enum ForeignRecord {
    Math(ForeignMathRecord),
    Document(ForeignDocumentRecord),
}
pub struct ForeignDocumentRecord {
    pub embed: nepl3_sentence_core::model::EmbedRef,
    pub document: std::sync::Arc<nepl3_doc_core::model::DocumentSyntax>,
    pub document_digest: Digest,
    pub origins: Vec<nepl3_doc_html::ElementOrigin>,
    pub foreign: Vec<DocumentMathRecord>,
}
pub struct DocumentMathRecord {
    pub embed: nepl3_doc_core::model::EmbedRef,
    pub output: MathRecord,
}
impl ForeignRecord {
    pub fn embed(&self) -> nepl3_sentence_core::model::EmbedRef {
        match self {
            Self::Math(record) => record.embed,
            Self::Document(record) => record.embed,
        }
    }
    pub fn remap(
        &mut self,
        map: &mut impl FnMut(u64) -> Result<u64, super::math::ProjectionError>,
        b: &mut Budget,
    ) -> Result<(), super::math::ProjectionError> {
        match self {
            Self::Math(record) => record.output.remap(map, b),
            Self::Document(record) => {
                for origin in &mut record.origins {
                    b.charge(Resource::Work, 1)?;
                    if origin.node >= record.document.value.nodes.len() as u64 {
                        return Err(super::math::ProjectionError::Mapping(origin.node));
                    }
                    origin.element = map(origin.element)?;
                }
                for foreign in &mut record.foreign {
                    foreign.output.remap(map, b)?;
                }
                Ok(())
            }
        }
    }
}
pub struct ForeignMathRecord {
    pub embed: nepl3_sentence_core::model::EmbedRef,
    pub output: MathRecord,
}
pub struct MathRecord {
    pub syntax: nepl3_math_core::model::MathSyntax,
    pub node_roots: Vec<u64>,
    pub annotation_roots: Vec<nepl3_math_mathml::AnnotationRoot>,
    pub annotations: Vec<super::math::AnnotationRecord>,
}
impl MathRecord {
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
    pub doc_surface: Option<&'a SchemaRef>,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> SentenceAnnotationRenderer<'_, C> {
    /// Render an explicitly supplied typed Sentence. Its complete source and
    /// model boundary is revalidated before namespace preparation or rendering.
    pub fn render_syntax(
        &mut self,
        sentence: SentenceSyntax,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        let mut ceiling = b.limits();
        ceiling.depth = ceiling.depth.min(64);
        let result = b.with_ceiling(ceiling, |b| {
            b.with_depth(|b| self.render_sentence(sentence, b))
        });
        b.poll()?;
        result
    }
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
        let sentence = self.lower_view(&guest.view(), b)?;
        self.render_sentence(sentence, b)
    }

    pub(crate) fn lower_view(
        &mut self,
        guest: &nepl3_core::syntax::ForeignClosureView<'_>,
        b: &mut Budget,
    ) -> Result<SentenceSyntax, Error<C::Error>> {
        b.charge(
            Resource::Work,
            (guest.syntax().schema.package.len() as u64)
                .saturating_add(self.surface.package.len() as u64)
                .saturating_add(guest.syntax().category.len() as u64)
                .saturating_add(64),
        )?;
        if &guest.syntax().schema != self.surface || guest.syntax().category != "Sentence" {
            return Err(Error::Selection);
        }
        let input = guest
            .validate_with_bundle(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let mut forms = Vec::new();
        for (kind, category, surface) in [
            ("Form:InlineMath", "Expr", self.math_surface),
            ("Form:DocumentInline", "Inline", self.doc_surface),
        ] {
            if let Some(surface) = surface {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<lower::ForeignInlineForm<'_>>() as u64,
                )?;
                forms
                    .try_reserve_exact(1)
                    .map_err(|_| b.stop(StopReason::AllocationLimit))?;
                forms.push(lower::ForeignInlineForm {
                    kind,
                    guest_schema: surface,
                    guest_category: category,
                });
            }
        }
        let sentence = lower::presentation::sentence_with_foreign(
            &input,
            self.surface,
            forms.as_slice(),
            self.registry,
            self.codec,
            b,
        )
        .map_err(Error::Lower)?;
        Ok(sentence)
    }

    fn render_sentence(
        &mut self,
        sentence: SentenceSyntax,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        if !matches!(
            sentence.value.root,
            nepl3_sentence_core::model::Root::Sentence(_)
        ) {
            return Err(Error::Selection);
        }
        let raw = portable::syntax::to_value(&sentence, self.registry, self.codec, b)
            .map_err(Error::Portable)?;
        let sentence_digest = self
            .codec
            .canonical_value_digest(b"NEPL3.Math.Annotation.Sentence.v1\0", &raw, b)
            .map_err(|error| match error.stop_reason() {
                Some(reason) => Error::Stopped(reason),
                None => Error::Foundation(error),
            })?;
        let selected =
            document::namespace::collect(&sentence, self.doc_surface, self.registry, self.codec, b)
                .map_err(|error| document_error(error, b))?;
        document::namespace::check_content(
            &sentence,
            &selected,
            self.math_surface,
            self.registry,
            self.codec,
            b,
        )
        .map_err(|error| document_error(error, b))?;
        let members = document::namespace::inspect(&selected, self.registry, self.codec, b)
            .map_err(|error| document_error(error, b))?;
        let member_refs = document::namespace::member_refs(&members, b)?;
        let namespace =
            nepl3_doc_core::labels::namespace::resolve(&member_refs, b).map_err(|error| {
                let error = document::namespace::resolution_error(
                    error,
                    &selected,
                    &member_refs,
                    self.registry,
                    self.codec,
                    b,
                );
                document_error(error, b)
            })?;
        let options = nepl3_doc_html::RenderOptions {
            parallel: nepl3_doc_html::ParallelMode::Rows,
        };
        let prepared =
            document::namespace::prepare(&namespace, &options, self.registry, self.codec, b)
                .map_err(|error| document_error(error, b))?;
        let mut document_position = 0usize;
        let mut foreign = Vec::new();
        let registry = self.registry;
        let surface = self.surface;
        let math_surface = self.math_surface;
        let doc_surface = self.doc_surface;
        let rendered = html::render_with_foreign_codec(
            &sentence,
            registry,
            self.codec,
            &mut |closure, embed, codec, b| {
                let candidate = selected.get(document_position).map(|selected| {
                    (
                        selected,
                        nepl3_doc_core::labels::namespace::MemberId(document_position as u64),
                    )
                });
                let (markup, record) = foreign::render(
                    closure,
                    embed,
                    candidate,
                    document::Preparation::Inline(&prepared),
                    &mut SentenceAnnotationRenderer {
                        registry,
                        surface,
                        math_surface,
                        doc_surface,
                        codec,
                    },
                    b,
                )?;
                if matches!(&record, ForeignRecord::Document(_)) {
                    document_position += 1;
                }
                b.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<ForeignRecord>() as u64).saturating_mul(2),
                )?;
                foreign.push(record);
                Ok::<_, Error<C::Error>>(markup)
            },
            b,
        )
        .map_err(|error| match error {
            html::RenderFailure::Sentence(error) => Error::Render(error),
            html::RenderFailure::Foreign(error) => error,
        })?;
        let (_, markup, origins, placements) = rendered.into_parts();
        if placements.len() != foreign.len() || document_position != selected.len() {
            return Err(Error::Selection);
        }
        for (record, placement) in foreign.iter_mut().zip(placements) {
            foreign::place(record, &placement, b)?;
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

fn document_error<E>(error: document::Error<E>, b: &mut Budget) -> Error<E> {
    match error {
        document::Error::Stopped(reason) => Error::Stopped(reason),
        error => match b.charge(
            Resource::AllocationUnits,
            core::mem::size_of_val(&error) as u64,
        ) {
            Ok(()) => Error::Document(Box::new(error)),
            Err(reason) => Error::Stopped(reason),
        },
    }
}
