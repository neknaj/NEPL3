use super::*;
use crate::package::PackageIdentity;
use alloc::format;
use nepl3_core::{
    budget::{Limits, StopReason, Usage},
    schema::{SchemaDescriptor, SchemaError},
    source::Digest,
    source::{SourceId, SourceSnapshot},
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 10_000,
        source_bytes: 10_000,
        depth: 100,
        diagnostics: 10,
        ..Limits::default()
    })
}
fn fixture(
    padding: usize,
    engine: bool,
    foundation: bool,
) -> Result<(SchemaRegistry, EntryContext, Span), ParseError> {
    let mut registry = SchemaRegistry::default();
    let mut names = (0..padding)
        .map(|i| format!("pad{i:02}"))
        .collect::<Vec<_>>();
    if engine {
        names.push("nepl3.engine".into());
    }
    if foundation {
        names.push("nepl3.foundation".into());
    }
    for package in names {
        let d = SchemaDescriptor {
            package,
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    // This seam constructs diagnostic arguments, not a structural proof. The
    // production collector owns subsequent schema validation.
    let entry = EntryContext {
        package: PackageIdentity {
            schema: SchemaRef {
                package: "fixture".into(),
                revision: 1,
                digest: Digest([0; 32]),
            },
            semantic_digest: Digest([1; 32]),
        },
        alias: "X".into(),
        category: "Node".into(),
        mode: "Code".into(),
    };
    let source = SourceSnapshot::new(
        SourceId("s".into()),
        0,
        "mem:s".into(),
        b"bad".to_vec(),
        &mut budget(),
    )?;
    Ok((registry, entry, source.span(0, 3)?))
}
#[test]
fn recovery_diagnostic_meters_both_selections_before_allocating() -> Result<(), ParseError> {
    for missing in [false, true] {
        let (compact, entry, span) = fixture(0, true, true)?;
        let mut base = budget();
        let expected = recovery(&entry, &span, missing, &compact, &mut base)?;
        let (padded, entry, span) = fixture(32, true, true)?;
        let mut b = budget();
        assert_eq!(recovery(&entry, &span, missing, &padded, &mut b)?, expected);
        // Each prefix contributes to engine and foundation selection, with
        // package UTF-8 lengths 12 and 16 respectively, plus 9 per compare.
        assert_eq!(
            b.usage(),
            Usage {
                work: base.usage().work + 32 * ((5 + 12 + 9) + (5 + 16 + 9)),
                ..base.usage()
            }
        );
        let first = 1 + 32 * (5 + 12 + 9) + (12 + 12 + 9);
        let both = first + 1 + 32 * (5 + 16 + 9) + (12 + 16 + 9) + (16 + 16 + 9);
        for work in [0, 1, first - 1, first, both - 1] {
            let mut limited = Budget::new(Limits {
                work,
                allocation_units: 0,
                ..budget().limits()
            });
            let result = recovery(&entry, &span, missing, &padded, &mut limited);
            assert_eq!(result, Err(ParseError::Stopped(StopReason::WorkLimit)));
            assert_eq!(limited.usage().allocation_units, 0);
            assert_eq!(limited.usage().nodes, 0);
        }
        let mut at_boundary = Budget::new(Limits {
            work: both,
            allocation_units: 0,
            ..budget().limits()
        });
        assert_eq!(
            recovery(&entry, &span, missing, &padded, &mut at_boundary),
            Err(ParseError::Stopped(StopReason::AllocationLimit))
        );
        let mut cancelled = budget();
        cancelled.cancel();
        assert_eq!(
            recovery(&entry, &span, missing, &padded, &mut cancelled),
            Err(ParseError::Stopped(StopReason::Cancelled))
        );
        assert_eq!(cancelled.usage(), Usage::default());
    }
    Ok(())
}
#[test]
fn recovery_diagnostic_keeps_missing_schema_order() -> Result<(), ParseError> {
    for (engine, foundation) in [(false, false), (false, true), (true, false)] {
        let (r, e, s) = fixture(0, engine, foundation)?;
        let mut b = budget();
        assert_eq!(
            recovery(&e, &s, true, &r, &mut b),
            Err(ParseError::Schema(SchemaError::UnknownSchema))
        );
        assert_eq!(b.usage().allocation_units, 0);
        let work = match (engine, foundation) {
            (false, false) => 1,
            (false, true) => 1 + 16 + 12 + 9,
            _ => 1 + 12 + 12 + 9 + 1 + 12 + 16 + 9,
        };
        assert_eq!(b.usage().work, work);
    }
    Ok(())
}
