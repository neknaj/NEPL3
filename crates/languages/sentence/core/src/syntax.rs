//! Source presentation is separate from the ordered sentence meaning arena.
use crate::{
    check::{self, CheckedShape},
    model::SentenceValue,
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{Mapping, Origin, OriginError, OriginGraph, OriginId, SourceMap},
    schema::{SchemaError, SchemaRegistry},
    source::{SourceAdmission, SourceError, SourceSnapshot, SourceStore, Span},
    view::{ViewBundle, ViewError},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeLocation {
    /// Required also for source-less nodes, which use a Synthetic/Generated origin.
    pub origin: OriginId,
    pub head: Option<Span>,
    pub cover: Option<Span>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentenceView {
    /// Index into the sentence meaning arena; ViewRefs stay local to `view`.
    pub owner: u64,
    pub head: Span,
    pub view: ViewBundle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SentenceSyntax {
    pub value: SentenceValue,
    /// Dense, same order and length as value.nodes. Semantic normalization must
    /// explicitly remap these associations rather than retain old numeric IDs.
    pub locations: Vec<NodeLocation>,
    pub sources: Vec<SourceSnapshot>,
    pub origins: Vec<Origin>,
    pub views: Vec<SentenceView>,
    pub source_maps: Vec<Mapping>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Shape(check::Error),
    Source(SourceError),
    Origin(OriginError),
    View(ViewError),
    Schema(SchemaError),
    LocationCount,
    OriginReference(u64),
    DuplicateSource,
    HeadCover(u64),
    ViewOwner(u64),
}
impl From<StopReason> for Error {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<check::Error> for Error {
    fn from(e: check::Error) -> Self {
        match e {
            check::Error::Stopped(s) => Self::Stopped(s),
            other => Self::Shape(other),
        }
    }
}
impl From<SourceError> for Error {
    fn from(e: SourceError) -> Self {
        match e {
            SourceError::Stopped(s) => Self::Stopped(s),
            other => Self::Source(other),
        }
    }
}
impl From<OriginError> for Error {
    fn from(e: OriginError) -> Self {
        match e {
            OriginError::Stopped(s) | OriginError::Source(SourceError::Stopped(s)) => {
                Self::Stopped(s)
            }
            other => Self::Origin(other),
        }
    }
}
impl From<ViewError> for Error {
    fn from(e: ViewError) -> Self {
        match e.stop_reason() {
            Some(s) => Self::Stopped(s),
            None => Self::View(e),
        }
    }
}
impl From<SchemaError> for Error {
    fn from(e: SchemaError) -> Self {
        match e {
            SchemaError::Stopped(s) => Self::Stopped(s),
            other => Self::Schema(other),
        }
    }
}
pub struct CheckedSyntax<'a> {
    syntax: &'a SentenceSyntax,
    shape: CheckedShape<'a>,
}
impl<'a> CheckedSyntax<'a> {
    pub fn syntax(&self) -> &'a SentenceSyntax {
        self.syntax
    }
    pub fn shape(&self) -> &CheckedShape<'a> {
        &self.shape
    }
}
pub(crate) fn sources(
    sources: &[SourceSnapshot],
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<SourceStore, Error> {
    let mut store = SourceStore::default();
    for source in sources {
        admission.admit_existing(source, b)?;
        if !store.insert_distinct_ref_with_budget(source, b)? {
            return Err(Error::DuplicateSource);
        }
    }
    Ok(store)
}
fn span(span: &Span, store: &SourceStore, b: &mut Budget) -> Result<(), Error> {
    b.charge(
        Resource::Work,
        span.snapshot_ref().source.0.len() as u64 + 40,
    )?;
    store
        .get_ref(span.snapshot_ref())
        .ok_or(SourceError::MissingSnapshot)?
        .slice(span)?;
    Ok(())
}
impl SentenceSyntax {
    /// Validates source/Origin/View closure and association shape, not source
    /// re-parsing, guest meaning, binding resolution or safe rendering.
    pub fn validate<'a>(
        &'a self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedSyntax<'a>, Error> {
        b.poll()?;
        if !registry.is_finalized() {
            return Err(SchemaError::Unfinalized.into());
        }
        let shape = self.value.validate_shape(b)?;
        if self.locations.len() != self.value.nodes.len() {
            return Err(Error::LocationCount);
        }
        let store = sources(&self.sources, admission, b)?;
        OriginGraph::validate_origins(&self.origins, &store, b)?;
        let maps = SourceMap::validate_mappings(&self.source_maps, &store, b)?;
        for (index, location) in self.locations.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if usize::try_from(location.origin.0)
                .ok()
                .is_none_or(|i| i >= self.origins.len())
            {
                return Err(Error::OriginReference(location.origin.0));
            }
            if let Some(cover) = &location.cover {
                span(cover, &store, b)?;
            }
            if let Some(head) = &location.head {
                span(head, &store, b)?;
                let Some(cover) = &location.cover else {
                    return Err(Error::HeadCover(index as u64));
                };
                if !maps.contains(cover, head, b)? {
                    return Err(Error::HeadCover(index as u64));
                }
            }
        }
        for view in &self.views {
            b.charge(Resource::Work, 1)?;
            let owner = usize::try_from(view.owner)
                .ok()
                .and_then(|i| self.locations.get(i))
                .ok_or(Error::ViewOwner(view.owner))?;
            span(&view.head, &store, b)?;
            if let Some(cover) = &owner.cover
                && !maps.contains(cover, &view.head, b)?
            {
                return Err(Error::ViewOwner(view.owner));
            }
            view.view.validate_with_maps(&store, registry, &maps, b)?;
            for element in &view.view.elements {
                if !maps.contains(&view.head, &element.span, b)? {
                    return Err(Error::ViewOwner(view.owner));
                }
            }
        }
        shape.validate_foreign(registry, b, admission)?;
        b.poll()?;
        Ok(CheckedSyntax {
            syntax: self,
            shape,
        })
    }
}
