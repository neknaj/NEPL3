use super::*;
use crate::model::{DocRoot, SentenceRef};
use nepl3_core::budget::Limits;
fn b() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        depth: 100,
        nodes: 1000,
        allocation_units: 1_000_000,
        ..Limits::default()
    })
}
#[test]
fn normal_form_flattens_only_sequences_and_retains_explicit_break() -> Result<(), ShapeError> {
    let kinds = vec![
        DocKind::Text { text: "a".into() },
        DocKind::Text {
            text: String::new(),
        },
        DocKind::Text { text: "b".into() },
        DocKind::Concat {
            inlines: vec![InlineRef(0), InlineRef(1), InlineRef(2)],
        },
        DocKind::Break,
        DocKind::Text { text: "c".into() },
        DocKind::Text {
            text: "read".into(),
        },
        DocKind::Ruby {
            base: InlineRef(5),
            reading: InlineRef(6),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(3), InlineRef(4), InlineRef(7)],
        },
    ];
    let input = DocValue {
        root: DocRoot::Sentence(SentenceRef(8)),
        nodes: kinds
            .into_iter()
            .map(|kind| DocNode {
                locations: Vec::new(),
                kind,
                origin: None,
                span: None,
            })
            .collect(),
        embeds: Default::default(),
    };
    let output = value(input, &mut Vec::new(), &mut b())?;
    output.validate_shape(&mut b())?;
    assert_eq!(output.nodes.len(), 6);
    assert_eq!(output.nodes[0].kind, DocKind::Text { text: "ab".into() });
    assert_eq!(output.nodes[1].kind, DocKind::Break);
    assert_eq!(
        output.nodes[4].kind,
        DocKind::Ruby {
            base: InlineRef(2),
            reading: InlineRef(3)
        }
    );
    assert_eq!(
        output.nodes[5].kind,
        DocKind::Sentence {
            inlines: vec![InlineRef(0), InlineRef(1), InlineRef(4)]
        }
    );
    let again = value(output.clone(), &mut Vec::new(), &mut b())?;
    assert_eq!(again, output);
    Ok(())
}
fn text_node(text: &str) -> DocNode {
    DocNode {
        locations: Vec::new(),
        kind: DocKind::Text { text: text.into() },
        origin: None,
        span: None,
    }
}

#[test]
fn shared_text_run_copies_once_without_mutating_shared_nodes() -> Result<(), ShapeError> {
    let original = text_node("日本語");
    let mut nodes = vec![original.clone()];
    let mut origins = Vec::new();
    let items = vec![InlineRef(0); 256];
    let mut budget = b();
    let output = sequence(&items, &mut nodes, &mut origins, &mut budget)?;
    assert_eq!(output, vec![InlineRef(1)]);
    assert_eq!(
        nodes.len(),
        2,
        "only the completed Text run is materialized"
    );
    assert_eq!(
        nodes[0], original,
        "other parents still own the original node"
    );
    assert_eq!(
        nodes[1].kind,
        DocKind::Text {
            text: "日本語".repeat(256)
        }
    );
    assert_eq!(budget.usage().nodes, 1);
    assert!(origins.is_empty());
    Ok(())
}

#[test]
fn adjacent_text_run_work_and_allocation_grow_linearly() -> Result<(), ShapeError> {
    let measure = |count: usize| -> Result<nepl3_core::budget::Usage, ShapeError> {
        let mut nodes = vec![text_node("0123456789abcdef0123456789abcdef")];
        let mut budget = Budget::new(Limits {
            work: 100_000_000,
            allocation_units: 100_000_000,
            nodes: 10_000,
            depth: 100,
            ..Limits::default()
        });
        sequence(
            &vec![InlineRef(0); count],
            &mut nodes,
            &mut Vec::new(),
            &mut budget,
        )?;
        Ok(budget.usage())
    };
    let small = measure(128)?;
    let large = measure(256)?;
    // Doubling one adjacent run doubles its bytes and references. A small
    // fixed node cost is independent of run length; prefix recopying is not.
    assert!(large.work <= small.work * 2 + 16, "{small:?} -> {large:?}");
    assert!(
        large.allocation_units <= small.allocation_units * 2 + 16,
        "{small:?} -> {large:?}"
    );
    assert_eq!(large.nodes, small.nodes);
    Ok(())
}

#[test]
fn text_run_preserves_ordered_origin_fold_and_source_less_segment() -> Result<(), ShapeError> {
    let mut nodes = vec![text_node("a"), text_node("b"), text_node("c")];
    nodes[0].origin = Some(OriginId(0));
    nodes[2].origin = Some(OriginId(0));
    let mut origins = vec![Origin::Synthetic {
        reason: "existing".into(),
        anchor: None,
    }];
    let output = sequence(
        &[InlineRef(0), InlineRef(1), InlineRef(2)],
        &mut nodes,
        &mut origins,
        &mut b(),
    )?;
    assert_eq!(output.len(), 1);
    let merged = &nodes[output[0].0 as usize];
    assert!(matches!(
        &origins[1],
        Origin::Synthetic { anchor: None, .. }
    ));
    assert_eq!(
        origins[2],
        Origin::Composite(vec![OriginId(0), OriginId(1)])
    );
    assert_eq!(
        origins[3],
        Origin::Composite(vec![OriginId(2), OriginId(0)])
    );
    assert_eq!(merged.origin, Some(OriginId(3)));
    assert_eq!(merged.span, None);
    Ok(())
}

#[test]
fn text_run_resource_stops_never_return_a_partial_sequence() {
    use nepl3_core::budget::StopReason;
    for (resource, reason) in [
        (Resource::Work, StopReason::WorkLimit),
        (Resource::AllocationUnits, StopReason::AllocationLimit),
        (Resource::Nodes, StopReason::NodeLimit),
    ] {
        let mut limits = Limits {
            work: 100_000,
            allocation_units: 100_000,
            nodes: 10,
            depth: 100,
            ..Limits::default()
        };
        match resource {
            Resource::Work => limits.work = 0,
            Resource::AllocationUnits => limits.allocation_units = 0,
            Resource::Nodes => limits.nodes = 0,
            _ => unreachable!(),
        }
        let mut budget = Budget::new(limits);
        let result = sequence(
            &[InlineRef(0), InlineRef(0)],
            &mut vec![text_node("a")],
            &mut Vec::new(),
            &mut budget,
        );
        assert_eq!(result, Err(ShapeError::Stopped(reason)));
        assert_eq!(budget.poll(), Err(reason));
    }
    let mut budget = b();
    budget.stop(StopReason::Cancelled);
    assert_eq!(
        sequence(&[], &mut Vec::new(), &mut Vec::new(), &mut budget),
        Err(ShapeError::Stopped(StopReason::Cancelled))
    );
}

#[test]
fn every_text_run_budget_boundary_preserves_exact_stop() -> Result<(), ShapeError> {
    use nepl3_core::budget::StopReason;
    let run = |budget: &mut Budget| {
        let mut nodes = vec![text_node("α"), text_node("b"), text_node("語")];
        nodes[0].origin = Some(OriginId(0));
        nodes[2].origin = Some(OriginId(0));
        let original = nodes.clone();
        let mut origins = vec![Origin::Synthetic {
            reason: "existing".into(),
            anchor: None,
        }];
        let result = sequence(
            &[InlineRef(0), InlineRef(1), InlineRef(2)],
            &mut nodes,
            &mut origins,
            budget,
        );
        assert_eq!(
            &nodes[..3],
            &original,
            "shared inputs stay unchanged even on failure"
        );
        result
    };
    let mut full = b();
    assert!(run(&mut full).is_ok());
    let used = full.usage();
    for (resource, amount, reason) in [
        (Resource::Work, used.work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            used.allocation_units,
            StopReason::AllocationLimit,
        ),
        (Resource::Nodes, used.nodes, StopReason::NodeLimit),
    ] {
        // This covers flattening, run scanning/allocation, synthetic/composite
        // origins and final output insertion, not only failure at entry.
        for limit in 0..=amount {
            let mut limits = Limits {
                work: 1_000_000,
                allocation_units: 1_000_000,
                nodes: 1000,
                depth: 100,
                ..Limits::default()
            };
            match resource {
                Resource::Work => limits.work = limit,
                Resource::AllocationUnits => limits.allocation_units = limit,
                Resource::Nodes => limits.nodes = limit,
                _ => unreachable!(),
            }
            let mut budget = Budget::new(limits);
            let result = run(&mut budget);
            if limit < amount {
                assert_eq!(
                    result,
                    Err(ShapeError::Stopped(reason)),
                    "{resource:?} {limit}/{amount}"
                );
                assert_eq!(budget.poll(), Err(reason));
            } else {
                assert!(result.is_ok(), "{resource:?}: {result:?}");
            }
        }
    }
    Ok(())
}
