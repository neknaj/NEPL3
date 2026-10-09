use super::*;
use alloc::vec;
use nepl3_core::source::Span;
use nepl3_core::{
    budget::{Limits, Usage},
    origin::{Mapping, MappingKind},
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000,
        allocation_units: 100_000,
        source_bytes: 1000,
        depth: 100,
        nodes: 1000,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
#[test]
fn mapping_scope_copy_charges_full_and_partial_storage_before_cloning() -> Result<(), ReaderError> {
    let mut spans = Vec::new();
    for id in ["a", "bb", "ccc"] {
        let source = SourceSnapshot::new(
            SourceId(id.into()),
            0,
            "memory:scope".into(),
            vec![b'x'],
            &mut budget(),
        )?;
        spans.push(source.span(0, 1)?);
    }
    let prior = vec![Mapping {
        source: spans[0].clone(),
        target: spans[1].clone(),
        kind: MappingKind::Transformed,
    }];
    let added = vec![Mapping {
        source: spans[1].clone(),
        target: spans[2].clone(),
        kind: MappingKind::Transformed,
    }];
    // Existing CopyCost: Vec slot; each Mapping slot; two Span and String
    // slots plus each SourceId's logical copy bytes. This formula does not call it.
    let work = 1 + (5 + 1 + 2) + (5 + 2 + 3);
    let allocation = core::mem::size_of::<Vec<Mapping>>() as u64
        + 2 * (core::mem::size_of::<Mapping>()
            + 2 * core::mem::size_of::<Span>()
            + 2 * core::mem::size_of::<String>()) as u64
        + 1
        + 2
        + 2
        + 3;
    let expected = Usage {
        work,
        allocation_units: allocation,
        ..Usage::default()
    };
    let mut b = budget();
    let copied = dispatch_mappings(&prior, &added, &mut b)?;
    assert_eq!(copied, vec![prior[0].clone(), added[0].clone()]);
    assert_eq!(b.usage(), expected);
    // Span identities may share immutable storage; the mapping vector is owned.
    assert_ne!(copied.as_ptr(), prior.as_ptr());
    for allocation_stop in [false, true] {
        let mut limits = budget().limits();
        let reason = if allocation_stop {
            limits.allocation_units = allocation - 1;
            StopReason::AllocationLimit
        } else {
            limits.work = work - 1;
            StopReason::WorkLimit
        };
        let mut b = Budget::new(limits);
        assert!(
            matches!(dispatch_mappings(&prior, &added, &mut b), Err(ReaderError::Stopped(r)) if r == reason)
        );
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(
            b.usage(),
            Usage {
                work: work - 3,
                allocation_units: if allocation_stop {
                    allocation - 3
                } else {
                    allocation
                },
                ..Usage::default()
            }
        );
        assert_eq!(prior[0].target, added[0].source);
    }
    let mut b = budget();
    b.charge(Resource::Work, 7)?;
    b.cancel();
    let before = b.usage();
    assert!(matches!(
        dispatch_mappings(&prior, &added, &mut b),
        Err(ReaderError::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(b.usage(), before);
    Ok(())
}
