//! Math display selected by the Doc host. This prepares one real embedded
//! expression; it does not grant a PreparedArticle or emit a document shell.
use super::annotations::{DocAnnotationRenderer, RenderedAnnotation};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::{DocKind, DocumentSyntax};
use nepl3_markup::mathml::Display;
use nepl3_math_core::{check, lower, model::MathSyntax};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Node(u64),
    Document(nepl3_doc_core::check::StructureError),
    Syntax(SyntaxError),
    Lower(lower::LowerError),
    Check(check::ShapeError),
    Render(nepl3_math_mathml::Error),
    Annotation(nepl3_math_mathml::AnnotationFailure<super::annotations::Error<E>>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
pub struct AnnotationRecord {
    /// Math embed index, not a Doc node or global Origin ID.
    pub embed: u64,
    pub document: DocumentSyntax,
    pub document_digest: nepl3_core::source::Digest,
    pub options: nepl3_doc_html::RenderOptions,
    pub origins: Vec<nepl3_doc_html::ElementOrigin>,
}
pub struct RenderedMath {
    pub syntax: MathSyntax,
    pub rendered: nepl3_math_mathml::Rendered,
    pub annotations: Vec<AnnotationRecord>,
}
/// Explicit semantic selections; no evaluation, network or implicit renderer.
/// The declared Math surface must match the complete closure schema identity.
pub struct MathDisplayHost<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub math_surface: &'a SchemaRef,
    pub doc_surface: Option<&'a SchemaRef>,
    pub doc_options: &'a nepl3_doc_html::RenderOptions,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> MathDisplayHost<'_, C> {
    /// Validate the Doc source closure, then render only InlineMath/DisplayMath.
    /// Code is deliberately rejected: displaying code is a different operation.
    /// This does not resolve the containing Article's labels, links or assets.
    pub fn render_node(
        &mut self,
        document: &DocumentSyntax,
        node: u64,
        b: &mut Budget,
    ) -> Result<RenderedMath, Error<C::Error>> {
        let result = (|| {
            document
                .validate_structure(self.registry, b, self.codec.source_admission())
                .map_err(Error::Document)?;
            let kind = &document
                .value
                .nodes
                .get(usize::try_from(node).map_err(|_| Error::Node(node))?)
                .ok_or(Error::Node(node))?
                .kind;
            let (embed, display) = match kind {
                DocKind::InlineMath { syntax } => (*syntax, Display::Inline),
                DocKind::DisplayMath { syntax } => (*syntax, Display::Block),
                _ => return Err(Error::Node(node)),
            };
            let guest = document
                .value
                .embeds
                .get(embed.0 as usize)
                .ok_or(Error::Node(node))?;
            self.render(&guest.closure, display, b)
        })();
        b.poll()?;
        result
    }
    /// Retains the actual lowered Math input and each lowered Doc annotation for
    /// interpreting the backend's node-root/origin mappings after this call.
    pub fn render(
        &mut self,
        guest: &ForeignClosure,
        display: Display,
        b: &mut Budget,
    ) -> Result<RenderedMath, Error<C::Error>> {
        let result = b.with_depth(|b| self.expression(guest, display, b));
        b.poll()?;
        result
    }
    fn expression(
        &mut self,
        guest: &ForeignClosure,
        display: Display,
        b: &mut Budget,
    ) -> Result<RenderedMath, Error<C::Error>> {
        b.charge(
            Resource::Work,
            (guest.syntax.schema.package.len() as u64)
                .saturating_add(self.math_surface.package.len() as u64)
                .saturating_add(guest.syntax.category.len() as u64)
                .saturating_add(64),
        )?;
        if &guest.syntax.schema != self.math_surface || guest.syntax.category != "Expr" {
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
        let syntax = lower::expression(
            &input,
            self.math_surface,
            check::Category::Expr,
            self.registry,
            b,
            self.codec.source_admission(),
        )
        .map_err(Error::Lower)?;
        let checked = check::expression(&syntax.value, b).map_err(Error::Check)?;
        let mut annotations = Vec::new();
        let rendered = if let Some(surface) = self.doc_surface {
            let mut host = DocAnnotationRenderer {
                registry: self.registry,
                surface,
                options: self.doc_options,
                codec: self.codec,
            };
            nepl3_math_mathml::render_with_annotations(
                &checked,
                display,
                &mut |closure, b| {
                    let mut embed = None;
                    for (i, candidate) in syntax.value.embeds.iter().enumerate() {
                        b.charge(Resource::Work, 1)?;
                        if core::ptr::eq(candidate, closure) {
                            embed = Some(i as u64);
                            break;
                        }
                    }
                    let embed = embed.ok_or(super::annotations::Error::Selection)?;
                    let result = host.render(closure, b)?;
                    // Move the rendered tree into MathML while keeping its provenance.
                    let RenderedAnnotation { document, fragment } = result;
                    let nepl3_doc_html::RenderedFragment {
                        document_digest,
                        options,
                        markup,
                        origins,
                    } = fragment;
                    // Metadata stays separate from the consumed markup; no duplicate
                    // tree is allocated merely to retain the annotation's origin map.
                    b.charge(
                        Resource::AllocationUnits,
                        (core::mem::size_of::<AnnotationRecord>() as u64).saturating_mul(2),
                    )?;
                    annotations.push(AnnotationRecord {
                        embed,
                        document,
                        document_digest,
                        options,
                        origins,
                    });
                    Ok::<_, super::annotations::Error<C::Error>>(markup)
                },
                b,
            )
            .map_err(Error::Annotation)?
        } else {
            nepl3_math_mathml::render(&checked, display, b).map_err(Error::Render)?
        };
        Ok(RenderedMath {
            syntax,
            rendered,
            annotations,
        })
    }
}
