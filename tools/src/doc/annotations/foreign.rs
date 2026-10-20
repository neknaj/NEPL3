//! One dispatch and provenance path for standalone annotations and paragraph parts.
use super::*;
use nepl3_core::syntax::ForeignClosureView;
use nepl3_doc_core::labels::namespace::MemberId;
use nepl3_sentence_core::model::EmbedRef;

pub(super) fn render<C: FoundationValueCodec>(
    closure: &ForeignClosureView<'_>,
    embed: EmbedRef,
    selected: Option<(&document::namespace::Selected, MemberId)>,
    prepared: document::Preparation<'_, '_>,
    host: &mut SentenceAnnotationRenderer<'_, C>,
    b: &mut Budget,
) -> Result<(nepl3_markup::html::HtmlRequest, ForeignRecord), Error<C::Error>> {
    if let Some(surface) = host.doc_surface {
        b.charge(
            Resource::Work,
            (surface.package.len()
                + closure.syntax().schema.package.len()
                + closure.syntax().category.len()) as u64
                + 70,
        )?;
        if &closure.syntax().schema == surface && closure.syntax().category == "Inline" {
            let (selected, member) = selected.ok_or(Error::Selection)?;
            if selected.embed != embed {
                return Err(Error::Selection);
            }
            // The actual HTML ancestors can be deeper than namespace collection.
            closure
                .validate(host.registry, b, host.codec.source_admission())
                .map_err(Error::Syntax)?;
            let (rendered, nested) = document::render_member(prepared, member, host, b)
                .map_err(|error| document_error(error, b))?;
            let (_, document_digest, markup, origins) = rendered.into_parts();
            return Ok((
                markup,
                ForeignRecord::Document(ForeignDocumentRecord {
                    embed,
                    document: std::sync::Arc::clone(&selected.document),
                    document_digest,
                    origins,
                    foreign: nested,
                }),
            ));
        }
    }
    let mut math = super::super::math::MathDisplayHost {
        registry: host.registry,
        math_surface: host.math_surface.ok_or(Error::Selection)?,
        sentence_surface: Some(host.surface),
        doc_surface: host.doc_surface,
        codec: host.codec,
    };
    let result = math
        .render_view(closure, nepl3_markup::mathml::Display::Inline, b)
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
    Ok((
        result.markup,
        ForeignRecord::Math(ForeignMathRecord {
            embed,
            output: MathRecord {
                syntax: result.syntax,
                node_roots: result.node_roots,
                annotation_roots: result.annotation_roots,
                annotations: result.annotations,
            },
        }),
    ))
}

pub(super) fn place<E>(
    record: &mut ForeignRecord,
    placement: &html::ForeignPlacement,
    b: &mut Budget,
) -> Result<(), Error<E>> {
    if record.embed() != placement.embed {
        return Err(Error::Selection);
    }
    record
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
        .map_err(Error::Projection)
}
