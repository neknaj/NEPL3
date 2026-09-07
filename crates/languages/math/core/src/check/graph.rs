use super::{Category, ShapeError, ValidatedMathShape, edges};
use crate::{model::*, number};
use alloc::{vec, vec::Vec};
use nepl3_core::budget::{Budget, Resource};

impl MathValue {
    pub fn validate_shape<'a>(
        &'a self,
        b: &mut Budget,
    ) -> Result<ValidatedMathShape<'a>, ShapeError> {
        b.poll()?;
        let count = self.nodes.len() as u64;
        b.charge(Resource::Work, count)?;
        b.charge(Resource::Nodes, count)?;
        b.charge(
            Resource::AllocationUnits,
            count
                .saturating_mul(40)
                .saturating_add(self.embeds.len() as u64),
        )?;
        let mut color = vec![0_u8; self.nodes.len()];
        let mut heights = vec![1_u64; self.nodes.len()];
        let mut used = vec![false; self.embeds.len()];
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = Vec::with_capacity(self.nodes.len());
        let (root, category) = edges::root(self.root);
        let root = self.reference(root, category)?;
        color[root] = 1;
        stack.push((root, 0_usize));
        let caller = b.current_depth();
        while let Some((node, next)) = stack.last().copied() {
            b.with_depth_at_least::<_, ShapeError>(
                caller.saturating_add(stack.len() as u64),
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
                let mut i = 0;
                while let Some((child, _)) = edges::edge(&self.nodes[node].kind, i) {
                    b.charge(Resource::Work, 1)?;
                    heights[node] = heights[node].max(heights[child as usize].saturating_add(1));
                    i += 1;
                }
                b.with_depth_at_least::<_, ShapeError>(
                    caller.saturating_add(heights[node]),
                    |_| Ok(()),
                )?;
                self.local(node, &mut used, b)?;
                color[node] = 2;
                order.push(node);
                stack.pop();
            }
        }
        for (i, color) in color.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if *color == 0 {
                return Err(ShapeError::Unreachable(i as u64));
            }
        }
        for (i, used) in used.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if !used {
                return Err(ShapeError::UnusedEmbed(i as u64));
            }
        }
        Ok(ValidatedMathShape { value: self, order })
    }
    fn reference(&self, id: u64, category: Category) -> Result<usize, ShapeError> {
        let index = usize::try_from(id).map_err(|_| ShapeError::Reference(id))?;
        let node = self.nodes.get(index).ok_or(ShapeError::Reference(id))?;
        if !edges::accepts(&node.kind, category) {
            return Err(ShapeError::Category {
                node: id,
                expected: category,
            });
        }
        Ok(index)
    }
    fn local(&self, index: usize, used: &mut [bool], b: &mut Budget) -> Result<(), ShapeError> {
        let node = &self.nodes[index];
        let id = index as u64;
        b.charge(Resource::Work, node.locations.len() as u64)?;
        if node.locations.len() > 1 {
            return Err(ShapeError::FieldLocation(id));
        }
        for location in &node.locations {
            if !matches!(
                (&node.kind, location.field),
                (MathKind::Symbol { .. }, MathField::SymbolName)
                    | (MathKind::Let { .. }, MathField::LetName)
                    | (MathKind::Sum { .. }, MathField::SumIndex)
                    | (MathKind::Integral { .. }, MathField::IntegralIndex)
            ) {
                return Err(ShapeError::FieldLocation(id));
            }
        }
        match &node.kind {
            MathKind::Number { value, .. } => {
                if !number::finite_decimal(value, b)? {
                    return Err(ShapeError::NonFiniteDecimalNumber(id));
                }
            }
            MathKind::Vector { values } => {
                if values.is_empty() {
                    return Err(ShapeError::EmptyVector(id));
                }
            }
            MathKind::Matrix { rows } => {
                if rows.is_empty() {
                    return Err(ShapeError::EmptyMatrix(id));
                }
                let mut width = None;
                for (row, reference) in rows.iter().enumerate() {
                    b.charge(Resource::Work, 1)?;
                    let MathKind::Row { values } = &self.nodes[reference.0 as usize].kind else {
                        return Err(ShapeError::Category {
                            node: reference.0,
                            expected: Category::Row,
                        });
                    };
                    if values.is_empty() || width.is_some_and(|w| w != values.len()) {
                        return Err(ShapeError::MatrixWidth {
                            node: id,
                            row: row as u64,
                        });
                    }
                    width = Some(values.len());
                }
            }
            MathKind::Fence { open, close, .. } => {
                for text in [open, close] {
                    b.charge(Resource::Work, text.len() as u64)?;
                    if text.chars().take(2).count() > 1 {
                        return Err(ShapeError::FenceWidth(id));
                    }
                }
            }
            MathKind::Root { degree, .. } => {
                if let MathKind::Number { value, .. } = &self.nodes[degree.0 as usize].kind
                    && number::is_zero(value, b)?
                {
                    return Err(ShapeError::InvalidRootDegree(id));
                }
            }
            MathKind::DocGuest { syntax } => {
                let index = usize::try_from(syntax.0).map_err(|_| ShapeError::Embed(syntax.0))?;
                let guest = self.embeds.get(index).ok_or(ShapeError::Embed(syntax.0))?;
                b.charge(Resource::Work, guest.syntax.category.len() as u64)?;
                if guest.syntax.category != "Sentence" {
                    return Err(ShapeError::GuestCategory(syntax.0));
                }
                used[index] = true;
            }
            _ => {}
        }
        Ok(())
    }
}
