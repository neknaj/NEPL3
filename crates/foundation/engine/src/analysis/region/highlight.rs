//! Shared no_std byte-span presentation of admitted engine regions.
//! The original schema-qualified classes and overlapping facts remain intact.
//! This native projection introduces no new serialized operation or proof.
use crate::analysis::region::{RegionKey, RegionOutcome, RegionReply, SourceRegion};
use alloc::vec::Vec;
use core::cmp::Reverse;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceError, SourceSnapshot},
};

/// One nonempty single-line byte range. Indices refer to the immutable reply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    /// Index in the input RegionOutcome::Complete.regions, not a copied fact.
    pub region: u64,
    /// Index in that region's classes. Later declared classes take precedence.
    pub class: u64,
}
#[derive(Debug, Eq, PartialEq)]
pub enum HighlightError {
    Stopped(StopReason),
    Source(SourceError),
    Key,
    Incomplete,
    Range,
}
impl From<StopReason> for HighlightError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
impl From<SourceError> for HighlightError {
    fn from(value: SourceError) -> Self {
        match value {
            SourceError::Stopped(reason) => Self::Stopped(reason),
            other => Self::Source(other),
        }
    }
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), HighlightError> {
    b.charge(
        Resource::AllocationUnits,
        (2 * core::mem::size_of::<T>()) as u64,
    )?;
    b.charge(Resource::Work, 1)?;
    items.push(item);
    Ok(())
}
fn rank(region: &SourceRegion, index: usize) -> (Reverse<u64>, Reverse<u64>, u64, u64, usize) {
    (
        Reverse(region.depth),
        Reverse(region.priority),
        region.span.end() - region.span.start(),
        region.declaration_order,
        index,
    )
}
/// Pure presentation projection of an already admitted RegionReply. Matching
/// keys and explicit source identity are checked again. This is not provider
/// authentication and does not admit any additional source lookup capability.
///
/// Precedence is deepest region, then explicit priority, shortest span, earliest
/// declaration order, and input order. Regions without classes are transparent.
/// Within the chosen region the last declared class wins; all classes and raw
/// regions remain in the immutable input. CRLF, CR and LF are never highlighted.
/// Clients perform their own position or rendering conversion.
/// Complexity is bounded quadratic in the number of boundaries/regions, plus
/// source scanning. Every traversal/allocation consumes the caller Budget.
pub fn normalize(
    reply: &RegionReply,
    expected: &RegionKey,
    source: &SourceSnapshot,
    b: &mut Budget,
) -> Result<Vec<HighlightSpan>, HighlightError> {
    b.poll()?;
    if &reply.key != expected {
        return Err(HighlightError::Key);
    }
    let regions = match &reply.outcome {
        RegionOutcome::Complete { regions, .. } => regions,
        RegionOutcome::Stopped(reason) => return Err(HighlightError::Stopped(*reason)),
        RegionOutcome::Invalid(_) => return Err(HighlightError::Incomplete),
    };
    let mut found = false;
    for admitted in &reply.sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + admitted.identity().source.0.len()) as u64 + 64,
        )?;
        if admitted.identity() == source.identity() {
            b.charge(
                Resource::Work,
                (source.text().len() + source.uri().len() + admitted.uri().len()) as u64 + 1,
            )?;
            if admitted != source {
                return Err(HighlightError::Range);
            }
            found = true;
        }
    }
    if !found {
        return Err(HighlightError::Source(SourceError::MissingSnapshot));
    }
    let mut boundaries = Vec::new();
    for region in regions {
        b.charge(
            Resource::Work,
            source.identity().source.0.len() as u64
                + region.span.snapshot_ref().source.0.len() as u64
                + 64,
        )?;
        if region.span.snapshot_ref() != source.identity() {
            return Err(HighlightError::Range);
        }
        source.check_range(region.span.start(), region.span.end())?;
        if region.span.start() != region.span.end() && !region.classes.is_empty() {
            push(&mut boundaries, region.span.start(), b)?;
            push(&mut boundaries, region.span.end(), b)?;
        }
    }
    if boundaries.is_empty() {
        return Ok(Vec::new());
    }
    b.charge(Resource::Work, source.text().len() as u64)?;
    for (i, byte) in source.text().bytes().enumerate() {
        if byte == b'\r' || byte == b'\n' {
            push(&mut boundaries, i as u64, b)?;
            push(&mut boundaries, i as u64 + 1, b)?;
        }
    }
    let count = boundaries.len() as u64;
    let sorting = count
        .checked_mul(64)
        .ok_or_else(|| HighlightError::Stopped(b.stop(StopReason::WorkLimit)))?;
    b.charge(Resource::Work, sorting)?;
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut output: Vec<HighlightSpan> = Vec::new();
    for window in boundaries.windows(2) {
        b.poll()?;
        let (start, end) = (window[0], window[1]);
        if matches!(
            source.text().as_bytes().get(start as usize),
            Some(b'\r' | b'\n')
        ) {
            continue;
        }
        let mut selected: Option<usize> = None;
        for (i, region) in regions.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if !region.classes.is_empty()
                && region.span.start() <= start
                && end <= region.span.end()
                && selected
                    .is_none_or(|previous| rank(region, i) < rank(&regions[previous], previous))
            {
                selected = Some(i);
            }
        }
        let Some(selected) = selected else {
            continue;
        };
        let item = HighlightSpan {
            byte_start: start,
            byte_end: end,
            region: selected as u64,
            class: regions[selected].classes.len() as u64 - 1,
        };
        if let Some(prior) = output.last_mut()
            && prior.byte_end == item.byte_start
            && prior.region == item.region
            && prior.class == item.class
        {
            prior.byte_end = item.byte_end;
        } else {
            push(&mut output, item, b)?;
        }
    }
    Ok(output)
}
