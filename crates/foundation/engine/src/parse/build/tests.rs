use super::*;
use alloc::{format, vec};
use nepl3_core::{budget::Limits, source::SourceId};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 1000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn source(name: &str, revision: u64, uri: &str, byte: u8) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId(name.into()),
        revision,
        uri.into(),
        vec![byte],
        &mut budget(),
    )
}

#[test]
fn closure_merge_preserves_source_order_full_identity_and_conflicts() -> Result<(), String> {
    let originals = [
        source("z", 0, "memory:z", b'z'),
        source("日本語", 0, "memory:a", b'a'),
        source("日本語", u64::MAX, "memory:b", b'b'),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| format!("{e:?}"))?;
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut arena = ParseArena::default();
        for i in order {
            arena
                .extend_sources(&originals[i..=i], &mut budget())
                .map_err(|e| format!("{e:?}"))?;
        }
        let expected = order.map(|i| originals[i].clone()).to_vec();
        arena
            .extend_sources(&originals, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(arena.sources, expected);
        for (uri, byte) in [("changed:uri", b'a'), ("memory:a", b'x')] {
            let conflict = source("日本語", 0, uri, byte).map_err(|e| format!("{e:?}"))?;
            assert_eq!(
                arena.extend_sources(&[conflict], &mut budget()),
                Err(SyntaxError::Source(SourceError::IdentityConflict))
            );
            assert_eq!(arena.sources, expected);
        }
        let mut stopped = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(arena.extend_sources(&originals, &mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        assert_eq!(arena.sources, expected);
    }
    Ok(())
}
