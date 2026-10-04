//! Inventory of retained source evidence, not a host-current dependency proof.
use super::quality::StrictNameInsertion;
use crate::parse::RetainedParse;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::SourceSnapshot,
    syntax::canonical::{BundleMappings, CanonicalError},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Original,
    Candidate,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Seed,
    ParseReply,
    /// Index in the root-first canonical foreign-bundle traversal.
    SyntaxBundle(usize),
    BindingReply,
}
#[derive(Clone, Copy, Debug)]
pub struct SourceEvidence<'a> {
    pub side: Side,
    pub stage: Stage,
    pub source: &'a SourceSnapshot,
}
/// Ordered borrowed occurrences, preserving duplicates and historical revisions.
/// These are the sources retained in the supported native envelopes, not a
/// minimal semantic dependency set or a guarantee about external host state.
/// Source mappings and non-source host/configuration dependencies are separate.
pub struct SourceInventory<'a> {
    entries: Vec<SourceEvidence<'a>>,
    report: Report,
}
impl<'a> SourceInventory<'a> {
    pub fn entries(&self) -> &[SourceEvidence<'a>] {
        &self.entries
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
#[derive(Debug)]
pub enum InventoryError {
    LimitsMismatch,
    Canonical(CanonicalError),
    Stopped(StopReason),
}
impl From<StopReason> for InventoryError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
fn append<'a>(
    entries: &mut Vec<SourceEvidence<'a>>,
    sources: &'a [SourceSnapshot],
    side: Side,
    stage: Stage,
    b: &mut Budget,
) -> Result<(), InventoryError> {
    b.charge(Resource::Work, 1)?;
    for source in sources {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<SourceEvidence<'a>>() as u64,
        )?;
        entries.push(SourceEvidence {
            side,
            stage,
            source,
        });
    }
    Ok(())
}
fn parsed<'a>(
    entries: &mut Vec<SourceEvidence<'a>>,
    parsed: &'a RetainedParse<'_>,
    side: Side,
    b: &mut Budget,
) -> Result<(), InventoryError> {
    append(
        entries,
        parsed.seed().sources().snapshots(),
        side,
        Stage::Seed,
        b,
    )?;
    append(
        entries,
        parsed.execution().sources(),
        side,
        Stage::ParseReply,
        b,
    )?;
    let bundles = BundleMappings::new(&parsed.execution().tree().bundle, b)
        .map_err(InventoryError::Canonical)?;
    for (index, bundle) in bundles.entries().iter().enumerate() {
        append(
            entries,
            &bundle.bundle().sources,
            side,
            Stage::SyntaxBundle(index),
            b,
        )?;
    }
    Ok(())
}
/// Collect original then candidate evidence. Within each side visit seed, parse
/// reply, canonical syntax bundles, then Binding reply. This never merges or
/// repairs identities and does not select a latest revision. A stopped traversal
/// returns no inventory; original reports and evidence remain unchanged.
/// Continue the cumulative Budget; matching Limits do not prove continuity.
pub fn collect<'a>(
    strict: &'a StrictNameInsertion<'_, '_, '_, '_>,
    b: &mut Budget,
) -> Result<SourceInventory<'a>, InventoryError> {
    let declaration = strict.declaration();
    let checked = declaration.reference().insertion().checked();
    if b.limits() != checked.limits() {
        return Err(InventoryError::LimitsMismatch);
    }
    b.poll()?;
    let mut entries = Vec::new();
    parsed(&mut entries, checked.original(), Side::Original, b)?;
    append(
        &mut entries,
        declaration.original().probe().reply().sources(),
        Side::Original,
        Stage::BindingReply,
        b,
    )?;
    parsed(&mut entries, checked.candidate(), Side::Candidate, b)?;
    append(
        &mut entries,
        declaration
            .candidate()
            .references()
            .trace()
            .reply()
            .sources(),
        Side::Candidate,
        Stage::BindingReply,
        b,
    )?;
    Ok(SourceInventory {
        entries,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
impl InventoryError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(s) | Self::Canonical(CanonicalError::Stopped(s)) => Some(*s),
            _ => None,
        }
    }
}
