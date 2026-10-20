use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_doc_core::{check::OccurrenceParent, model::*};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 10_000_000,
        nodes: 1_000_000,
        depth: 1000,
        ..Limits::default()
    })
}
fn value(root: DocRoot, kinds: Vec<DocKind>) -> DocValue {
    DocValue {
        root,
        nodes: kinds
            .into_iter()
            .map(|kind| DocNode {
                kind,
                locations: vec![],
                origin: None,
                span: None,
            })
            .collect(),
        embeds: Default::default(),
    }
}
fn sample() -> DocValue {
    value(
        DocRoot::Sentence(SentenceRef(3)),
        vec![
            DocKind::Text { text: "a".into() },
            DocKind::Text { text: "b".into() },
            DocKind::Concat {
                inlines: vec![InlineRef(0), InlineRef(0)],
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(2), InlineRef(1)],
            },
        ],
    )
}
#[test]
fn constructor_order_keeps_shared_occurrences_and_parent_edges() -> Result<(), String> {
    let input = sample();
    let checked = input
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let output = checked
        .structural_occurrences(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        output.iter().map(|v| v.node).collect::<Vec<_>>(),
        vec![3, 2, 0, 0, 1]
    );
    assert_eq!(
        output.iter().map(|v| v.depth).collect::<Vec<_>>(),
        vec![1, 2, 3, 3, 2]
    );
    assert_eq!(
        output.iter().map(|v| v.parent).collect::<Vec<_>>(),
        vec![
            None,
            Some(OccurrenceParent {
                occurrence: 0,
                child: 0
            }),
            Some(OccurrenceParent {
                occurrence: 1,
                child: 0
            }),
            Some(OccurrenceParent {
                occurrence: 1,
                child: 1
            }),
            Some(OccurrenceParent {
                occurrence: 0,
                child: 1
            }),
        ]
    );
    Ok(())
}
#[test]
fn prior_shape_proof_cannot_skip_new_depth_or_cancellation() -> Result<(), String> {
    let input = sample();
    let checked = input
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut limits = budget().limits();
    limits.depth = 4;
    let mut b = Budget::new(limits);
    let result = b.with_depth_at_least(2, |b| checked.structural_occurrences(b));
    assert_eq!(result, Err(StopReason::DepthLimit));
    assert_eq!(
        checked.structural_occurrences(&mut b),
        Err(StopReason::DepthLimit)
    );
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        checked.structural_occurrences(&mut cancelled),
        Err(StopReason::Cancelled)
    );
    let leaf = value(
        DocRoot::Sentence(SentenceRef(0)),
        vec![DocKind::Sentence { inlines: vec![] }],
    );
    let proof = leaf
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    limits.depth = u64::MAX;
    let mut overflow = Budget::new(limits);
    assert_eq!(
        overflow.with_depth_at_least(u64::MAX, |b| proof.structural_occurrences(b)),
        Err(StopReason::DepthLimit)
    );
    Ok(())
}
#[test]
fn expanded_and_wide_dags_stop_without_a_partial_result() -> Result<(), String> {
    let mut kinds = vec![DocKind::Text {
        text: "leaf".into(),
    }];
    for node in 1..24 {
        kinds.push(DocKind::Concat {
            inlines: vec![InlineRef(node - 1), InlineRef(node - 1)],
        });
    }
    let expanded = value(DocRoot::Inline(InlineRef(23)), kinds);
    let proof = expanded
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut limits = budget().limits();
    limits.work = 200;
    let mut limited = Budget::new(limits);
    assert_eq!(
        proof.structural_occurrences(&mut limited),
        Err(StopReason::WorkLimit)
    );
    let wide = value(
        DocRoot::Sentence(SentenceRef(1)),
        vec![
            DocKind::Text {
                text: "leaf".into(),
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(0); 1000],
            },
        ],
    );
    let proof = wide
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    limits.work = 1_000_000;
    limits.allocation_units = 128;
    assert_eq!(
        proof.structural_occurrences(&mut Budget::new(limits)),
        Err(StopReason::AllocationLimit)
    );
    assert_eq!(
        proof
            .structural_occurrences(&mut budget())
            .map_err(|e| format!("{e:?}"))?
            .len(),
        1001
    );
    Ok(())
}

#[test]
fn traversal_is_structural_not_visibility_or_alt_projection() -> Result<(), String> {
    let parallel = value(
        DocRoot::Flow(FlowRef(0)),
        vec![
            DocKind::Parallel {
                variants: vec![VariantRef(1), VariantRef(2)],
            },
            DocKind::Variant {
                language: "en".into(),
                sentence: SentenceRef(3),
            },
            DocKind::Variant {
                language: "ja".into(),
                sentence: SentenceRef(4),
            },
            DocKind::Sentence { inlines: vec![] },
            DocKind::Sentence { inlines: vec![] },
        ],
    );
    let proof = parallel
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        proof
            .structural_occurrences(&mut budget())
            .map_err(|e| format!("{e:?}"))?
            .iter()
            .map(|v| v.node)
            .collect::<Vec<_>>(),
        vec![0, 1, 3, 2, 4]
    );
    let image = value(
        DocRoot::Block(BlockRef(2)),
        vec![
            DocKind::Sentence { inlines: vec![] },
            DocKind::Sentence { inlines: vec![] },
            DocKind::Image {
                asset: AssetRef {
                    id: "image".into(),
                    digest: None,
                },
                alt: SentenceRef(0),
                caption: Some(SentenceRef(1)),
            },
        ],
    );
    let proof = image
        .validate_shape(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let output = proof
        .structural_occurrences(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        output.iter().map(|v| v.node).collect::<Vec<_>>(),
        vec![2, 0, 1]
    );
    assert_eq!(
        output[1].parent,
        Some(OccurrenceParent {
            occurrence: 0,
            child: 0
        })
    );
    assert_eq!(
        output[2].parent,
        Some(OccurrenceParent {
            occurrence: 0,
            child: 1
        })
    );
    Ok(())
}
