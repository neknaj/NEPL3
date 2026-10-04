#[path = "sources/state.rs"]
mod state;
use super::*;
use nepl3_core::budget::{Resource, StopReason};
use nepl3_engine::analysis::insertion::{
    quality::StrictNameInsertion,
    sources::{self, InventoryError, Side, Stage},
};

pub(super) fn verify(
    strict: &StrictNameInsertion<'_, '_, '_, '_>,
    b: &mut Budget,
) -> Result<(), String> {
    let before = strict.report().clone();
    let mut measure = Budget::new(b.limits());
    let inventory = sources::collect(strict, &mut measure).map_err(err)?;
    assert!(!inventory.entries().is_empty());
    let declaration = strict.declaration();
    let checked = declaration.reference().insertion().checked();
    let mut expected = Vec::new();
    expect_parse(checked.original(), Side::Original, &mut expected);
    for source in declaration.original().probe().reply().sources() {
        expected.push((Side::Original, Stage::BindingReply, source));
    }
    expect_parse(checked.candidate(), Side::Candidate, &mut expected);
    for source in declaration
        .candidate()
        .references()
        .trace()
        .reply()
        .sources()
    {
        expected.push((Side::Candidate, Stage::BindingReply, source));
    }
    assert_eq!(inventory.entries().len(), expected.len());
    for (row, (side, stage, source)) in inventory.entries().iter().zip(&expected) {
        assert_eq!((row.side, row.stage), (*side, *stage));
        assert!(core::ptr::eq(row.source, *source));
    }

    for side in [Side::Original, Side::Candidate] {
        for stage in [Stage::Seed, Stage::SyntaxBundle(0), Stage::BindingReply] {
            assert!(
                inventory
                    .entries()
                    .iter()
                    .any(|v| v.side == side && v.stage == stage),
                "{side:?} {stage:?}"
            );
        }
    }
    let mut candidate_seen = false;
    for row in inventory.entries() {
        if row.side == Side::Candidate {
            candidate_seen = true;
        } else {
            assert!(!candidate_seen);
        }
    }
    state::verify(&inventory, b)?;
    let use_ = measure.usage();
    assert_eq!(inventory.report().usage, use_);
    assert_eq!(use_.diagnostics, 0);
    assert_eq!(use_.events, 0);
    assert!(inventory.report().diagnostics.is_empty());
    for (resource, cap, cost, reason) in [
        (
            Resource::Work,
            b.limits().work,
            use_.work,
            StopReason::WorkLimit,
        ),
        (
            Resource::Nodes,
            b.limits().nodes,
            use_.nodes,
            StopReason::NodeLimit,
        ),
        (
            Resource::AllocationUnits,
            b.limits().allocation_units,
            use_.allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        let mut exact = Budget::new(b.limits());
        exact.charge(resource, cap - cost).map_err(err)?;
        let again = sources::collect(strict, &mut exact).map_err(err)?;
        assert_eq!(again.entries().len(), inventory.entries().len());
        for (a, c) in inventory.entries().iter().zip(again.entries()) {
            assert_eq!((a.side, a.stage), (c.side, c.stage));
            assert!(core::ptr::eq(a.source, c.source));
        }
        let mut short = Budget::new(b.limits());
        short.charge(resource, cap - (cost - 1)).map_err(err)?;
        let Err(error) = sources::collect(strict, &mut short) else {
            return Err("inventory accepted short budget".into());
        };
        assert_eq!(error.stop_reason(), Some(reason));
    }
    let base = b.limits().depth - use_.depth;
    let mut depth_exact = Budget::new(b.limits());
    let exact = depth_exact
        .with_depth_at_least(base, |b| sources::collect(strict, b))
        .map_err(err)?;
    assert_eq!(exact.entries().len(), inventory.entries().len());
    assert_eq!(depth_exact.usage().depth, b.limits().depth);
    assert_eq!(depth_exact.current_depth(), 0);
    let mut depth_short = Budget::new(b.limits());
    let Err(error) = depth_short.with_depth_at_least(base + 1, |b| sources::collect(strict, b))
    else {
        return Err("inventory accepted shallow depth".into());
    };
    assert!(matches!(
        error,
        InventoryError::Canonical(nepl3_core::syntax::canonical::CanonicalError::Stopped(
            StopReason::DepthLimit
        ))
    ));
    assert_eq!(depth_short.current_depth(), 0);
    assert_eq!(depth_short.poll(), Err(StopReason::DepthLimit));
    let mut cancelled = Budget::new(b.limits());
    cancelled.cancel();
    let Err(error) = sources::collect(strict, &mut cancelled) else {
        return Err("cancelled inventory".into());
    };
    assert_eq!(error.stop_reason(), Some(StopReason::Cancelled));
    let mut limits = b.limits();
    limits.work -= 1;
    let mut other = Budget::new(limits);
    other.cancel();
    assert!(matches!(
        sources::collect(strict, &mut other),
        Err(InventoryError::LimitsMismatch)
    ));
    assert_eq!(other.usage(), Budget::new(limits).usage());
    assert_eq!(strict.report(), &before);
    let collected = sources::collect(strict, b).map_err(err)?;
    assert_eq!(collected.entries().len(), inventory.entries().len());
    Ok(())
}

fn expect_parse<'a>(
    parsed: &'a nepl3_engine::parse::RetainedParse<'_>,
    side: Side,
    rows: &mut Vec<(Side, Stage, &'a SourceSnapshot)>,
) {
    for source in parsed.seed().sources().snapshots() {
        rows.push((side, Stage::Seed, source));
    }
    for source in parsed.execution().sources() {
        rows.push((side, Stage::ParseReply, source));
    }
    let mut index = 0;
    expect_bundle(&parsed.execution().tree().bundle, side, &mut index, rows);
}
// Independent fixture traversal: no production BundleMappings or collect call.
fn expect_bundle<'a>(
    bundle: &'a nepl3_core::syntax::SyntaxBundle,
    side: Side,
    index: &mut usize,
    rows: &mut Vec<(Side, Stage, &'a SourceSnapshot)>,
) {
    let stage = Stage::SyntaxBundle(*index);
    *index += 1;
    for source in &bundle.sources {
        rows.push((side, stage, source));
    }
    let mut pending = vec![bundle.root];
    let mut seen = vec![false; bundle.nodes.len()];
    let mut foreign = Vec::new();
    while let Some(id) = pending.pop() {
        let i = id.0 as usize;
        if seen[i] {
            continue;
        }
        seen[i] = true;
        for field in &bundle.nodes[i].fields {
            if let nepl3_core::syntax::FieldValue::Foreign(value) = field {
                foreign.push(&value.bundle);
            }
        }
        for field in bundle.nodes[i].fields.iter().rev() {
            match field {
                nepl3_core::syntax::FieldValue::Child(id) => pending.push(*id),
                nepl3_core::syntax::FieldValue::Children(ids) => {
                    pending.extend(ids.iter().rev().copied())
                }
                _ => {}
            }
        }
    }
    for child in foreign {
        expect_bundle(child, side, index, rows);
    }
}
