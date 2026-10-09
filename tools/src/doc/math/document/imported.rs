use super::*;
use crate::doc::{
    annotations::MathRecord,
    math::{
        assets::PreparedAssets,
        display::{
            TexPreparation,
            generation::composite::{
                FallbackReason, GeneratedRange, Representation, import::Metadata,
            },
            process::{Config, reply::Observations},
            request::Controls,
        },
    },
};
use nepl3_doc_html::ForeignPlacement;
use nepl3_markup::mathml::Display;

/// Source maps and generated ranges refer to the containing LocalDocument arena.
/// The source Math/Sentence/Doc structures remain owned and unchanged.
/// ```compile_fail
/// fn duplicate(d: nepl3_tools::doc::math::document::Imported<'_, '_, '_>) { d.clone(); }
/// ```
/// ```compile_fail
/// fn detach(d: nepl3_tools::doc::math::document::Imported<'_, '_, '_>) { d.into_parts(); }
/// ```
/// ```compile_fail
/// fn mutate(d: &mut nepl3_tools::doc::math::document::Imported<'_, '_, '_>) { d.source.node_roots.clear(); }
/// ```
pub struct Imported<'d, 'c, 'r> {
    pub(super) context: guests::Context<'d>,
    pub(super) source: MathRecord,
    pub(super) metadata: Metadata<'c, 'r>,
    pub(super) original_nodes: u64,
    pub(super) placement: Option<ForeignPlacement>,
}
impl<'d, 'c, 'r> Imported<'d, 'c, 'r> {
    pub fn context(&self) -> &guests::Context<'d> {
        &self.context
    }
    pub fn source(&self) -> &MathRecord {
        &self.source
    }
    pub fn scope(&self) -> &str {
        &self.metadata.scope
    }
    pub fn stylesheet(&self) -> &str {
        &self.metadata.stylesheet
    }
    pub fn assets(&self) -> Option<&PreparedAssets> {
        self.metadata.assets
    }
    pub fn display(&self) -> Display {
        self.metadata.display
    }
    pub fn tex(&self) -> &TexPreparation {
        &self.metadata.tex
    }
    pub fn representation(&self) -> Representation {
        self.metadata.representation
    }
    pub fn generated(&self) -> Option<GeneratedRange> {
        self.metadata.generated
    }
    /// None records an explicit no-renderer path, not a missing observation.
    pub fn selected_config(&self) -> Option<Config<'c>> {
        self.metadata
            .selection
            .as_ref()
            .map(|selected| selected.config)
    }
    pub fn controls(&self) -> Option<Controls> {
        self.metadata
            .selection
            .as_ref()
            .map(|selected| selected.controls)
    }
    pub fn observations(&self) -> Option<&Observations> {
        self.metadata.observations.as_ref()
    }
    pub fn fallback(&self) -> Option<&FallbackReason> {
        self.metadata.fallback.as_ref()
    }
    pub fn termination_failure(&self) -> Option<bool> {
        self.metadata.termination_failure
    }
    pub fn placement(&self) -> Option<ForeignPlacement> {
        self.placement
    }
    pub(super) fn bind<E>(
        &mut self,
        rendered: &RenderedInlineWithForeign,
        b: &mut Budget,
    ) -> Result<(), Error<E>> {
        b.charge(Resource::Work, 40)?;
        if rendered.fragment.document_digest != self.context.document_digest()
            || self.metadata.doc_node != self.context.node()
        {
            return Err(Error::Association);
        }
        let placement = usize::try_from(self.context.ordinal())
            .ok()
            .and_then(|n| rendered.foreign.get(n))
            .copied()
            .ok_or(Error::Association)?;
        if placement.embed != self.context.reference()
            || placement.elements != self.original_nodes
            || placement
                .first_element
                .checked_add(placement.elements)
                .is_none_or(|end| end > rendered.fragment.markup.fragment.nodes.len() as u64)
        {
            return Err(Error::Association);
        }
        let mut map = |n| {
            if n >= self.original_nodes {
                return Err(ProjectionError::Mapping(n));
            }
            placement
                .first_element
                .checked_add(n)
                .ok_or(ProjectionError::Mapping(n))
        };
        self.source.remap(&mut map, b).map_err(|e| match e {
            ProjectionError::Stopped(s) => Error::Stopped(s),
            e => Error::Mapping(e),
        })?;
        if let Some(range) = &mut self.metadata.generated {
            b.charge(Resource::Work, 4)?;
            if range.first.checked_add(range.elements) != Some(self.original_nodes) {
                return Err(Error::Association);
            }
            range.first = map(range.first).map_err(Error::Mapping)?;
            range.visual_root = map(range.visual_root).map_err(Error::Mapping)?;
            range.accessible_root = map(range.accessible_root).map_err(Error::Mapping)?;
        }
        self.placement = Some(placement);
        Ok(())
    }
}
