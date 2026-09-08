use super::*;
use alloc::vec;
use nepl3_core::{
    budget::Usage,
    diagnostic::Report,
    source::{Digest, SourceId, SourceRef},
};

fn prefix() -> Prefix {
    Prefix {
        scope: Rc::new(TokenizationScope {
            operation_id: "original".into(),
            profile_digest: Digest([7; 32]),
            snapshot: SourceRef {
                source_id: SourceId("input".into()),
                revision: 7,
                digest: Digest([8; 32]),
            },
        }),
        limits: Limits {
            source_bytes: 99,
            work: 99,
            depth: 99,
            nodes: 99,
            allocation_units: 99,
            output_bytes: 99,
            diagnostics: 99,
            events: 99,
        },
        lengths: [0; 4],
        overflow: Some(TraceOverflow { dropped: 3 }),
    }
}
fn live() -> AcceptedReport {
    AcceptedReport {
        report: Report {
            usage: Usage {
                work: 21,
                ..Usage::default()
            },
            trace_overflow: Some(TraceOverflow { dropped: 11 }),
            ..Report::default()
        },
        sources: vec![],
        source_maps: vec![],
    }
}

#[test]
fn every_short_prefix_is_rejected_without_issuing_an_accepted_proof() {
    for field in 0..4 {
        let mut saved = prefix();
        saved.lengths[field] = 1;
        assert!(saved.restore(live()).is_err());
    }
}

#[test]
fn recovery_keeps_scope_limits_and_usage_but_restores_entry_overflow() -> Result<(), ReaderError> {
    let saved = prefix();
    let scope = Rc::clone(&saved.scope);
    let limits = saved.limits;
    let original = saved
        .restore(live())
        .map_err(|()| ReaderError::Continuation)?;
    assert!(Rc::ptr_eq(&scope, &original.scope));
    assert_eq!(original.limits, limits);
    assert_eq!(
        original.report.trace_overflow,
        Some(TraceOverflow { dropped: 3 })
    );
    assert_eq!(original.report.usage.work, 21);
    Ok(())
}
