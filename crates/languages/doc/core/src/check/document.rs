use super::{ShapeError, ValidatedDocShape, edges};
use crate::model::{DocKind, DocumentSyntax};
use alloc::vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{OriginError, OriginGraph, SourceMap},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceError, SourceStore},
    syntax::SyntaxError,
    view::ViewError,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StructureError {
    Stopped(StopReason),
    Shape(ShapeError),
    Source(SourceError),
    Origin(OriginError),
    View(ViewError),
    Syntax(SyntaxError),
    DuplicateSource,
    OriginReference(u64),
    ViewOwner,
    FieldPosition(u64),
}
impl From<StopReason> for StructureError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<ShapeError> for StructureError {
    fn from(v: ShapeError) -> Self {
        match v {
            ShapeError::Stopped(s) => Self::Stopped(s),
            v => Self::Shape(v),
        }
    }
}
impl From<SourceError> for StructureError {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(s) => Self::Stopped(s),
            v => Self::Source(v),
        }
    }
}
impl From<OriginError> for StructureError {
    fn from(v: OriginError) -> Self {
        match v {
            OriginError::Stopped(s) | OriginError::Source(SourceError::Stopped(s)) => {
                Self::Stopped(s)
            }
            v => Self::Origin(v),
        }
    }
}
impl From<ViewError> for StructureError {
    fn from(v: ViewError) -> Self {
        match v.stop_reason() {
            Some(s) => Self::Stopped(s),
            None => Self::View(v),
        }
    }
}
impl From<SyntaxError> for StructureError {
    fn from(v: SyntaxError) -> Self {
        match v.stop_reason() {
            Some(s) => Self::Stopped(s),
            None => Self::Syntax(v),
        }
    }
}
/// Source, view, origin and guest syntax safety; this is not label resolution,
/// guest semantic checking, or permission to render an unprepared Article.
pub struct ValidatedDocumentSyntax<'a> {
    document: &'a DocumentSyntax,
    shape: ValidatedDocShape<'a>,
}
impl<'a> ValidatedDocumentSyntax<'a> {
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub fn shape(&self) -> &ValidatedDocShape<'a> {
        &self.shape
    }
}
impl DocumentSyntax {
    pub fn validate_structure<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedDocumentSyntax<'a>, StructureError> {
        b.poll()?;
        if !registry.is_finalized() {
            return Err(SyntaxError::from(nepl3_core::schema::SchemaError::Unfinalized).into());
        }
        let shape = self.value.validate_shape(b)?;
        let mut store = SourceStore::default();
        for (i, source) in self.sources.iter().enumerate() {
            admission.admit_existing(source, b)?;
            for prior in &self.sources[..i] {
                b.charge(
                    Resource::Work,
                    (prior.identity().source.0.len() + source.identity().source.0.len()) as u64
                        + 40,
                )?;
                if prior.identity().source == source.identity().source
                    && prior.identity().revision == source.identity().revision
                {
                    return Err(StructureError::DuplicateSource);
                }
            }
            store.insert_with_budget(source.clone_with_budget(b)?, b)?;
        }
        OriginGraph::validate_origins(&self.origins, &store, b)?;
        let maps = SourceMap::validate_mappings(&self.source_maps, &store, b)?;
        for (index, node) in self.value.nodes.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if let Some(origin) = node.origin
                && usize::try_from(origin.0)
                    .ok()
                    .is_none_or(|i| i >= self.origins.len())
            {
                return Err(StructureError::OriginReference(origin.0));
            }
            if let Some(span) = &node.span {
                b.charge(
                    Resource::Work,
                    span.snapshot_ref().source.0.len() as u64 + 40,
                )?;
                store
                    .get_ref(span.snapshot_ref())
                    .ok_or(SourceError::MissingSnapshot)?
                    .slice(span)?;
            }
            for location in &node.locations {
                b.charge(Resource::Work, 1)?;
                if let Some(origin) = location.origin
                    && usize::try_from(origin.0)
                        .ok()
                        .is_none_or(|id| id >= self.origins.len())
                {
                    return Err(StructureError::OriginReference(origin.0));
                }
                if let Some(span) = &location.span {
                    b.charge(
                        Resource::Work,
                        span.snapshot_ref().source.0.len() as u64 + 40,
                    )?;
                    store
                        .get_ref(span.snapshot_ref())
                        .ok_or(SourceError::MissingSnapshot)?
                        .slice(span)?;
                    if let Some(cover) = &node.span
                        && !maps.contains(cover, span, b)?
                    {
                        return Err(StructureError::FieldPosition(index as u64));
                    }
                    if let Some(origin) = location.origin {
                        // The selected bytes must be supported by an explicit
                        // cause. Composite/Generated arenas remain iterative.
                        let mut pending = alloc::vec::Vec::new();
                        b.charge(Resource::AllocationUnits, 8)?;
                        pending.push(origin);
                        let mut found = false;
                        while let Some(origin) = pending.pop() {
                            b.charge(Resource::Work, 1)?;
                            let (position, parents) = match &self.origins[origin.0 as usize] {
                                nepl3_core::origin::Origin::Direct(span) => (Some(span), &[][..]),
                                nepl3_core::origin::Origin::Composite(parents) => {
                                    (None, parents.as_slice())
                                }
                                nepl3_core::origin::Origin::Generated {
                                    callsite, inputs, ..
                                } => (callsite.as_ref(), inputs.as_slice()),
                                nepl3_core::origin::Origin::Synthetic { anchor, .. } => {
                                    (anchor.as_ref(), &[][..])
                                }
                            };
                            if let Some(position) = position
                                && maps.contains(position, span, b)?
                            {
                                found = true;
                                break;
                            }
                            for parent in parents {
                                b.charge(Resource::Work, 1)?;
                                b.charge(Resource::AllocationUnits, 8)?;
                                pending.push(*parent);
                            }
                        }
                        if !found {
                            return Err(StructureError::FieldPosition(index as u64));
                        }
                    }
                }
            }
        }
        for view in &self.views {
            b.charge(
                Resource::Work,
                view.head.snapshot_ref().source.0.len() as u64 + 40,
            )?;
            store
                .get_ref(view.head.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .slice(&view.head)?;
            view.view.validate_with_maps(&store, registry, &maps, b)?;
            for element in &view.view.elements {
                if !maps.contains(&view.head, &element.span, b)? {
                    return Err(StructureError::ViewOwner);
                }
            }
        }
        // A shared embed is checked at the deepest owner path, not whichever
        // shallow branch happened to reach the referenced node first.
        b.charge(
            Resource::AllocationUnits,
            (self.value.nodes.len() as u64)
                .saturating_mul(8)
                .saturating_add((self.value.embeds.len() as u64).saturating_mul(8)),
        )?;
        let mut depths = vec![0u64; self.value.nodes.len()];
        let root = edges::root(self.value.root).0 as usize;
        depths[root] = 1;
        let mut embeds = vec![0u64; self.value.embeds.len()];
        for node in shape.postorder().iter().rev() {
            let mut index = 0;
            while let Some((child, _)) = edges::edge(&self.value.nodes[*node].kind, index) {
                b.charge(Resource::Work, 1)?;
                depths[child as usize] =
                    depths[child as usize].max(depths[*node].saturating_add(1));
                index += 1;
            }
            let embed = match self.value.nodes[*node].kind {
                DocKind::Guest { syntax, .. }
                | DocKind::InlineMath { syntax }
                | DocKind::DisplayMath { syntax }
                | DocKind::CircuitFigure { syntax, .. }
                | DocKind::Code { syntax } => Some(syntax.0),
                _ => None,
            };
            if let Some(e) = embed {
                embeds[e as usize] = embeds[e as usize].max(depths[*node]);
            }
        }
        let base = b.current_depth();
        for (embed, depth) in self.value.embeds.iter().zip(embeds) {
            b.with_depth_at_least::<_, StructureError>(base.saturating_add(depth), |b| {
                embed.closure.validate(registry, b, admission)?;
                Ok(())
            })?;
        }
        Ok(ValidatedDocumentSyntax {
            document: self,
            shape,
        })
    }
}
