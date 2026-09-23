//! Iterative finite-arena checking, including longest paths through shared
//! subtrees. Does not resolve URLs, validate foreign source closures or execute.
use crate::model::{EmbedRef, InlineRef, Kind, Root, SentenceValue};
use alloc::{vec, vec::Vec};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_core::{schema::SchemaRegistry, source::SourceAdmission, syntax::SyntaxError};

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
    Foreign(SyntaxError),
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<SyntaxError> for Error {
    fn from(value: SyntaxError) -> Self {
        match value.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Foreign(value),
        }
    }
}

/// Only the arena shape and local sentence invariants have been checked.
pub struct CheckedShape<'a> {
    value: &'a SentenceValue,
    order: Vec<usize>,
}
/// One structural occurrence, in ordered-child traversal order. The same node
/// or embed may occur more than once. Depth is relative to the Sentence root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignOccurrence {
    pub node: InlineRef,
    pub embed: EmbedRef,
    pub depth: u64,
}
impl<'a> CheckedShape<'a> {
    pub fn value(&self) -> &'a SentenceValue {
        self.value
    }
    /// Each node occurs once, after all of its children.
    pub fn postorder(&self) -> &[usize] {
        &self.order
    }

    /// Enumerate every foreign occurrence, including readings and notes.
    /// Hosts can select namespace participants before rendering any guest.
    /// This is a shape traversal only: closures are neither validated nor run.
    /// Shared subtrees are expanded under the caller's cumulative budget; no
    /// partial sequence is returned on a stop. The explicit stack uses O(depth)
    /// storage, independently of the number of siblings or shared occurrences.
    pub fn foreign_occurrences(&self, b: &mut Budget) -> Result<Vec<ForeignOccurrence>, Error> {
        fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), Error> {
            b.charge(Resource::Work, 1)?;
            b.charge(
                Resource::AllocationUnits,
                2 * core::mem::size_of::<T>() as u64,
            )?;
            items.push(item);
            Ok(())
        }
        b.poll()?;
        let root = match self.value.root {
            Root::Sentence(root) => root.0,
            Root::Inline(root) => root.0,
        };
        let mut stack = Vec::new();
        let mut occurrences = Vec::new();
        push(&mut stack, (root as usize, 0usize), b)?;
        let base = b.current_depth();
        while let Some(&(node, next)) = stack.last() {
            let depth = stack.len() as u64;
            b.with_depth_at_least::<_, Error>(base.saturating_add(depth), |b| {
                b.charge(Resource::Work, 1)?;
                if next == 0
                    && let Kind::ForeignInline { syntax } = self.value.nodes[node]
                {
                    push(
                        &mut occurrences,
                        ForeignOccurrence {
                            node: InlineRef(node as u64),
                            embed: syntax,
                            depth,
                        },
                        b,
                    )?;
                }
                Ok(())
            })?;
            if let Some(child) = self.value.nodes[node].child(next) {
                if let Some(frame) = stack.last_mut() {
                    frame.1 += 1;
                }
                push(&mut stack, (child.0 as usize, 0), b)?;
            } else {
                stack.pop();
            }
        }
        Ok(occurrences)
    }

    /// Checks guest closures at their deepest semantic owner occurrence. This
    /// remains a source/syntax proof, never a guest evaluation or rendering proof.
    pub fn validate_foreign(
        &self,
        registry: &SchemaRegistry,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), Error> {
        let embeds = self.foreign_depths(b)?;
        let base = b.current_depth();
        for (closure, depth) in self.value.embeds.iter().zip(embeds) {
            b.with_depth_at_least::<_, Error>(base.saturating_add(depth), |b| {
                closure.validate(registry, b, admission)?;
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Deepest owner occurrence for each embed, relative to this Sentence root.
    /// Hosts use these depths when invoking a selected guest operation. Shared
    /// nodes retain their longest path; returned indices match `value.embeds`.
    pub fn foreign_depths(&self, b: &mut Budget) -> Result<Vec<u64>, Error> {
        b.poll()?;
        let count = self.value.nodes.len();
        let allocation = count
            .checked_add(self.value.embeds.len())
            .and_then(|n| n.checked_mul(core::mem::size_of::<u64>()))
            .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
        b.charge(Resource::AllocationUnits, allocation as u64)?;
        let mut depths = vec![0u64; count];
        let mut embeds = vec![0u64; self.value.embeds.len()];
        let root = match self.value.root {
            Root::Sentence(r) => r.0,
            Root::Inline(r) => r.0,
        };
        // Arena indices and reachability were established by this borrowed proof.
        depths[root as usize] = 1;
        for node in self.order.iter().rev().copied() {
            b.charge(Resource::Work, 1)?;
            let mut index = 0;
            while let Some(child) = self.value.nodes[node].child(index) {
                b.charge(Resource::Work, 1)?;
                depths[child.0 as usize] =
                    depths[child.0 as usize].max(depths[node].saturating_add(1));
                index += 1;
            }
            if let Kind::ForeignInline { syntax } = self.value.nodes[node] {
                embeds[syntax.0 as usize] = embeds[syntax.0 as usize].max(depths[node]);
            }
        }
        Ok(embeds)
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
