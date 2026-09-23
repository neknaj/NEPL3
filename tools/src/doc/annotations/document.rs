//! Selected Doc Inline rendering. Namespace composition is an explicit host step.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Diagnostic,
    schema::SchemaRegistry,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{
    check,
    labels::{LabelDiagnosticError, LabelError},
    lower,
    model::{DocumentSyntax, EmbedKind},
    portable::PortableError,
    prepare::{DocPreparationPlan, PreparationError},
};
use nepl3_doc_html::LocalPreparationError;
use std::sync::Arc;

pub mod namespace;

#[derive(Debug)]
pub struct NamespaceOwner {
    pub member: nepl3_doc_core::labels::namespace::MemberId,
    pub node: u64,
    pub document: Arc<DocumentSyntax>,
}

#[derive(Debug)]
pub enum Error<E> {
    Selection,
    Math(Box<super::super::math::Error<E>>),
    Projection(super::super::math::ProjectionError),
    Syntax(SyntaxError),
    SentenceShape(nepl3_sentence_core::check::Error),
    Lower(lower::DocumentLowerError<E>),
    Stopped(nepl3_core::budget::StopReason),
    Boundary(PortableError<E>),
    Structure(check::StructureError),
    ExpectedRoot(check::Category),
    Label(Box<Diagnostic>),
    LabelDiagnostic(LabelDiagnosticError<E>),
    NamespaceDuplicate {
        definition: NamespaceOwner,
        previous: NamespaceOwner,
        diagnostic: Box<Diagnostic>,
    },
    NeedsResolution(DocPreparationPlan),
    LanguageOptions,
    MissingVariant {
        node: u64,
    },
    ListStart {
        node: u64,
        start: u64,
    },
    Render(nepl3_doc_html::RenderError),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

pub(super) fn lower_inline<C: FoundationValueCodec>(
    closure: &ForeignClosure,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<DocumentSyntax, Error<C::Error>> {
    closure
        .validate(registry, b, codec.source_admission())
        .map_err(Error::Syntax)?;
    let input = closure
        .syntax
        .bundle
        .validate_with_sources(registry, b, codec.source_admission())
        .map_err(Error::Syntax)?;
    lower::document(&input, surface, check::Category::Inline, registry, b, codec)
        .map_err(Error::Lower)
}

pub(super) fn render_member<C: FoundationValueCodec>(
    prepared: &nepl3_doc_html::namespace::PreparedForeignNamespace<'_>,
    member: nepl3_doc_core::labels::namespace::MemberId,
    host: &mut super::SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<
    (
        nepl3_doc_html::namespace::PendingPart,
        Vec<super::DocumentMathRecord>,
    ),
    Error<C::Error>,
> {
    let mut foreign = Vec::new();
    let rendered = nepl3_doc_html::namespace::render_part_with_foreign(
        prepared,
        member,
        &mut |guest, embed, b| {
            if guest.kind != EmbedKind::InlineMath {
                return Err(Error::Selection);
            }
            let mut host = super::super::math::MathDisplayHost {
                registry: host.registry,
                math_surface: host.math_surface.ok_or(Error::Selection)?,
                sentence_surface: Some(host.surface),
                doc_surface: host.doc_surface,
                codec: host.codec,
            };
            let result = host
                .render(&guest.closure, nepl3_markup::mathml::Display::Inline, b)
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
                core::mem::size_of::<super::DocumentMathRecord>() as u64,
            )?;
            foreign
                .try_reserve_exact(1)
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
            foreign.push(super::DocumentMathRecord {
                embed,
                output: super::MathRecord {
                    syntax: result.syntax,
                    node_roots: result.node_roots,
                    annotation_roots: result.annotation_roots,
                    annotations: result.annotations,
                },
            });
            Ok(result.markup)
        },
        b,
    )
    .map_err(|error| match error {
        nepl3_doc_html::namespace::ForeignPartError::Member(_) => Error::Selection,
        nepl3_doc_html::namespace::ForeignPartError::Render(
            nepl3_doc_html::ForeignRenderError::Render(error),
        ) => Error::Render(error),
        nepl3_doc_html::namespace::ForeignPartError::Render(
            nepl3_doc_html::ForeignRenderError::Foreign(error),
        ) => error,
    })?;
    if foreign.len() != rendered.foreign.len() {
        return Err(Error::Selection);
    }
    for (record, placement) in foreign.iter_mut().zip(&rendered.foreign) {
        b.charge(Resource::Work, 1)?;
        if record.embed != placement.embed {
            return Err(Error::Selection);
        }
        record
            .output
            .remap(
                &mut |element| {
                    if element >= placement.elements {
                        return Err(super::super::math::ProjectionError::Mapping(element));
                    }
                    placement
                        .first_element
                        .checked_add(element)
                        .ok_or(super::super::math::ProjectionError::Mapping(element))
                },
                b,
            )
            .map_err(Error::Projection)?;
    }
    Ok((rendered.part, foreign))
}

fn preparation_error<C: FoundationValueCodec>(
    error: LocalPreparationError<'_, C::Error>,
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Error<C::Error> {
    match error {
        LocalPreparationError::Stopped(reason) => Error::Stopped(reason),
        LocalPreparationError::NeedsResolution(plan) => Error::NeedsResolution(plan),
        LocalPreparationError::Language => Error::LanguageOptions,
        LocalPreparationError::MissingVariant { node } => Error::MissingVariant { node },
        LocalPreparationError::ListStart { node, start } => Error::ListStart { node, start },
        LocalPreparationError::Input(error) => match error {
            PreparationError::Stopped(reason) => Error::Stopped(reason),
            PreparationError::Boundary(error) => Error::Boundary(error),
            PreparationError::Label(error) => match error {
                LabelError::Stopped(reason) => Error::Stopped(reason),
                LabelError::Structure(error) => Error::Structure(error),
                LabelError::ExpectedArticle => Error::ExpectedRoot(check::Category::Article),
                LabelError::ExpectedSentence => Error::ExpectedRoot(check::Category::Sentence),
                LabelError::ExpectedInline => Error::ExpectedRoot(check::Category::Inline),
                error @ (LabelError::Duplicate { .. }
                | LabelError::DuplicateOccurrence { .. }
                | LabelError::Unresolved { .. }) => {
                    match error.diagnostic(document, registry, codec, b) {
                        Ok(diagnostic) => match b.charge(
                            nepl3_core::budget::Resource::AllocationUnits,
                            core::mem::size_of::<Diagnostic>() as u64,
                        ) {
                            Ok(()) => Error::Label(Box::new(diagnostic)),
                            Err(reason) => Error::Stopped(reason),
                        },
                        Err(error) => Error::LabelDiagnostic(error),
                    }
                }
            },
        },
    }
}
