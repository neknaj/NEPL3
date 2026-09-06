//! A native reader context carries proof of its actual environment digest.
use crate::model::ReaderContext;
use alloc::vec::Vec;
use core::ops::Deref;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{OriginError, OriginGraph},
    schema::{SchemaError, SchemaRegistry},
    source::{Digest, SnapshotId, SourceError, SourceSnapshot, SourceStore},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};

#[derive(Debug)]
pub enum ContextError<E> {
    Stopped(StopReason),
    Schema(SchemaError),
    Origin(OriginError),
    Source(SourceError),
    Boundary(E),
    InvalidContext,
    DigestMismatch,
}
impl<E> From<StopReason> for ContextError<E> {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

#[derive(Debug)]
pub struct CheckedReaderContext<'a> {
    raw: &'a ReaderContext,
    foundation: SchemaRef,
    sources: Vec<&'a SourceSnapshot>,
}
impl CheckedReaderContext<'_> {
    pub fn raw(&self) -> &ReaderContext {
        self.raw
    }
    pub fn foundation_schema(&self) -> &SchemaRef {
        &self.foundation
    }
    pub fn sources(&self) -> &[&SourceSnapshot] {
        &self.sources
    }
}
impl Deref for CheckedReaderContext<'_> {
    type Target = ReaderContext;
    fn deref(&self) -> &ReaderContext {
        self.raw
    }
}

impl ReaderContext {
    pub fn check<'a, C: FoundationValueCodec>(
        &'a self,
        codec: &mut C,
        sources: &'a SourceStore,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<CheckedReaderContext<'a>, ContextError<C::Error>> {
        budget.poll()?;
        let closure = source_closure(
            self,
            |id| {
                sources
                    .snapshots()
                    .iter()
                    .find(|source| source.identity() == id)
            },
            budget,
        )
        .map_err(ContextError::Source)?;
        for source in &closure {
            codec
                .admit_source(source, budget)
                .map_err(ContextError::Boundary)?;
        }
        if !registry.is_finalized()
            || registry.descriptor(codec.foundation_schema()).is_none()
            || registry.descriptor(&self.schema).is_none()
            || self.category.is_empty()
            || self.mode.is_empty()
        {
            return Err(ContextError::InvalidContext);
        }
        OriginGraph::validate_origins(&self.origins, sources, budget)
            .map_err(ContextError::Origin)?;
        for (index, binding) in self.environment.value.bindings.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if binding.name.is_empty()
                || binding.namespace.name.is_empty()
                || registry.descriptor(&binding.namespace.schema).is_none()
                || self.environment.value.bindings[..index]
                    .iter()
                    .any(|b| b.namespace == binding.namespace && b.name == binding.name)
                || binding
                    .origin
                    .is_some_and(|id| id.0 >= self.origins.len() as u64)
            {
                return Err(ContextError::InvalidContext);
            }
            registry
                .validate_typed(&binding.value, budget)
                .map_err(ContextError::Schema)?;
        }
        for (index, resource) in self.environment.value.resources.iter().enumerate() {
            budget.charge(Resource::Work, resource.bytes.len() as u64 + 1)?;
            if resource.id.is_empty()
                || Digest::of(&resource.bytes) != resource.digest
                || self.environment.value.resources[..index]
                    .iter()
                    .any(|r| r.id == resource.id)
            {
                return Err(ContextError::InvalidContext);
            }
        }
        let digest = codec
            .environment_digest(&self.environment.value, budget)
            .map_err(ContextError::Boundary)?;
        if digest != self.environment.digest {
            return Err(ContextError::DigestMismatch);
        }
        budget.charge(
            Resource::AllocationUnits,
            codec.foundation_schema().package.len() as u64,
        )?;
        Ok(CheckedReaderContext {
            raw: self,
            foundation: codec.foundation_schema().clone(),
            sources: closure,
        })
    }
}

fn source_closure<'a>(
    raw: &ReaderContext,
    resolve: impl Fn(&SnapshotId) -> Option<&'a SourceSnapshot>,
    budget: &mut Budget,
) -> Result<Vec<&'a SourceSnapshot>, SourceError> {
    let mut sources: Vec<&SourceSnapshot> = Vec::new();
    for origin in &raw.origins {
        budget.charge(Resource::Work, 1)?;
        let span = match origin {
            nepl3_core::origin::Origin::Direct(span) => Some(span),
            nepl3_core::origin::Origin::Generated { callsite, .. } => callsite.as_ref(),
            nepl3_core::origin::Origin::Synthetic { anchor, .. } => anchor.as_ref(),
            nepl3_core::origin::Origin::Composite(_) => None,
        };
        if let Some(span) = span {
            let source = resolve(span.snapshot_ref()).ok_or(SourceError::MissingSnapshot)?;
            source.slice(span)?;
            budget.charge(Resource::Work, sources.len() as u64)?;
            if !sources
                .iter()
                .any(|prior| prior.identity() == source.identity())
            {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<&SourceSnapshot>() as u64,
                )?;
                sources.push(source);
            }
        }
    }
    budget.charge(
        Resource::Work,
        (sources.len() as u64).saturating_mul(sources.len() as u64),
    )?;
    sources.sort_unstable_by(|a, b| a.identity().cmp(b.identity()));
    Ok(sources)
}

/// Only a private session slot whose echo was completely checked may restore this proof.
pub(crate) fn restored<'a>(
    raw: &'a ReaderContext,
    foundation: &SchemaRef,
    sources: &'a [SourceSnapshot],
    budget: &mut Budget,
) -> Result<CheckedReaderContext<'a>, SourceError> {
    let closure = source_closure(
        raw,
        |id| sources.iter().find(|source| source.identity() == id),
        budget,
    )?;
    budget.charge(Resource::AllocationUnits, foundation.package.len() as u64)?;
    Ok(CheckedReaderContext {
        raw,
        foundation: foundation.clone(),
        sources: closure,
    })
}
