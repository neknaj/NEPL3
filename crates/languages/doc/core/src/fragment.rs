//! Checked semantic fragment extraction with unchanged provenance identities.
use crate::{
    check::{ShapeError, StructureError, edges},
    model::{DocKind, DocRoot, DocumentSyntax},
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::SourceAdmission,
};

impl DocumentSyntax {
    /// Extract a reachable semantic subgraph and reindex its nodes and embeds.
    ///
    /// The entire input is validated first. Source snapshots, Origin arenas,
    /// views and source maps retain their original identities, including entries
    /// not used by the fragment. Node spans still describe original source;
    /// printing and applying an edit require a new snapshot and a fresh parse.
    /// This operation neither evaluates guests nor produces printed guest text.
    /// Labels and preparation results must be resolved for the new fragment.
    /// Validation and cloning costs depend on the entire input document.
    pub fn fragment(
        &self,
        root: DocRoot,
        registry: &SchemaRegistry,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, StructureError> {
        let checked = self.validate_structure(registry, budget, admission)?;
        let (root_id, category) = edges::root(root);
        let index = usize::try_from(root_id).map_err(|_| ShapeError::Reference(root_id))?;
        let node = self
            .value
            .nodes
            .get(index)
            .ok_or(ShapeError::Reference(root_id))?;
        if !edges::accepts(&node.kind, category) {
            return Err(ShapeError::Category {
                node: root_id,
                expected: category,
            }
            .into());
        }
        budget.charge(
            Resource::AllocationUnits,
            (self.value.nodes.len() as u64).saturating_mul(
                (core::mem::size_of::<bool>()
                    + core::mem::size_of::<usize>()
                    + core::mem::size_of::<u64>()) as u64,
            ),
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (self.value.embeds.len() as u64).saturating_mul(8),
        )?;
        let mut selected = vec![false; self.value.nodes.len()];
        let mut pending = Vec::with_capacity(self.value.nodes.len());
        selected[index] = true;
        pending.push(index);
        while let Some(index) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            let mut next = 0;
            while let Some((child, _)) = edges::edge(&self.value.nodes[index].kind, next) {
                budget.charge(Resource::Work, 1)?;
                let child = child as usize; // Full input validation bounds every edge.
                if !selected[child] {
                    selected[child] = true;
                    pending.push(child);
                }
                next += 1;
            }
        }
        let mut mapping = vec![u64::MAX; self.value.nodes.len()];
        let mut embed_mapping = vec![u64::MAX; self.value.embeds.len()];
        let mut count = 0;
        for &old in checked.shape().postorder() {
            budget.charge(Resource::Work, 1)?;
            if selected[old] {
                mapping[old] = count;
                count += 1;
            }
        }
        let mut output = self.clone_with_budget(budget)?;
        let mut nodes = core::mem::take(&mut output.value.nodes);
        budget.charge(
            Resource::AllocationUnits,
            (output.value.embeds.len() as u64)
                .saturating_mul(core::mem::size_of::<Option<crate::model::DocEmbed>>() as u64),
        )?;
        let mut embeds: Vec<_> = core::mem::take(&mut output.value.embeds)
            .into_iter()
            .map(Some)
            .collect();
        budget.charge(
            Resource::AllocationUnits,
            count.saturating_mul(core::mem::size_of::<crate::model::DocNode>() as u64),
        )?;
        budget.charge(
            Resource::AllocationUnits,
            (embeds.len() as u64)
                .saturating_mul(core::mem::size_of::<crate::model::DocEmbed>() as u64),
        )?;
        output.value.nodes = Vec::with_capacity(count as usize);
        output.value.embeds = Vec::with_capacity(embeds.len());
        // Move owned slots; no unmetered deep clone of a guest.
        for &old in checked.shape().postorder() {
            budget.charge(Resource::Work, 1)?;
            if !selected[old] {
                continue;
            }
            let node = &mut nodes[old];
            edges::rewrite(&mut node.kind, |id| {
                budget.charge(Resource::Work, 1)?;
                Ok(mapping[id as usize])
            })?;
            let embed = match &mut node.kind {
                DocKind::Guest { syntax, .. }
                | DocKind::InlineMath { syntax }
                | DocKind::DisplayMath { syntax }
                | DocKind::CircuitFigure { syntax, .. }
                | DocKind::Code { syntax } => Some(syntax),
                _ => None,
            };
            if let Some(embed) = embed {
                let old = embed.0 as usize;
                if embed_mapping[old] == u64::MAX {
                    embed_mapping[old] = output.value.embeds.len() as u64;
                    // Keep the full ForeignClosure byte-for-byte.
                    let moved = embeds[old].take().ok_or(ShapeError::Embed(old as u64))?;
                    output.value.embeds.push(moved);
                }
                embed.0 = embed_mapping[old];
            }
        }
        // Node order is the validated postorder, not original arena order.
        budget.charge(
            Resource::AllocationUnits,
            (nodes.len() as u64)
                .saturating_mul(core::mem::size_of::<Option<crate::model::DocNode>>() as u64),
        )?;
        let mut owned: Vec<_> = nodes.into_iter().map(Some).collect();
        for &old in checked.shape().postorder() {
            budget.charge(Resource::Work, 1)?;
            if selected[old] {
                output
                    .value
                    .nodes
                    .push(owned[old].take().ok_or(ShapeError::Reference(old as u64))?);
            }
        }
        output.value.root = root;
        edges::rewrite_root(&mut output.value.root, |id| Ok(mapping[id as usize]))?;
        output.validate_structure(registry, budget, admission)?;
        Ok(output)
    }
}
