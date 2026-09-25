use super::*;
use alloc::{format, vec::Vec};
use nepl3_core::{
    budget::Limits,
    source::{Digest, SourceId},
};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        source_bytes: 1000,
        allocation_units: 1_000_000,
        ..Limits::default()
    })
}

fn source() -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("test".into()),
        1,
        "memory:test".into(),
        Vec::new(),
        &mut budget(),
    )
}

#[test]
fn canonical_order_preserves_all_fields_and_rejects_duplicate_identity() -> Result<(), WireError> {
    let source = source()?;
    let references = [
        SourceRef {
            source_id: SourceId("b".into()),
            revision: 0,
            digest: Digest([0; 32]),
        },
        SourceRef {
            source_id: SourceId("a".into()),
            revision: 2,
            digest: Digest([1; 32]),
        },
        SourceRef {
            source_id: SourceId("a".into()),
            revision: 1,
            digest: Digest([2; 32]),
        },
    ];
    let mut entries: Vec<_> = references.iter().cloned().map(|r| (r, &source)).collect();
    sort(&mut entries, &mut budget())?;
    assert_eq!(
        entries.iter().map(|entry| &entry.0).collect::<Vec<_>>(),
        [&references[2], &references[1], &references[0]]
    );
    for digest in [Digest([2; 32]), Digest([3; 32])] {
        let mut duplicate = references[2].clone();
        duplicate.digest = digest;
        let mut entries = [(duplicate, &source), (references[2].clone(), &source)];
        assert!(matches!(
            sort(&mut entries, &mut budget()),
            Err(WireError::Source(SourceError::IdentityConflict))
        ));
        // The descending prefix enters heapsort before the nonadjacent
        // duplicate can be found. Both equal and conflicting digests reject.
        let mut duplicate = references[2].clone();
        duplicate.digest = digest;
        let mut entries = [
            (references[0].clone(), &source),
            (references[2].clone(), &source),
            (references[1].clone(), &source),
            (duplicate, &source),
        ];
        assert!(matches!(
            sort(&mut entries, &mut budget()),
            Err(WireError::Source(SourceError::IdentityConflict))
        ));
    }
    Ok(())
}

#[test]
fn source_order_scales_with_count_and_identifier_length_and_stops() -> Result<(), WireError> {
    let source = source()?;
    let mut short_work = [[0_u64; 3]; 3];
    for prefix in [0, 128] {
        for (layout, short_layout) in short_work.iter_mut().enumerate() {
            let mut previous = None;
            for (scale, count) in [128, 256, 512].into_iter().enumerate() {
                let mut input: Vec<_> = (0..count)
                    .map(|id| {
                        (
                            SourceRef {
                                source_id: SourceId(format!("{}-{id:06}", "x".repeat(prefix))),
                                revision: 0,
                                digest: Digest([0; 32]),
                            },
                            &source,
                        )
                    })
                    .collect();
                if layout == 1 {
                    input.reverse();
                } else if layout == 2 {
                    input.rotate_left(count / 3);
                }
                let mut entries = input.clone();
                let mut measured = budget();
                sort(&mut entries, &mut measured)?;
                let work = measured.usage().work;
                if prefix == 0 {
                    short_layout[scale] = work;
                } else {
                    // The same comparisons with 128 extra prefix bytes must
                    // charge their byte work, not only comparison count.
                    assert!(work > short_layout[scale] * 8);
                }
                if let Some(prior) = previous {
                    // Doubling n permits n log n growth, rejects quadratic growth.
                    assert!(work < prior * 3, "{count}: {work} >= 3 * {prior}");
                }
                previous = Some(work);
                for (id, entry) in entries.iter().enumerate() {
                    assert_eq!(
                        entry.0.source_id.0,
                        format!("{}-{id:06}", "x".repeat(prefix))
                    );
                }
                for limit in [work, work - 1] {
                    let mut entries = input.clone();
                    let mut limited = Budget::new(Limits {
                        work: limit,
                        ..Limits::default()
                    });
                    let result = sort(&mut entries, &mut limited);
                    if limit == work {
                        result?;
                    } else {
                        assert!(matches!(
                            result,
                            Err(WireError::Stopped(StopReason::WorkLimit))
                        ));
                        assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
                        assert!(matches!(
                            sort(&mut entries, &mut limited),
                            Err(WireError::Stopped(StopReason::WorkLimit))
                        ));
                    }
                }
                // Sorting uses only the caller-owned working array.
                assert_eq!(measured.usage().allocation_units, 0);
            }
        }
    }
    Ok(())
}

#[test]
fn empty_singleton_and_odd_unicode_source_sets_are_canonical() -> Result<(), WireError> {
    let source = source()?;
    for names in [&[][..], &["界"][..], &["😀", "界", "a", "あ", "z"][..]] {
        let mut entries: Vec<_> = names
            .iter()
            .map(|name| {
                (
                    SourceRef {
                        source_id: SourceId((*name).into()),
                        revision: 1,
                        digest: Digest([0; 32]),
                    },
                    &source,
                )
            })
            .collect();
        let mut cancelled = budget();
        cancelled.stop(StopReason::Cancelled);
        assert!(matches!(
            sort(&mut entries, &mut cancelled),
            Err(WireError::Stopped(StopReason::Cancelled))
        ));
        sort(&mut entries, &mut budget())?;
        let expected: &[&str] = match names.len() {
            0 => &[],
            1 => &["界"],
            _ => &["a", "z", "あ", "界", "😀"],
        };
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.0.source_id.0.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
    Ok(())
}
