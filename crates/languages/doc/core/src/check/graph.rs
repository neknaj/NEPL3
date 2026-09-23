use super::{ShapeError, ValidatedDocShape, edges};
use crate::model::{DocKind, DocValue, EmbedKind};
use alloc::{vec, vec::Vec};
use nepl3_core::budget::{Budget, Resource};

impl DocValue {
    pub fn validate_shape<'a>(
        &'a self,
        budget: &mut Budget,
    ) -> Result<ValidatedDocShape<'a>, ShapeError> {
        self.walk_shape(budget, true)
    }
    pub(crate) fn walk_shape<'a>(
        &'a self,
        budget: &mut Budget,
        require_all: bool,
    ) -> Result<ValidatedDocShape<'a>, ShapeError> {
        budget.poll()?;
        let count = self.nodes.len() as u64;
        budget.charge(Resource::Work, count)?;
        budget.charge(Resource::Nodes, count)?;
        budget.charge(
            Resource::AllocationUnits,
            count
                .saturating_mul(42)
                .saturating_add(self.embeds.len() as u64),
        )?;
        let mut color = vec![0u8; self.nodes.len()];
        let mut heights = vec![1u64; self.nodes.len()];
        let mut used = vec![false; self.embeds.len()];
        let mut order = Vec::with_capacity(self.nodes.len());
        let (root, category) = edges::root(self.root);
        let root = self.reference(root, category)?;
        let mut stack = Vec::with_capacity(self.nodes.len());
        stack.push((root, 0usize));
        color[root] = 1;
        let base = budget.current_depth();
        while let Some((node, next)) = stack.last().copied() {
            budget.with_depth_at_least::<_, ShapeError>(
                base.saturating_add(stack.len() as u64),
                |b| {
                    b.charge(Resource::Work, 1)?;
                    Ok(())
                },
            )?;
            if let Some((child, category)) = edges::edge(&self.nodes[node].kind, next) {
                let child = self.reference(child, category)?;
                if let Some(frame) = stack.last_mut() {
                    frame.1 += 1;
                }
                match color[child] {
                    1 => return Err(ShapeError::Cycle(child as u64)),
                    0 => {
                        color[child] = 1;
                        stack.push((child, 0));
                    }
                    _ => {}
                }
            } else {
                let mut child_index = 0;
                while let Some((child, _)) = edges::edge(&self.nodes[node].kind, child_index) {
                    budget.charge(Resource::Work, 1)?;
                    let height = heights
                        .get(child as usize)
                        .copied()
                        .ok_or(ShapeError::Reference(child))?;
                    heights[node] = heights[node].max(height.saturating_add(1));
                    child_index += 1;
                }
                budget.with_depth_at_least::<_, ShapeError>(
                    base.saturating_add(heights[node]),
                    |_| Ok(()),
                )?;
                budget.with_depth_at_least(base.saturating_add(stack.len() as u64), |b| {
                    self.local(node, &mut used, b)
                })?;
                color[node] = 2;
                order.push(node);
                stack.pop();
            }
        }
        for (i, c) in color.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if require_all && *c == 0 {
                return Err(ShapeError::Unreachable(i as u64));
            }
        }
        for (i, present) in used.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if !present {
                return Err(ShapeError::UnusedEmbed(i as u64));
            }
        }
        Ok(ValidatedDocShape { value: self, order })
    }
    fn reference(&self, node: u64, category: super::Category) -> Result<usize, ShapeError> {
        let index = usize::try_from(node).map_err(|_| ShapeError::Reference(node))?;
        let value = self.nodes.get(index).ok_or(ShapeError::Reference(node))?;
        if !edges::accepts(&value.kind, category) {
            return Err(ShapeError::Category {
                node,
                expected: category,
            });
        }
        Ok(index)
    }
    fn local(&self, node: usize, used: &mut [bool], budget: &mut Budget) -> Result<(), ShapeError> {
        use DocKind::*;
        let id = node as u64;
        let item = &self.nodes[node];
        // Every current named constructor has exactly one location kind;
        // retaining repeated entries cannot make its selection ambiguous.
        budget.charge(Resource::Work, item.locations.len() as u64)?;
        if item.locations.len() > 1 {
            return Err(ShapeError::FieldLocation(node as u64));
        }
        for location in &item.locations {
            if !matches!(
                (&item.kind, location.field),
                (DocKind::Section { .. }, crate::model::DocField::SectionId)
                    | (DocKind::Anchor { .. }, crate::model::DocField::AnchorId)
                    | (
                        DocKind::Reference { .. },
                        crate::model::DocField::ReferenceTarget
                    )
            ) {
                return Err(ShapeError::FieldLocation(node as u64));
            }
        }
        match &self.nodes[node].kind {
            Parallel { variants } => {
                if variants.len() < 2 {
                    return Err(ShapeError::ParallelArity(id));
                }
                for (i, a) in variants.iter().enumerate() {
                    for b in &variants[..i] {
                        let a_lang = self.language(a.0)?;
                        let b_lang = self.language(b.0)?;
                        budget.charge(
                            Resource::Work,
                            (a_lang.len() as u64)
                                .saturating_add(b_lang.len() as u64)
                                .saturating_add(1),
                        )?;
                        if a_lang.eq_ignore_ascii_case(b_lang) {
                            return Err(ShapeError::DuplicateLanguage {
                                node: id,
                                first: b.0,
                                second: a.0,
                            });
                        }
                    }
                }
            }
            Table {
                columns,
                header,
                rows,
            } => {
                for r in header.iter().chain(rows) {
                    budget.charge(Resource::Work, 1)?;
                    let row = self
                        .nodes
                        .get(r.0 as usize)
                        .ok_or(ShapeError::Reference(r.0))?;
                    if !matches!(&row.kind,Row{cells} if cells.len()==columns.len()) {
                        return Err(ShapeError::TableWidth(r.0));
                    }
                }
            }
            _ => {}
        }
        if let Some((reference, kind)) = item.kind.embedded() {
            self.embed(reference.0, kind, used)?;
        }
        Ok(())
    }
    fn language(&self, node: u64) -> Result<&str, ShapeError> {
        match self.nodes.get(node as usize).map(|n| &n.kind) {
            Some(DocKind::Variant { language, .. }) => Ok(language),
            _ => Err(ShapeError::Reference(node)),
        }
    }
    fn embed(&self, index: u64, expected: EmbedKind, used: &mut [bool]) -> Result<(), ShapeError> {
        let i = usize::try_from(index).map_err(|_| ShapeError::Embed(index))?;
        if self.embeds.get(i).is_none_or(|v| v.kind != expected) {
            return Err(ShapeError::Embed(index));
        }
        *used.get_mut(i).ok_or(ShapeError::Embed(index))? = true;
        Ok(())
    }
}
