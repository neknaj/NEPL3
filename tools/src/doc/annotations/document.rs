//! Selected Doc Inline rendering. Namespace composition is an explicit host step.
use nepl3_core::{
    budget::Budget,
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
    model::DocumentSyntax,
    portable::PortableError,
    prepare::{DocPreparationPlan, PreparationError},
};
use nepl3_doc_html::{LocalPreparationError, ParallelMode, RenderOptions, RenderedFragment};

#[derive(Debug)]
pub enum Error<E> {
    Syntax(SyntaxError),
    Lower(lower::DocumentLowerError<E>),
    Stopped(nepl3_core::budget::StopReason),
    Boundary(PortableError<E>),
    Structure(check::StructureError),
    ExpectedRoot(check::Category),
    Label(Box<Diagnostic>),
    LabelDiagnostic(LabelDiagnosticError<E>),
    NeedsResolution(DocPreparationPlan),
    LanguageOptions,
    MissingVariant { node: u64 },
    ListStart { node: u64, start: u64 },
    Render(nepl3_doc_html::RenderError),
}

pub(super) fn render<C: FoundationValueCodec>(
    closure: &ForeignClosure,
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<(DocumentSyntax, RenderedFragment), Error<C::Error>> {
    closure
        .validate(registry, b, codec.source_admission())
        .map_err(Error::Syntax)?;
    let input = closure
        .syntax
        .bundle
        .validate_with_sources(registry, b, codec.source_admission())
        .map_err(Error::Syntax)?;
    let document = lower::document(&input, surface, check::Category::Inline, registry, b, codec)
        .map_err(Error::Lower)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let prepared = nepl3_doc_html::prepare_local_inline(&document, &options, registry, codec, b)
        .map_err(|error| preparation_error(error, &document, registry, codec, b))?;
    let rendered = nepl3_doc_html::render_inline(&prepared, b).map_err(Error::Render)?;
    Ok((document, rendered))
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
