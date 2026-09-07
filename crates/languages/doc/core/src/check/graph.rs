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
        let mut visible = vec![false; self.nodes.len()];
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
                let node_visible =
                    budget.with_depth_at_least(base.saturating_add(stack.len() as u64), |b| {
                        self.local(node, &visible, &mut used, b)?;
                        self.visible(node, &visible, b)
                    })?;
                visible[node] = node_visible;
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
    fn local(
        &self,
        node: usize,
        visible: &[bool],
        used: &mut [bool],
        budget: &mut Budget,
    ) -> Result<(), ShapeError> {
        use DocKind::*;
        let id = node as u64;
        match &self.nodes[node].kind {
            Ruby { base, reading } => {
                for r in [base, reading] {
                    if !visibility(visible, r.0)? {
                        return Err(ShapeError::EmptyAnnotationPart(r.0));
                    }
                }
            }
            Anno { base, notes } => {
                if notes.is_empty() {
                    return Err(ShapeError::AnnotationNotes(id));
                }
                if !visibility(visible, base.0)? {
                    return Err(ShapeError::EmptyAnnotationPart(base.0));
                }
                for r in notes {
                    budget.charge(Resource::Work, 1)?;
                    if !visibility(visible, r.0)? {
                        return Err(ShapeError::EmptyAnnotationPart(r.0));
                    }
                }
            }
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
            InlineMath { syntax } => self.embed(syntax.0, EmbedKind::InlineMath, used)?,
            DisplayMath { syntax } => self.embed(syntax.0, EmbedKind::DisplayMath, used)?,
            CircuitFigure { syntax, .. } => self.embed(syntax.0, EmbedKind::CircuitFigure, used)?,
            Code { syntax } => self.embed(syntax.0, EmbedKind::Code, used)?,
            _ => {}
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
    fn visible(
        &self,
        node: usize,
        visible: &[bool],
        budget: &mut Budget,
    ) -> Result<bool, ShapeError> {
        use DocKind::*;
        match &self.nodes[node].kind {
            Text { text } | InlineCode { text } => Ok(!text.is_empty()),
            Break | InlineMath { .. } | InlineImage { .. } => Ok(true),
            Concat { inlines } | Sentence { inlines } => {
                for r in inlines {
                    budget.charge(Resource::Work, 1)?;
                    if visibility(visible, r.0)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            Ruby { base, .. } | Anno { base, .. } => visibility(visible, base.0),
            Anchor { label, .. } | Reference { label, .. } | Link { label, .. } => {
                visibility(visible, label.0)
            }
            Emphasis { inline } | Strong { inline } => visibility(visible, inline.0),
            _ => Ok(false),
        }
    }
}
fn visibility(values: &[bool], node: u64) -> Result<bool, ShapeError> {
    let i = usize::try_from(node).map_err(|_| ShapeError::Reference(node))?;
    values.get(i).copied().ok_or(ShapeError::Reference(node))
}
