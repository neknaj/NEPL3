use super::*;
use alloc::format;
use nepl3_core::{
    diagnostic::TraceOverflow,
    origin::{Mapping, MappingKind},
    source::SourceId,
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
fn accepted() -> Result<AcceptedReport, SourceError> {
    let source = SourceSnapshot::new(
        SourceId("original".into()),
        7,
        "memory:original".into(),
        vec![b'x', b'y'],
        &mut budget(),
    )?;
    let span = source.span(0, 1)?;
    let target = source.span(1, 2)?;
    Ok(AcceptedReport {
        report: Report {
            trace_overflow: Some(TraceOverflow { dropped: 7 }),
            ..Report::default()
        },
        sources: vec![source],
        source_maps: vec![Mapping {
            source: span.clone(),
            target,
            kind: MappingKind::Transformed,
        }],
    })
}

#[test]
fn hard_failure_returns_owned_collector_without_copying_or_new_budget() -> Result<(), String> {
    let original = accepted().map_err(|e| format!("{e:?}"))?;
    let sources = original.sources.as_ptr();
    let maps = original.source_maps.as_ptr();
    let b = Budget::new(Limits {
        allocation_units: 0,
        work: 0,
        ..budget().limits()
    });
    let Err(failure) = owned_failure(ReaderError::ProviderContract, original, &b) else {
        return Err("a contract violation is not a successful or stopped reply".into());
    };
    assert_eq!(failure.error, ReaderError::ProviderContract);
    assert_eq!(failure.accepted.sources.as_ptr(), sources);
    assert_eq!(failure.accepted.source_maps.as_ptr(), maps);
    assert_eq!(failure.accepted.sources[0].identity().revision, 7);
    assert_eq!(
        failure.accepted.report.trace_overflow,
        Some(TraceOverflow { dropped: 7 })
    );
    assert_eq!(b.usage().allocation_units, 0);
    Ok(())
}

#[test]
fn stopped_owned_collector_retains_closure_and_original_stop() -> Result<(), String> {
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
        StopReason::EventLimit,
    ] {
        let original = accepted().map_err(|e| format!("{e:?}"))?;
        let sources = original.sources.as_ptr();
        let maps = original.source_maps.as_ptr();
        let mut b = budget();
        b.stop(reason);
        let Ok(ReadReply::Stopped {
            reason: actual,
            sources: retained,
            source_maps,
            report,
        }) = owned_failure(ReaderError::Stopped(reason), original, &b)
        else {
            return Err("stop must preserve its collector".into());
        };
        assert_eq!(actual, reason);
        assert_eq!(retained.as_ptr(), sources);
        assert_eq!(source_maps.as_ptr(), maps);
        assert_eq!(report.usage, b.usage());
        assert_eq!(b.poll(), Err(reason));
        assert_eq!(report.trace_overflow, Some(TraceOverflow { dropped: 7 }));
    }
    Ok(())
}
