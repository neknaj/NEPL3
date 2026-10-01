use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    diagnostic::Report,
    source::{Digest, PositionEncoding, SourceId, SourceSnapshot},
    value::SchemaRef,
    view::{FallbackRole, PresentationClass},
};
use nepl3_engine::analysis::{AnalysisKey, region::*};
use nepl3_lsp::highlight::{HighlightError, normalize};
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
fn overlap_and_utf16_lines() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("ab😀c\r\ndef\rg\nh")?;
    let r = reply(
        &s,
        vec![
            region(&s, 0, s.text().len() as u64, 1, 999, 0)?,
            region(&s, 2, 6, 2, 0, 1)?,
        ],
    );
    let before = r.clone();
    let got = normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        got.iter()
            .map(|s| (s.line, s.character, s.length, s.region))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, 2, 0),
            (0, 2, 2, 1),
            (0, 4, 1, 0),
            (1, 0, 3, 0),
            (2, 0, 1, 0),
            (3, 0, 1, 0)
        ]
    );
    assert_eq!(r, before);
    for pair in got.windows(2) {
        assert!(pair[0].byte_end <= pair[1].byte_start);
    }
    for (encoding, len) in [(PositionEncoding::Utf8, 4), (PositionEncoding::Utf32, 1)] {
        let got = normalize(&r, &key(), &s, encoding, &mut b()).map_err(|e| format!("{e:?}"))?;
        assert_eq!(got[1].length, len);
    }
    Ok(())
}
#[test]
fn empty_gaps_classless_regions_and_precedence() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("abcdef")?;
    let mut transparent = region(&s, 1, 5, 99, 99, 99)?;
    transparent.classes.clear();
    let mut last = region(&s, 2, 4, 2, 5, 1)?;
    let mut extra = last.classes[0].clone();
    extra.name = "annotation".into();
    extra.fallback = FallbackRole::Annotation;
    last.classes.push(extra);
    let r = reply(
        &s,
        vec![
            region(&s, 0, 6, 1, 0, 0)?,
            transparent,
            last,
            region(&s, 2, 4, 2, 4, 0)?,
            region(&s, 6, 6, 9, 9, 9)?,
        ],
    );
    let got = normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        got.iter()
            .map(|s| (s.byte_start, s.byte_end, s.region, s.class))
            .collect::<Vec<_>>(),
        vec![(0, 2, 0, 0), (2, 4, 2, 1), (4, 6, 0, 0)]
    );
    let r = reply(&s, vec![]);
    assert!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
            .map_err(|e| format!("{e:?}"))?
            .is_empty()
    );
    Ok(())
}
#[test]
fn stale_source_and_stops_do_not_produce_partial_success() -> Result<(), Box<dyn std::error::Error>>
{
    let s = source("abc")?;
    let r = reply(&s, vec![region(&s, 0, 3, 1, 0, 0)?]);
    let mut stale = key();
    stale.reader_facts_digest = Digest::of(b"other");
    assert_eq!(
        normalize(&r, &stale, &s, PositionEncoding::Utf16, &mut b()),
        Err(HighlightError::Key)
    );
    let other = source("xyz")?;
    assert!(normalize(&r, &key(), &other, PositionEncoding::Utf16, &mut b()).is_err());
    for limit in [0, 1, 10, 100] {
        let mut budget = Budget::new(Limits {
            work: limit,
            allocation_units: 10_000,
            ..Limits::default()
        });
        assert_eq!(
            normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut budget),
            Err(HighlightError::Stopped(StopReason::WorkLimit))
        );
    }
    let mut budget = b();
    budget.cancel();
    assert_eq!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut budget),
        Err(HighlightError::Stopped(StopReason::Cancelled))
    );
    let mut budget = Budget::new(Limits {
        work: 10_000,
        allocation_units: 0,
        ..Limits::default()
    });
    assert_eq!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut budget),
        Err(HighlightError::Stopped(StopReason::AllocationLimit))
    );
    let mut invalid = r;
    invalid.outcome = RegionOutcome::Stopped(StopReason::NodeLimit);
    assert_eq!(
        normalize(&invalid, &key(), &s, PositionEncoding::Utf16, &mut b()),
        Err(HighlightError::Stopped(StopReason::NodeLimit))
    );
    Ok(())
}

#[test]
fn tie_breaks_and_adjacent_winner_fragments() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("abcdef")?;
    for (left, right, winner) in [
        (region(&s, 0, 6, 1, 0, 0)?, region(&s, 2, 4, 1, 0, 0)?, 1),
        (region(&s, 2, 4, 1, 0, 9)?, region(&s, 2, 4, 1, 0, 2)?, 1),
        (region(&s, 2, 4, 1, 0, 2)?, region(&s, 2, 4, 1, 0, 2)?, 0),
    ] {
        let r = reply(&s, vec![left, right]);
        let got = normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
            .map_err(|e| format!("{e:?}"))?;
        assert!(
            got.iter()
                .any(|v| v.byte_start <= 2 && v.byte_end >= 4 && v.region == winner)
        );
    }
    let r = reply(
        &s,
        vec![region(&s, 0, 6, 2, 0, 0)?, region(&s, 1, 5, 1, 0, 0)?],
    );
    let got = normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].byte_start, got[0].byte_end), (0, 6));
    Ok(())
}

#[test]
fn terminator_only_and_unicode_scalars() -> Result<(), Box<dyn std::error::Error>> {
    let s = source("\r\n\r\n")?;
    let r = reply(
        &s,
        vec![region(&s, 0, 4, 1, 0, 0)?, region(&s, 1, 2, 2, 0, 0)?],
    );
    assert!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
            .map_err(|e| format!("{e:?}"))?
            .is_empty()
    );
    let s = source("\u{feff}日本e\u{301}\n😀")?;
    let r = reply(&s, vec![region(&s, 0, s.text().len() as u64, 1, 0, 0)?]);
    for (enc, lengths) in [
        (PositionEncoding::Utf8, vec![12, 4]),
        (PositionEncoding::Utf16, vec![5, 2]),
        (PositionEncoding::Utf32, vec![5, 1]),
    ] {
        let got = normalize(&r, &key(), &s, enc, &mut b()).map_err(|e| format!("{e:?}"))?;
        assert_eq!(got.iter().map(|v| v.length).collect::<Vec<_>>(), lengths);
    }
    Ok(())
}

#[test]
fn empty_source_closure_and_duplicate_identity_conflict() -> Result<(), Box<dyn std::error::Error>>
{
    let s = source("a")?;
    let mut r = reply(&s, vec![]);
    r.sources.clear();
    assert!(matches!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b()),
        Err(HighlightError::Source(_))
    ));
    r.sources.push(s.clone());
    r.sources.push(
        SourceSnapshot::new(
            SourceId("doc".into()),
            1,
            "file:///other".into(),
            b"a".to_vec(),
            &mut b(),
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    assert_eq!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b()),
        Err(HighlightError::Range)
    );
    Ok(())
}

#[test]
fn long_identity_cost_is_prepaid() -> Result<(), Box<dyn std::error::Error>> {
    let s = SourceSnapshot::new(
        SourceId("x".repeat(4096)),
        1,
        "file:///doc".into(),
        b"a".to_vec(),
        &mut b(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let r = reply(&s, vec![region(&s, 0, 1, 1, 0, 0)?]);
    let mut limited = Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1024,
        ..Limits::default()
    });
    assert_eq!(
        normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut limited),
        Err(HighlightError::Stopped(StopReason::AllocationLimit))
    );
    let got = normalize(&r, &key(), &s, PositionEncoding::Utf16, &mut b())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(got.len(), 1);
    Ok(())
}

#[test]
fn source_identity_allocation_is_paid_once_by_core_index() -> Result<(), Box<dyn std::error::Error>>
{
    let mut usages = Vec::new();
    for name in ["a".to_owned(), "字".repeat(1024)] {
        let source = SourceSnapshot::new(
            SourceId(name),
            1,
            "file:///same".into(),
            b"word".to_vec(),
            &mut b(),
        )
        .map_err(|e| format!("{e:?}"))?;
        let reply = reply(&source, vec![region(&source, 0, 4, 1, 0, 0)?]);
        let mut budget = b();
        let spans = normalize(
            &reply,
            &key(),
            &source,
            PositionEncoding::Utf16,
            &mut budget,
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(spans.len(), 1);
        usages.push(budget.usage());
    }
    // Same text, line table and output vectors: only one owned SourceId copy
    // varies with its UTF-8 byte length. position() allocates no temporary Span.
    assert_eq!(
        usages[1].allocation_units - usages[0].allocation_units,
        3072 - 1
    );
    assert_eq!(usages[1].work - usages[0].work, 7 * (3072 - 1));
    Ok(())
}
