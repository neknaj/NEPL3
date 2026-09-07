//! Budgeted arena construction. Token payload has one owner; prefix fields only hold children.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceError, SourceSnapshot, Span},
    syntax::{FieldValue, NodeRef, SyntaxBundle, SyntaxError, SyntaxNode, TokenRef},
    value::KindRef,
    view::Token,
};

use super::model::ParseArena;
pub(super) fn slot<T>(budget: &mut Budget) -> Result<(), StopReason> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)
}
pub(super) fn text(value: &str, budget: &mut Budget) -> Result<String, StopReason> {
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    budget.charge(Resource::Work, value.len() as u64)?;
    Ok(value.into())
}
pub(super) fn span(value: &Span, budget: &mut Budget) -> Result<Span, StopReason> {
    slot::<Span>(budget)?;
    budget.charge(
        Resource::AllocationUnits,
        value.snapshot_ref().source.0.len() as u64,
    )?;
    Ok(value.clone())
}
impl ParseArena {
    /// Merge a complete accepted source closure with one temporary index. The
    /// public source list keeps insertion order and has no serialized cache.
    pub(super) fn extend_sources(
        &mut self,
        sources: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<(), SyntaxError> {
        budget.poll()?;
        if sources.is_empty() {
            return Ok(());
        }
        let mut index = Vec::new();
        for (i, source) in self.sources.iter().enumerate() {
            let at = source_position(&index, &self.sources, source, budget)?
                .map_or_else(Ok, |_| Err(SourceError::IdentityConflict))?;
            slot::<usize>(budget)?;
            budget.charge(Resource::Work, (index.len() - at) as u64)?;
            index.insert(at, i);
        }
        for source in sources {
            match source_position(&index, &self.sources, source, budget)? {
                Ok(at) => {
                    let prior = &self.sources[index[at]];
                    budget.charge(
                        Resource::Work,
                        (prior.uri().len() as u64)
                            .saturating_add(source.uri().len() as u64)
                            .saturating_add(33),
                    )?;
                    if prior.identity() != source.identity() || prior.uri() != source.uri() {
                        return Err(SourceError::IdentityConflict.into());
                    }
                }
                Err(at) => {
                    slot::<usize>(budget)?;
                    budget.charge(Resource::Work, (index.len() - at) as u64)?;
                    let owned = source.clone_with_budget(budget)?;
                    index.insert(at, self.sources.len());
                    self.sources.push(owned);
                }
            }
        }
        Ok(())
    }
    pub fn source(
        &mut self,
        source: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<(), SyntaxError> {
        for prior in &self.sources {
            budget.charge(Resource::Work, source.identity().source.0.len() as u64 + 33)?;
            if prior.identity().source == source.identity().source
                && prior.identity().revision == source.identity().revision
            {
                budget.charge(Resource::Work, source.uri().len() as u64 + 33)?;
                if prior.identity() != source.identity() || prior.uri() != source.uri() {
                    return Err(SourceError::IdentityConflict.into());
                }
                return Ok(());
            }
        }
        self.sources.push(source.clone_with_budget(budget)?);
        Ok(())
    }
    pub fn head(
        &mut self,
        token: Token,
        kind: &KindRef,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<NodeRef, SyntaxError> {
        budget.charge(Resource::Nodes, 1)?;
        budget.charge(Resource::Work, 1)?;
        slot::<SyntaxNode>(budget)?;
        slot::<Token>(budget)?;
        slot::<Origin>(budget)?;
        let name = text(registry.kind_name(&kind.schema, kind.local_kind)?, budget)?;
        let head = span(&token.head, budget)?;
        let cover = span(&token.head, budget)?;
        let origin_span = span(&token.head, budget)?;
        budget.charge(Resource::AllocationUnits, kind.schema.package.len() as u64)?;
        let node = SyntaxNode {
            schema: kind.schema.clone(),
            kind: name,
            fields: Vec::new(),
            head: Some(head),
            cover: Some(cover),
            origin: OriginId(self.origins.len() as u64),
            token: Some(TokenRef(self.tokens.len() as u64)),
        };
        let id = NodeRef(self.nodes.len() as u64);
        self.origins.push(Origin::Direct(origin_span));
        self.tokens.push(token);
        self.nodes.push(node);
        Ok(id)
    }
    pub fn complete(
        &mut self,
        id: NodeRef,
        fields: &mut Vec<FieldValue>,
        end: u64,
        budget: &mut Budget,
    ) -> Result<(), SyntaxError> {
        let node = self
            .nodes
            .get(usize::try_from(id.0).map_err(|_| SyntaxError::Reference)?)
            .ok_or(SyntaxError::Reference)?;
        let head = node
            .head
            .as_ref()
            .or(node.cover.as_ref())
            .ok_or(SyntaxError::Cover)?;
        let mut source = None;
        for candidate in &self.sources {
            budget.charge(
                Resource::Work,
                (head.snapshot_ref().source.0.len() as u64)
                    .saturating_add(candidate.identity().source.0.len() as u64)
                    .saturating_add(33),
            )?;
            if candidate.identity() == head.snapshot_ref() {
                source = Some(candidate);
                break;
            }
        }
        let source = source.ok_or(SourceError::MissingSnapshot)?;
        let cover = source.span_with_budget(head.start(), end, budget)?;
        let origin_span = span(&cover, budget)?;
        let origin_id = usize::try_from(node.origin.0).map_err(|_| SyntaxError::Reference)?;
        if self.origins.get(origin_id).is_none() {
            return Err(SyntaxError::Reference);
        }
        let node = self
            .nodes
            .get_mut(usize::try_from(id.0).map_err(|_| SyntaxError::Reference)?)
            .ok_or(SyntaxError::Reference)?;
        node.cover = Some(cover);
        node.fields = core::mem::take(fields);
        self.origins[origin_id] = Origin::Direct(origin_span);
        Ok(())
    }
    pub fn finish(self, root: NodeRef) -> SyntaxBundle {
        SyntaxBundle {
            sources: self.sources,
            nodes: self.nodes,
            origins: self.origins,
            tokens: self.tokens,
            source_maps: self.source_maps,
            environments: self.environments,
            root,
        }
    }
}

fn source_position(
    index: &[usize],
    sources: &[SourceSnapshot],
    source: &SourceSnapshot,
    budget: &mut Budget,
) -> Result<Result<usize, usize>, StopReason> {
    let (mut low, mut high) = (0, index.len());
    while low < high {
        let mid = low + (high - low) / 2;
        let prior = sources[index[mid]].identity();
        let id = source.identity();
        budget.charge(
            Resource::Work,
            (prior.source.0.len() as u64)
                .saturating_add(id.source.0.len() as u64)
                .saturating_add(1),
        )?;
        match prior
            .source
            .cmp(&id.source)
            .then_with(|| prior.revision.cmp(&id.revision))
        {
            core::cmp::Ordering::Equal => return Ok(Ok(mid)),
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
        }
    }
    Ok(Err(low))
}
