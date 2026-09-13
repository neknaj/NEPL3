//! Explicit host selection for Doc Sentence annotations. Source/Origin closure
//! validation precedes lowering; this adapter never evaluates an embedded Math.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Diagnostic,
    schema::SchemaRegistry,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{labels, lower, prepare};
use nepl3_doc_html::{LocalPreparationError, RenderOptions, RenderedFragment};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Syntax(SyntaxError),
    Lower(lower::DocumentLowerError<E>),
    Boundary(nepl3_doc_core::portable::PortableError<E>),
    Structure(nepl3_doc_core::check::StructureError),
    Label(Box<Diagnostic>),
    Diagnostic(labels::LabelDiagnosticError<E>),
    Root,
    NeedsResolution(prepare::DocPreparationPlan),
    Language,
    MissingVariant(u64),
    ListStart { node: u64, start: u64 },
    Render(nepl3_doc_html::RenderError),
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
pub struct RenderedAnnotation {
    pub document: nepl3_doc_core::model::DocumentSyntax,
    pub fragment: RenderedFragment,
}
/// No default guest selection: the complete schema identity and Sentence entry
/// must match. The returned origin mapping remains relative to the lowered Doc
/// input; callers retain the original closure for source-aware reporting.
pub struct DocAnnotationRenderer<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub options: &'a RenderOptions,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> DocAnnotationRenderer<'_, C> {
    pub fn render(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<RenderedAnnotation, Error<C::Error>> {
        let result = b.with_depth(|b| self.document(guest, b));
        b.poll()?;
        result
    }
    fn document(
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
        let syntax = guest
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let doc = lower::document(
            &syntax,
            self.surface,
            nepl3_doc_core::check::Category::Sentence,
            self.registry,
            b,
            self.codec,
        )
        .map_err(Error::Lower)?;
        let prepared = nepl3_doc_html::prepare_local_sentence(
            &doc,
            self.options,
            self.registry,
            self.codec,
            b,
        )
        .map_err(|e| match e {
            LocalPreparationError::Stopped(s) => Error::Stopped(s),
            LocalPreparationError::NeedsResolution(plan) => Error::NeedsResolution(plan),
            LocalPreparationError::Language => Error::Language,
            LocalPreparationError::MissingVariant { node } => Error::MissingVariant(node),
            LocalPreparationError::ListStart { node, start } => Error::ListStart { node, start },
            LocalPreparationError::Input(e) => match e {
                prepare::PreparationError::Stopped(s) => Error::Stopped(s),
                prepare::PreparationError::Boundary(e) => Error::Boundary(e),
                prepare::PreparationError::Label(e) => match e {
                    labels::LabelError::Stopped(s) => Error::Stopped(s),
                    labels::LabelError::Structure(e) => Error::Structure(e),
                    labels::LabelError::ExpectedArticle | labels::LabelError::ExpectedSentence => {
                        Error::Root
                    }
                    e => match e.diagnostic(&doc, self.registry, self.codec, b) {
                        Ok(diagnostic) => match b.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of::<Diagnostic>() as u64,
                        ) {
                            Ok(()) => Error::Label(Box::new(diagnostic)),
                            Err(s) => Error::Stopped(s),
                        },
                        Err(e) => Error::Diagnostic(e),
                    },
                },
            },
        })?;
        let fragment = nepl3_doc_html::render_sentence(&prepared, b).map_err(Error::Render)?;
        Ok(RenderedAnnotation {
            document: doc,
            fragment,
        })
    }
}
