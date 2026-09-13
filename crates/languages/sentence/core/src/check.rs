//! Iterative finite-arena checking, including longest paths through shared
//! subtrees. Does not resolve URLs, validate foreign source closures or execute.
use crate::model::{Kind, Root, SentenceValue};
use alloc::{vec, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Category {
    Sentence,
    Inline,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Reference(u64),
    Category { node: u64, expected: Category },
    Cycle(u64),
    Unreachable(u64),
    EmptyAnnotationPart(u64),
    AnnotationNotes(u64),
    Embed(u64),
    UnusedEmbed(u64),
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

/// Only the arena shape and local sentence invariants have been checked.
pub struct CheckedShape<'a> {
    value: &'a SentenceValue,
    order: Vec<usize>,
}
impl<'a> CheckedShape<'a> {
    pub fn value(&self) -> &'a SentenceValue {
        self.value
    }
    /// Each node occurs once, after all of its children.
    pub fn postorder(&self) -> &[usize] {
        &self.order
    }
}

fn reference(value: &SentenceValue, id: u64, expected: Category) -> Result<usize, Error> {
    let index = usize::try_from(id).map_err(|_| Error::Reference(id))?;
    let kind = value.nodes.get(index).ok_or(Error::Reference(id))?;
    if matches!(kind, Kind::Sentence { .. }) != (expected == Category::Sentence) {
        return Err(Error::Category { node: id, expected });
    }
    Ok(index)
}

impl SentenceValue {
    pub fn validate_shape<'a>(&'a self, b: &mut Budget) -> Result<CheckedShape<'a>, Error> {
        b.poll()?;
        let count = self.nodes.len();
        b.charge(Resource::Work, count as u64)?;
        b.charge(Resource::Nodes, count as u64)?;
        let scratch_per_node = core::mem::size_of::<u8>()
            + core::mem::size_of::<bool>()
            + core::mem::size_of::<u64>()
            + core::mem::size_of::<usize>()
            + core::mem::size_of::<(usize, usize)>();
        let allocation = count
            .checked_mul(scratch_per_node)
            .and_then(|n| n.checked_add(self.embeds.len()))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, allocation as u64)?;
        let (id, category) = match self.root {
            Root::Sentence(r) => (r.0, Category::Sentence),
            Root::Inline(r) => (r.0, Category::Inline),
        };
        let root = reference(self, id, category)?;
        let mut colors = vec![0u8; count];
        let mut visible = vec![false; count];
        let mut heights = vec![1u64; count];
        let mut used = vec![false; self.embeds.len()];
        let mut order = Vec::with_capacity(count);
        let mut stack = Vec::with_capacity(count);
        stack.push((root, 0usize));
        colors[root] = 1;
        let base = b.current_depth();
        while let Some((node, next)) = stack.last().copied() {
            b.with_depth_at_least::<_, Error>(base.saturating_add(stack.len() as u64), |b| {
                b.charge(Resource::Work, 1)?;
                Ok(())
            })?;
            if let Some(child) = self.nodes[node].child(next) {
                let child = reference(self, child.0, Category::Inline)?;
                if let Some(frame) = stack.last_mut() {
                    frame.1 += 1;
                }
                match colors[child] {
                    1 => return Err(Error::Cycle(child as u64)),
                    0 => {
                        colors[child] = 1;
                        stack.push((child, 0));
                    }
                    _ => {}
                }
                continue;
            }
            let mut any = false;
            let mut index = 0;
            while let Some(child) = self.nodes[node].child(index) {
                b.charge(Resource::Work, 1)?;
                let child = reference(self, child.0, Category::Inline)?;
                heights[node] = heights[node].max(heights[child].saturating_add(1));
                any |= visible[child];
                index += 1;
            }
            b.with_depth_at_least::<_, Error>(base.saturating_add(heights[node]), |_| Ok(()))?;
            let require_content = |child: u64| -> Result<(), Error> {
                let index = reference(self, child, Category::Inline)?;
                if !visible[index] {
                    return Err(Error::EmptyAnnotationPart(child));
                }
                Ok(())
            };
            match &self.nodes[node] {
                Kind::Ruby { base, reading } => {
                    require_content(base.0)?;
                    require_content(reading.0)?;
                }
                Kind::InlineAnno { base, notes } => {
                    if notes.is_empty() {
                        return Err(Error::AnnotationNotes(node as u64));
                    }
                    require_content(base.0)?;
                    for note in notes {
                        b.charge(Resource::Work, 1)?;
                        require_content(note.0)?;
                    }
                }
                _ => {}
            }
            visible[node] = match &self.nodes[node] {
                Kind::Text { text } | Kind::Code { text } => {
                    b.charge(Resource::Work, text.len() as u64)?;
                    !text.is_empty()
                }
                Kind::Break => true,
                Kind::ForeignInline { syntax } => {
                    let index = usize::try_from(syntax.0).map_err(|_| Error::Embed(syntax.0))?;
                    *used.get_mut(index).ok_or(Error::Embed(syntax.0))? = true;
                    true
                }
                Kind::ExternalLink { uri, .. } => {
                    b.charge(Resource::Work, uri.len() as u64)?;
                    any
                }
                _ => any,
            };
            colors[node] = 2;
            order.push(node);
            stack.pop();
        }
        for (index, color) in colors.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if *color == 0 {
                return Err(Error::Unreachable(index as u64));
            }
        }
        for (index, used) in used.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if !used {
                return Err(Error::UnusedEmbed(index as u64));
            }
        }
        b.poll()?;
        Ok(CheckedShape { value: self, order })
    }
}
