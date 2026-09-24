use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_doc_core::{
    check::{Category, ShapeError},
    model::*,
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 10_000_000,
        depth: 200_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn value(root: DocRoot, kinds: Vec<DocKind>) -> DocValue {
    // Shape validation checks the slot role, not the independent guest schema.
    // Full content admission is covered by foreign/portable integration tests.
    let embeds = if kinds
        .iter()
        .any(|kind| matches!(kind, DocKind::Sentence { .. }))
    {
        vec![DocEmbed {
            kind: EmbedKind::Sentence,
            content: DocContent::Value {
                value: nepl3_core::value::TypedValue::Record(nepl3_core::value::Record {
                    schema: nepl3_core::value::SchemaRef {
                        package: "shape-fixture".into(),
                        revision: 1,
                        digest: nepl3_core::source::Digest::of(b"shape fixture"),
                    },
                    kind: "Opaque".into(),
                    fields: vec![],
                }),
            },
        }]
    } else {
        vec![]
    };
    DocValue {
        root,
        nodes: kinds
            .into_iter()
            .map(|kind| DocNode {
                locations: Vec::new(),
                kind,
                origin: None,
                span: None,
            })
            .collect(),
        embeds,
    }
}

#[test]
fn categories_and_graph_are_checked_before_semantic_use() -> Result<(), ShapeError> {
    let good = value(
        DocRoot::Variant(VariantRef(1)),
        vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Variant {
                language: "en".into(),
                sentence: SentenceRef(0),
            },
        ],
    );
    assert_eq!(good.validate_shape(&mut budget())?.postorder(), &[0, 1]);
    let mut wrong = good.clone();
    wrong.nodes[0].kind = DocKind::Paragraph { items: vec![] };
    assert!(matches!(
        wrong.validate_shape(&mut budget()),
        Err(ShapeError::Category {
            node: 0,
            expected: Category::Sentence
        })
    ));
    let cycle = value(
        DocRoot::Block(BlockRef(0)),
        vec![DocKind::Paragraph {
            items: vec![FlowRef(0)],
        }],
    );
    assert!(matches!(
        cycle.validate_shape(&mut budget()),
        Err(ShapeError::Cycle(0))
    ));
    let unused = value(
        DocRoot::Sentence(SentenceRef(0)),
        vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::RawCode {
                language_hint: None,
                text: "x".into(),
            },
        ],
    );
    assert!(matches!(
        unused.validate_shape(&mut budget()),
        Err(ShapeError::Unreachable(1))
    ));
    Ok(())
}
#[test]
fn parallel_constraints_use_actual_variant_languages() -> Result<(), ShapeError> {
    let mut parallel = value(
        DocRoot::Flow(FlowRef(3)),
        vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Variant {
                language: "en".into(),
                sentence: SentenceRef(0),
            },
            DocKind::Variant {
                language: "ja".into(),
                sentence: SentenceRef(0),
            },
            DocKind::Parallel {
                variants: vec![VariantRef(1), VariantRef(2)],
            },
        ],
    );
    parallel.validate_shape(&mut budget())?;
    parallel.nodes[2].kind = DocKind::Variant {
        language: "EN".into(),
        sentence: SentenceRef(0),
    };
    assert!(matches!(
        parallel.validate_shape(&mut budget()),
        Err(ShapeError::DuplicateLanguage {
            first: 1,
            second: 2,
            ..
        })
    ));
    Ok(())
}
#[test]
fn deep_arenas_stop_and_drop_without_recursive_ownership() -> Result<(), ShapeError> {
    let count = 100_000usize;
    let mut nodes = Vec::with_capacity(count);
    nodes.push(DocKind::RawCode {
        language_hint: None,
        text: "x".into(),
    });
    for i in 1..count {
        nodes.push(DocKind::Paragraph {
            items: vec![FlowRef((i - 1) as u64)],
        });
    }
    let deep = value(DocRoot::Block(BlockRef((count - 1) as u64)), nodes);
    let mut shallow = budget();
    let mut limits = shallow.limits();
    limits.depth = 64;
    shallow = Budget::new(limits);
    assert!(matches!(
        deep.validate_shape(&mut shallow),
        Err(ShapeError::Stopped(StopReason::DepthLimit))
    ));
    assert_eq!(deep.validate_shape(&mut budget())?.postorder().len(), count);
    let clone = deep.clone();
    drop(clone);
    drop(deep);
    Ok(())
}
#[test]
fn shared_child_still_counts_on_the_longest_path() -> Result<(), ShapeError> {
    let dag = value(
        DocRoot::Block(BlockRef(2)),
        vec![
            DocKind::RawCode {
                language_hint: None,
                text: "x".into(),
            },
            DocKind::Paragraph {
                items: vec![FlowRef(0)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(0), FlowRef(1)],
            },
        ],
    );
    let mut b = budget();
    let mut limits = b.limits();
    limits.depth = 2;
    b = Budget::new(limits);
    assert!(matches!(
        dag.validate_shape(&mut b),
        Err(ShapeError::Stopped(StopReason::DepthLimit))
    ));
    let mut b = budget();
    dag.validate_shape(&mut b)?;
    assert_eq!(b.usage().depth, 3);
    assert_eq!(b.usage().nodes, 3);
    Ok(())
}

#[test]
fn foreign_occurrences_preserve_sharing_order_roles_and_longest_depth() -> Result<(), ShapeError> {
    use nepl3_doc_core::check::ForeignOccurrence;
    let doc = value(
        DocRoot::Block(BlockRef(3)),
        vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Paragraph {
                items: vec![FlowRef(1), FlowRef(0)],
            },
            DocKind::Paragraph {
                items: vec![FlowRef(0), FlowRef(2), FlowRef(1)],
            },
        ],
    );
    let original = doc.clone();
    let checked = doc.validate_shape(&mut budget())?;
    let expected: Vec<_> = [(0, 2), (1, 3), (0, 3), (1, 2)]
        .into_iter()
        .map(|(node, depth)| ForeignOccurrence {
            node,
            embed: EmbedRef(0),
            kind: EmbedKind::Sentence,
            depth,
        })
        .collect();
    assert_eq!(checked.foreign_occurrences(&mut budget())?, expected);
    assert_eq!(checked.foreign_depths(&mut budget())?, [3]);
    for discovery in [false, true] {
        let mut full = budget();
        if discovery {
            checked.foreign_occurrences(&mut full)?;
        } else {
            checked.foreign_depths(&mut full)?;
        }
        for resource in [
            nepl3_core::budget::Resource::Work,
            nepl3_core::budget::Resource::AllocationUnits,
        ] {
            for shortage in [0, 1] {
                let mut limits = budget().limits();
                let reason = match resource {
                    nepl3_core::budget::Resource::Work => {
                        limits.work = full.usage().work - shortage;
                        StopReason::WorkLimit
                    }
                    _ => {
                        limits.allocation_units = full.usage().allocation_units - shortage;
                        StopReason::AllocationLimit
                    }
                };
                let mut limited = Budget::new(limits);
                let result = if discovery {
                    checked.foreign_occurrences(&mut limited).map(|_| ())
                } else {
                    checked.foreign_depths(&mut limited).map(|_| ())
                };
                if shortage == 0 {
                    result?;
                } else {
                    assert_eq!(result, Err(ShapeError::Stopped(reason)));
                    assert_eq!(limited.poll(), Err(reason));
                }
            }
        }
        let mut cancelled = budget();
        cancelled.cancel();
        let result = if discovery {
            checked.foreign_occurrences(&mut cancelled).map(|_| ())
        } else {
            checked.foreign_depths(&mut cancelled).map(|_| ())
        };
        assert_eq!(result, Err(ShapeError::Stopped(StopReason::Cancelled)));
        let mut limits = budget().limits();
        limits.depth = 7;
        let mut nested = Budget::new(limits);
        let result = nested.with_depth_at_least(5, |b| {
            if discovery {
                checked.foreign_occurrences(b).map(|_| ())
            } else {
                checked.foreign_depths(b).map(|_| ())
            }
        });
        assert_eq!(result, Err(ShapeError::Stopped(StopReason::DepthLimit)));
        assert_eq!(nested.current_depth(), 0);
    }
    assert_eq!(doc, original);
    let parallel = value(
        DocRoot::Flow(FlowRef(4)),
        vec![
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
            DocKind::Variant {
                language: "ja".into(),
                sentence: SentenceRef(0),
            },
            DocKind::Variant {
                language: "en".into(),
                sentence: SentenceRef(1),
            },
            DocKind::Parallel {
                variants: vec![VariantRef(3), VariantRef(2)],
            },
        ],
    );
    assert_eq!(
        parallel
            .validate_shape(&mut budget())?
            .foreign_occurrences(&mut budget())?
            .iter()
            .map(|o| (o.node, o.depth))
            .collect::<Vec<_>>(),
        [(1, 3), (0, 3)]
    );
    let mut figure = value(
        DocRoot::Block(BlockRef(0)),
        vec![
            DocKind::CircuitFigure {
                syntax: EmbedRef(1),
                caption: SentenceRef(1),
            },
            DocKind::Sentence {
                syntax: EmbedRef(0),
            },
        ],
    );
    let mut guest = figure.embeds[0].clone();
    guest.kind = EmbedKind::CircuitFigure;
    figure.embeds.push(guest);
    let checked = figure.validate_shape(&mut budget())?;
    assert_eq!(
        checked
            .foreign_occurrences(&mut budget())?
            .iter()
            .map(|o| (o.embed, o.kind, o.depth))
            .collect::<Vec<_>>(),
        [
            (EmbedRef(1), EmbedKind::CircuitFigure, 1),
            (EmbedRef(0), EmbedKind::Sentence, 2)
        ]
    );
    assert_eq!(checked.foreign_depths(&mut budget())?, [2, 1]);
    Ok(())
}

#[test]
fn foreign_depths_visit_shared_dags_once_while_expansion_is_budgeted() -> Result<(), ShapeError> {
    let mut previous = None;
    for count in [128, 256, 512] {
        let mut kinds = vec![DocKind::Sentence {
            syntax: EmbedRef(0),
        }];
        for i in 1..count {
            kinds.push(DocKind::Paragraph {
                items: vec![FlowRef(i - 1), FlowRef(i - 1)],
            });
        }
        let doc = value(DocRoot::Block(BlockRef(count - 1)), kinds);
        let checked = doc.validate_shape(&mut budget())?;
        let mut measured = budget();
        assert_eq!(checked.foreign_depths(&mut measured)?, [count]);
        if let Some(work) = previous {
            assert!(measured.usage().work < work * 3);
        }
        previous = Some(measured.usage().work);
        let mut limits = budget().limits();
        limits.work = 10_000;
        let mut limited = Budget::new(limits);
        assert_eq!(
            checked.foreign_occurrences(&mut limited),
            Err(ShapeError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
    }
    Ok(())
}
