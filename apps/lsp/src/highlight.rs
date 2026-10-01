//! Normalize admitted engine regions without changing their overlapping facts.
//! Class indices retain the exact schema-qualified presentation class. They are
//! not semantic-token legend indices: legend negotiation is a separate stage.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{LineIndex, PositionEncoding, SourceError, SourceSnapshot},
};
use nepl3_engine::analysis::region::{RegionKey, RegionReply, highlight};

/// One nonempty single-line range, in the negotiated character encoding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightSpan {
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u32,
    pub character: u32,
    pub length: u32,
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
    PositionOverflow,
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
        (2 * std::mem::size_of::<T>()) as u64,
    )?;
    b.charge(Resource::Work, 1)?;
    items.push(item);
    Ok(())
}
fn integer(value: u64) -> Result<u32, HighlightError> {
    if value > i32::MAX as u64 {
        return Err(HighlightError::PositionOverflow);
    }
    Ok(value as u32)
}
/// Pure presentation projection of an already admitted RegionReply. Matching
/// keys and explicit source identity are checked again. This is not provider
/// authentication and does not admit any additional source lookup capability.
///
/// Precedence is deepest region, then explicit priority, shortest span, earliest
/// declaration order, and input order. Regions without classes are transparent.
/// Within the chosen region the last declared class wins; all classes and raw
/// regions remain in the immutable input. CRLF, CR and LF are never highlighted.
/// UTF-8/16/32 positions use the core LineIndex contract; results fit LSP uinteger.
/// Complexity is bounded quadratic in the number of boundaries/regions, plus
/// line conversion work. Every traversal/allocation consumes the caller Budget.
pub fn normalize(
    reply: &RegionReply,
    expected: &RegionKey,
    source: &SourceSnapshot,
    encoding: PositionEncoding,
    b: &mut Budget,
) -> Result<Vec<HighlightSpan>, HighlightError> {
    let spans = highlight::normalize(reply, expected, source, b).map_err(|error| match error {
        highlight::HighlightError::Stopped(e) => HighlightError::Stopped(e),
        highlight::HighlightError::Source(e) => HighlightError::Source(e),
        highlight::HighlightError::Key => HighlightError::Key,
        highlight::HighlightError::Incomplete => HighlightError::Incomplete,
        highlight::HighlightError::Range => HighlightError::Range,
    })?;
    if spans.is_empty() {
        return Ok(Vec::new());
    }
    let identity_bytes = source.identity().source.0.len() as u64;
    // Core LineIndex prepays its own SnapshotId copy. Position conversion
    // only compares the identity and scans text; it creates no temporary Span.
    let index = LineIndex::new(source, b)?;
    let mut output = Vec::new();
    for span in spans {
        b.charge(
            Resource::Work,
            (source.text().len() as u64)
                .saturating_mul(2)
                .saturating_add(identity_bytes.saturating_mul(2))
                .saturating_add(128),
        )?;
        let first = index.position(source, span.byte_start, encoding)?;
        let last = index.position(source, span.byte_end, encoding)?;
        if first.line != last.line || last.character <= first.character {
            return Err(HighlightError::Range);
        }
        push(
            &mut output,
            HighlightSpan {
                byte_start: span.byte_start,
                byte_end: span.byte_end,
                line: integer(first.line)?,
                character: integer(first.character)?,
                length: integer(last.character - first.character)?,
                region: span.region,
                class: span.class,
            },
            b,
        )?;
    }
    Ok(output)
}
