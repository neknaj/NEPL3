use super::*;
use nepl3_core::{
    diagnostic::TraceOverflow,
    origin::{Mapping, MappingKind},
    source::{SourceId, SourceSnapshot},
    value::NdfValue,
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10000,
        work: 100000,
        depth: 100,
        nodes: 10000,
        allocation_units: 100000,
        output_bytes: 10000,
        diagnostics: 10,
        events: 10,
    })
}

#[test]
fn tokenizer_failure_moves_source_closure_with_exhausted_budget() -> Result<(), ReaderError> {
    let source = SourceSnapshot::new(
        SourceId("original".into()),
        7,
        "memory:original".into(),
        vec![b'x', b'y'],
        &mut budget(),
    )?;
    let scope = TokenizationScope {
        operation_id: "operation".into(),
        profile_digest: Digest([0; 32]),
        snapshot: source.reference(),
    };
    let mut accepted = AcceptedTokenizationReport::empty(scope, &mut budget())?;
    accepted.source_maps.push(Mapping {
        source: source.span(0, 1)?,
        target: source.span(1, 2)?,
        kind: MappingKind::Transformed,
    });
    accepted.sources.push(source);
    accepted.report.trace_overflow = Some(TraceOverflow { dropped: 7 });
    let sources = accepted.sources.as_ptr();
    let maps = accepted.source_maps.as_ptr();
    let mut b = budget();
    b.charge(Resource::Work, 11)?;
    b.stop(StopReason::AllocationLimit);
    let before = b.usage();
    let seed = seed_failure(ReaderError::ProviderContract, accepted, &b);
    assert_eq!(seed.error, ReaderError::ProviderContract);
    assert_eq!(seed.accepted.sources.as_ptr(), sources);
    assert_eq!(seed.accepted.source_maps.as_ptr(), maps);
    let runtime::AcceptedReport {
        report,
        sources: owned,
        source_maps,
    } = seed.accepted;
    let current = ReaderCheckpoint {
        cursor: 1,
        state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        diagnostics: report.diagnostics,
        events: report.events,
        trace_overflow: report.trace_overflow,
        sources: owned,
        source_maps,
    };
    let failure = current_failure(ReaderError::Continuation, current, &b);
    assert_eq!(failure.error, ReaderError::Continuation);
    assert_eq!(failure.accepted.sources.as_ptr(), sources);
    assert_eq!(failure.accepted.source_maps.as_ptr(), maps);
    assert_eq!(failure.accepted.sources[0].identity().revision, 7);
    assert_eq!(
        failure.accepted.report.trace_overflow,
        Some(TraceOverflow { dropped: 7 })
    );
    assert_eq!(failure.accepted.report.usage, before);
    assert_eq!(b.usage(), before);
    assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
    Ok(())
}
