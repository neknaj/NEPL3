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
    DocValue {
        root,
        nodes: kinds
            .into_iter()
            .map(|kind| DocNode {
                kind,
                origin: None,
                span: None,
            })
            .collect(),
        embeds: vec![],
    }
}

#[test]
fn categories_and_graph_are_checked_before_semantic_use() -> Result<(), ShapeError> {
    let good = value(
        DocRoot::Variant(VariantRef(1)),
        vec![
            DocKind::Sentence { inlines: vec![] },
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
        DocRoot::Inline(InlineRef(0)),
        vec![DocKind::Concat {
            inlines: vec![InlineRef(0)],
        }],
    );
    assert!(matches!(
        cycle.validate_shape(&mut budget()),
        Err(ShapeError::Cycle(0))
    ));
    let unused = value(
        DocRoot::Sentence(SentenceRef(0)),
        vec![DocKind::Sentence { inlines: vec![] }, DocKind::Break],
    );
    assert!(matches!(
        unused.validate_shape(&mut budget()),
        Err(ShapeError::Unreachable(1))
    ));
    Ok(())
}
#[test]
fn parallel_and_annotation_constraints_use_actual_child_content() -> Result<(), ShapeError> {
    let mut parallel = value(
        DocRoot::Flow(FlowRef(3)),
        vec![
            DocKind::Sentence { inlines: vec![] },
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
    let empty = value(
        DocRoot::Inline(InlineRef(3)),
        vec![
            DocKind::Text {
                text: String::new(),
            },
            DocKind::Concat {
                inlines: vec![InlineRef(0)],
            },
            DocKind::Text {
                text: "reading".into(),
            },
            DocKind::Ruby {
                base: InlineRef(1),
                reading: InlineRef(2),
            },
        ],
    );
    assert!(matches!(
        empty.validate_shape(&mut budget()),
        Err(ShapeError::EmptyAnnotationPart(1))
    ));
    let break_part = value(
        DocRoot::Inline(InlineRef(1)),
        vec![
            DocKind::Break,
            DocKind::Anno {
                base: InlineRef(0),
                notes: vec![InlineRef(0)],
            },
        ],
    );
    break_part.validate_shape(&mut budget())?;
    Ok(())
}
#[test]
fn deep_arenas_stop_and_drop_without_recursive_ownership() -> Result<(), ShapeError> {
    let count = 100_000usize;
    let mut nodes = Vec::with_capacity(count);
    nodes.push(DocKind::Text { text: "x".into() });
    for i in 1..count {
        nodes.push(DocKind::Strong {
            inline: InlineRef((i - 1) as u64),
        });
    }
    let deep = value(DocRoot::Inline(InlineRef((count - 1) as u64)), nodes);
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
        DocRoot::Inline(InlineRef(2)),
        vec![
            DocKind::Text { text: "x".into() },
            DocKind::Concat {
                inlines: vec![InlineRef(0)],
            },
            DocKind::Concat {
                inlines: vec![InlineRef(0), InlineRef(1)],
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
