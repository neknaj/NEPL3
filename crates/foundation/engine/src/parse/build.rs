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
#[cfg(test)]
mod tests;
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
/// Private session-owned index, never accepted from a continuation or provider.
pub(super) struct SourceIndex {
    entries: Vec<usize>,
    hint: Option<usize>,
}
impl SourceIndex {
    pub(super) fn new(
        sources: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<Self, SyntaxError> {
        budget.poll()?;
        let mut entries = Vec::new();
        let mut hint = None;
        for (i, source) in sources.iter().enumerate() {
            let at = source_position(&entries, sources, source, hint, budget)?
                .map_or_else(Ok, |_| Err(SourceError::IdentityConflict))?;
            slot::<usize>(budget)?;
            budget.charge(Resource::Work, (entries.len() - at) as u64)?;
            entries.insert(at, i);
            hint = Some(at);
        }
        Ok(Self { entries, hint })
    }
}
impl ParseArena {
    /// Merge a complete accepted source closure with one temporary index. The
    /// public source list keeps insertion order and has no serialized cache.
    #[cfg(test)]
    pub(super) fn extend_sources(
        &mut self,
        sources: &[SourceSnapshot],
        budget: &mut Budget,
    ) -> Result<(), SyntaxError> {
        let mut index = SourceIndex::new(&self.sources, budget)?;
        self.extend_sources_indexed(sources, &mut index, budget)
    }
    pub(super) fn extend_sources_indexed(
        &mut self,
        sources: &[SourceSnapshot],
        cached: &mut SourceIndex,
        budget: &mut Budget,
    ) -> Result<(), SyntaxError> {
        budget.poll()?;
        if sources.is_empty() {
            return Ok(());
        }
        // Both are owned by the same private Machine arena. No caller-provided
        // vector or echoed continuation can replace either side of this pair.
        debug_assert_eq!(cached.entries.len(), self.sources.len());
        let index = &mut cached.entries;
        let hint = &mut cached.hint;
        for source in sources {
            match source_position(index, &self.sources, source, *hint, budget)? {
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
                    *hint = Some(at);
                }
                Err(at) => {
                    slot::<usize>(budget)?;
                    budget.charge(Resource::Work, (index.len() - at) as u64)?;
                    let owned = source.clone_with_budget(budget)?;
                    index.insert(at, self.sources.len());
                    self.sources.push(owned);
                    *hint = Some(at);
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
    hint: Option<usize>,
    budget: &mut Budget,
) -> Result<Result<usize, usize>, StopReason> {
    let (mut low, mut high) = (0, index.len());
    let mut compare = |at: usize| {
        let prior = sources[index[at]].identity();
        let id = source.identity();
        budget.charge(
            Resource::Work,
            (prior.source.0.len() as u64)
                .saturating_add(id.source.0.len() as u64)
                .saturating_add(1),
        )?;
        Ok::<_, StopReason>(
            prior
                .source
                .cmp(&id.source)
                .then_with(|| prior.revision.cmp(&id.revision)),
        )
    };
    // This position is local to one closure merge. Insertion updates it after
    // index shifts; a hit only saves search work, never snapshot validation.
    if let Some(at) = hint {
        match compare(at)? {
            core::cmp::Ordering::Equal => return Ok(Ok(at)),
            core::cmp::Ordering::Less => {
                low = at + 1;
                if low < high {
                    match compare(low)? {
                        core::cmp::Ordering::Equal => return Ok(Ok(low)),
                        core::cmp::Ordering::Greater => return Ok(Err(low)),
                        core::cmp::Ordering::Less => low += 1,
                    }
                }
            }
            core::cmp::Ordering::Greater => {
                high = at;
                if high > 0 {
                    match compare(high - 1)? {
                        core::cmp::Ordering::Equal => return Ok(Ok(high - 1)),
                        core::cmp::Ordering::Less => return Ok(Err(high)),
                        core::cmp::Ordering::Greater => high -= 1,
                    }
                }
            }
        }
    }
    while low < high {
        let mid = low + (high - low) / 2;
        match compare(mid)? {
            core::cmp::Ordering::Equal => return Ok(Ok(mid)),
            core::cmp::Ordering::Less => low = mid + 1,
            core::cmp::Ordering::Greater => high = mid,
        }
    }
    Ok(Err(low))
}
