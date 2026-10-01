use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    diagnostic::Report,
    source::{Digest, SourceId, SourceSnapshot},
    value::SchemaRef,
    view::{FallbackRole, PresentationClass},
};
use nepl3_engine::analysis::region::highlight::{HighlightError, normalize};
use nepl3_engine::analysis::{AnalysisKey, region::*};
fn b() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        source_bytes: 1_000_000,
        ..Limits::default()
    })
}
fn source(s: &str) -> Result<SourceSnapshot, Box<dyn std::error::Error>> {
    SourceSnapshot::new(
        SourceId("doc".into()),
        1,
        "file:///doc".into(),
        s.as_bytes().to_vec(),
        &mut b(),
    )
    .map_err(|e| format!("{e:?}").into())
}
fn key() -> RegionKey {
    let d = Digest::of(b"key");
    RegionKey {
        analysis: AnalysisKey {
            tree_digest: d,
            profile_digest: d,
            execution_digest: d,
            request_digest: d,
        },
        reader_facts_digest: d,
    }
}
fn region(
    s: &SourceSnapshot,
    start: u64,
    end: u64,
    depth: u64,
    priority: u64,
    order: u64,
) -> Result<SourceRegion, Box<dyn std::error::Error>> {
    let span = s.span(start, end).map_err(|e| format!("{e:?}"))?;
    Ok(SourceRegion {
        target: RegionTarget {
            bundle: 0,
            node: Some(0),
            part: RegionPart::Node,
        },
        logical_span: span.clone(),
        span,
        mapping: RegionMapping::Direct,
        priority,
        depth,
        declaration_order: order,
        classes: vec![PresentationClass {
            schema: SchemaRef {
                package: "example".into(),
                revision: 1,
                digest: Digest::of(b"class"),
            },
            name: "content".into(),
            fallback: FallbackRole::Content,
        }],
    })
}
fn reply(s: &SourceSnapshot, regions: Vec<SourceRegion>) -> RegionReply {
    RegionReply {
        key: key(),
        capability: RegionCapability::SyntaxOnly,
        outcome: RegionOutcome::Complete {
            selection: None,
            regions,
        },
        report: Report::default(),
        sources: vec![s.clone()],
    }
}
#[test]
fn shared_spans_preserve_gaps_newlines_and_input() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("ab😀c\r\ndef\rg\nh")?;
    let r = reply(
        &s,
        vec![
            region(&s, 0, s.text().len() as u64, 1, 99, 0)?,
            region(&s, 2, 6, 2, 0, 1)?,
        ],
    );
    let original = r.clone();
    let spans = normalize(&r, &key(), &s, &mut b()).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        spans
            .iter()
            .map(|v| (v.byte_start, v.byte_end, v.region, v.class))
            .collect::<Vec<_>>(),
        vec![
            (0, 2, 0, 0),
            (2, 6, 1, 0),
            (6, 7, 0, 0),
            (9, 12, 0, 0),
            (13, 14, 0, 0),
            (15, 16, 0, 0)
        ]
    );
    assert_eq!(r, original);
    Ok(())
}
#[test]
fn shared_normalization_stops_and_rejects_stale_keys() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("abcdef")?;
    let r = reply(&s, vec![region(&s, 0, 6, 1, 0, 0)?]);
    let mut wrong = key();
    wrong.reader_facts_digest = Digest::of(b"other");
    assert_eq!(
        normalize(&r, &wrong, &s, &mut b()),
        Err(HighlightError::Key)
    );
    let mut limits = b().limits();
    limits.allocation_units = 0;
    assert_eq!(
        normalize(&r, &key(), &s, &mut Budget::new(limits)),
        Err(HighlightError::Stopped(StopReason::AllocationLimit))
    );
    let mut cancelled = b();
    cancelled.stop(StopReason::Cancelled);
    assert_eq!(
        normalize(&r, &key(), &s, &mut cancelled),
        Err(HighlightError::Stopped(StopReason::Cancelled))
    );
    let mut limits = b().limits();
    limits.work = 0;
    assert_eq!(
        normalize(&r, &key(), &s, &mut Budget::new(limits)),
        Err(HighlightError::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}
