//! Local Reader effects and bidirectionally connected mapping provenance.
use super::{
    ParseError, build,
    model::{ParseArena, ReaderFactBatch},
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    origin::{Mapping, Origin},
    source::{SnapshotId, Span},
    syntax::NodeRef,
};
use nepl3_reader::model::ReaderFact;
#[derive(Clone, Copy)]
struct Range<'a> {
    snapshot: &'a SnapshotId,
    start: u64,
    end: u64,
}
impl<'a> From<&'a Span> for Range<'a> {
    fn from(span: &'a Span) -> Self {
        Self {
            snapshot: span.snapshot_ref(),
            start: span.start(),
            end: span.end(),
        }
    }
}
fn same(a: Range<'_>, b: Range<'_>, budget: &mut Budget) -> Result<bool, StopReason> {
    Ok(a.snapshot.compare_with_budget(b.snapshot, budget)?.is_eq())
}
fn intersects(a: Range<'_>, b: Range<'_>, budget: &mut Budget) -> Result<bool, StopReason> {
    budget.charge(Resource::Work, 2)?;
    // Inclusive endpoints retain zero-width and boundary paths conservatively.
    Ok(a.start <= b.end && b.start <= a.end && same(a, b, budget)?)
}
struct Closure<'a> {
    ranges: Vec<Range<'a>>,
    pending: Vec<Range<'a>>,
}
impl<'a> Closure<'a> {
    fn add(&mut self, mut range: Range<'a>, b: &mut Budget) -> Result<(), StopReason> {
        for existing in &self.ranges {
            b.charge(Resource::Work, 2)?;
            if existing.start <= range.start
                && range.end <= existing.end
                && same(*existing, range, b)?
            {
                return Ok(());
            }
        }
        let mut index = 0;
        while index < self.ranges.len() {
            if intersects(self.ranges[index], range, b)? {
                let old = self.ranges.swap_remove(index);
                range.start = range.start.min(old.start);
                range.end = range.end.max(old.end);
                index = 0;
            } else {
                index += 1;
            }
        }
        build::slot::<Range<'a>>(b)?;
        build::slot::<Range<'a>>(b)?;
        self.ranges.push(range);
        self.pending.push(range);
        Ok(())
    }
    fn span(&mut self, span: &'a Span, b: &mut Budget) -> Result<(), StopReason> {
        self.add(span.into(), b)
    }
    fn select(mut self, maps: &'a [Mapping], b: &mut Budget) -> Result<Vec<bool>, StopReason> {
        b.charge(Resource::AllocationUnits, maps.len() as u64)?;
        b.charge(Resource::Work, maps.len() as u64)?;
        let mut keep = vec![false; maps.len()];
        // Before traversal begins, ranges is the exact union of all seed
        // ranges. Intermediate versions left in pending add no reachability.
        // Reuse its already charged storage, avoiding repeated full map scans.
        if !maps.is_empty() && self.pending.len() > self.ranges.len() {
            b.charge(Resource::Work, self.ranges.len() as u64)?;
            self.pending.clear();
            // add() appends to pending whenever it appends to ranges, and only
            // removes from ranges. Thus pending's capacity already suffices.
            self.pending.extend_from_slice(&self.ranges);
        }
        while let Some(range) = self.pending.pop() {
            for (index, mapping) in maps.iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                if keep[index] {
                    continue;
                }
                if intersects(range, (&mapping.source).into(), b)?
                    || intersects(range, (&mapping.target).into(), b)?
                {
                    keep[index] = true;
                    // Keep full rows and both directions, including alternate paths.
                    self.span(&mapping.source, b)?;
                    self.span(&mapping.target, b)?;
                }
            }
        }
        b.poll()?;
        Ok(keep)
    }
}
/// Selection over private parser state backed by the complete accepted Reader
/// collector, never validation of arbitrary external maps. Prepare before any
/// completed frame/arena is released; all failure paths leave the arena intact.
pub(super) fn prepare(
    arena: &ParseArena,
    root: NodeRef,
    end: u64,
    local_maps: &[Mapping],
    facts: &[ReaderFactBatch],
    b: &mut Budget,
) -> Result<Vec<bool>, ParseError> {
    let mut closure = Closure {
        ranges: Vec::new(),
        pending: Vec::new(),
    };
    let node = arena
        .nodes
        .get(usize::try_from(root.0).map_err(|_| ParseError::Reference)?)
        .ok_or(ParseError::Reference)?;
    let head = node
        .head
        .as_ref()
        .or(node.cover.as_ref())
        .ok_or(ParseError::Reference)?;
    // complete() extends this exact head's cover and Direct origin to end.
    closure.add(
        Range {
            snapshot: head.snapshot_ref(),
            start: head.start(),
            end,
        },
        b,
    )?;
    for map in local_maps {
        b.charge(Resource::Work, 1)?;
        closure.span(&map.source, b)?;
        closure.span(&map.target, b)?;
    }
    for node in &arena.nodes {
        b.charge(Resource::Work, 1)?;
        for span in node.head.iter().chain(node.cover.iter()) {
            closure.span(span, b)?;
        }
    }
    for origin in &arena.origins {
        b.charge(Resource::Work, 1)?;
        match origin {
            Origin::Direct(span) => closure.span(span, b)?,
            Origin::Generated {
                callsite: Some(span),
                ..
            }
            | Origin::Synthetic {
                anchor: Some(span), ..
            } => closure.span(span, b)?,
            _ => {}
        }
    }
    for token in &arena.tokens {
        b.charge(Resource::Work, 1)?;
        closure.span(&token.head, b)?;
        for trivia in &token.leading_trivia {
            b.charge(Resource::Work, 1)?;
            closure.span(&trivia.span, b)?;
        }
        for view in &token.views.elements {
            b.charge(Resource::Work, 1)?;
            closure.span(&view.span, b)?;
        }
    }
    for batch in facts {
        b.charge(Resource::Work, 1)?;
        for trivia in &batch.trivia {
            b.charge(Resource::Work, 1)?;
            closure.span(&trivia.span, b)?;
        }
        for fact in &batch.facts {
            b.charge(Resource::Work, 1)?;
            match fact {
                ReaderFact::Capture { span, .. } | ReaderFact::Presentation { span, .. } => {
                    closure.span(span, b)?
                }
                ReaderFact::Relation { from, to, .. } => {
                    closure.span(from, b)?;
                    closure.span(to, b)?;
                }
            }
        }
    }
    let keep = closure.select(&arena.source_maps, b)?;
    b.charge(Resource::Work, arena.source_maps.len() as u64)?;
    Ok(keep)
}
/// The mask has this arena's unchanged mapping length and filter work is prepaid.
pub(super) fn apply(arena: &mut ParseArena, keep: &[bool]) {
    let mut index = 0;
    arena.source_maps.retain(|_| {
        let retain = keep[index];
        index += 1;
        retain
    });
}
#[cfg(test)]
mod tests;
