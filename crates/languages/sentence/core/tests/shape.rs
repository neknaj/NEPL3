use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_sentence_core::{
    check::{Category, Error},
    model::*,
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 10_000_000,
        depth: 200_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn value(nodes: Vec<Kind>) -> SentenceValue {
    SentenceValue {
        root: Root::Sentence(SentenceRef(0)),
        nodes,
        embeds: vec![],
    }
}
fn checked(v: &SentenceValue, b: &mut Budget) -> Result<(), Error> {
    v.validate_shape(b).map(|_| ())
}

#[test]
fn empty_sentence_and_ordered_ruby_notes_have_distinct_content() -> Result<(), Error> {
    checked(
        &value(vec![Kind::Sentence { inlines: vec![] }]),
        &mut budget(),
    )?;
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::InlineAnno {
            base: InlineRef(2),
            notes: vec![InlineRef(5), InlineRef(6)],
        },
        Kind::Ruby {
            base: InlineRef(3),
            reading: InlineRef(4),
        },
        Kind::Text {
            text: "漢字".into(),
        },
        Kind::Text {
            text: "かんじ".into(),
        },
        Kind::Code {
            text: "first".into(),
        },
        Kind::Break,
    ]);
    let before = v.clone();
    let proof = v.validate_shape(&mut budget())?;
    // Ordered children precede owners; a note does not become the Ruby reading.
    assert_eq!(proof.postorder(), &[3, 4, 2, 5, 6, 1, 0]);
    assert_eq!(proof.value(), &before);
    Ok(())
}

#[test]
fn annotation_cannot_turn_empty_wrapped_text_into_content() {
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::Ruby {
            base: InlineRef(2),
            reading: InlineRef(4),
        },
        Kind::Strong {
            inline: InlineRef(3),
        },
        Kind::Text {
            text: String::new(),
        },
        Kind::Text {
            text: "reading".into(),
        },
    ]);
    assert_eq!(
        checked(&v, &mut budget()),
        Err(Error::EmptyAnnotationPart(2))
    );
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::InlineAnno {
            base: InlineRef(2),
            notes: vec![],
        },
        Kind::Break,
    ]);
    assert_eq!(checked(&v, &mut budget()), Err(Error::AnnotationNotes(1)));
}

#[test]
fn migrated_doc_annotation_cases_use_sentence_shape_contract() -> Result<(), Error> {
    let empty = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::Ruby {
            base: InlineRef(2),
            reading: InlineRef(4),
        },
        Kind::Concat {
            inlines: vec![InlineRef(3)],
        },
        Kind::Text {
            text: String::new(),
        },
        Kind::Text {
            text: "reading".into(),
        },
    ]);
    assert_eq!(
        checked(&empty, &mut budget()),
        Err(Error::EmptyAnnotationPart(2))
    );
    let break_part = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::InlineAnno {
            base: InlineRef(2),
            notes: vec![InlineRef(2)],
        },
        Kind::Break,
    ]);
    checked(&break_part, &mut budget())?;
    Ok(())
}

#[test]
fn references_categories_cycles_and_unreachable_nodes_are_rejected() {
    let cases = [
        (
            value(vec![Kind::Sentence {
                inlines: vec![InlineRef(u64::MAX)],
            }]),
            Error::Reference(u64::MAX),
        ),
        (
            value(vec![Kind::Sentence {
                inlines: vec![InlineRef(0)],
            }]),
            Error::Category {
                node: 0,
                expected: Category::Inline,
            },
        ),
        (
            value(vec![
                Kind::Sentence {
                    inlines: vec![InlineRef(1)],
                },
                Kind::Concat {
                    inlines: vec![InlineRef(1)],
                },
            ]),
            Error::Cycle(1),
        ),
        (
            value(vec![Kind::Sentence { inlines: vec![] }, Kind::Break]),
            Error::Unreachable(1),
        ),
        (
            value(vec![
                Kind::Sentence {
                    inlines: vec![InlineRef(1)],
                },
                Kind::ForeignInline {
                    syntax: EmbedRef(0),
                },
            ]),
            Error::Embed(0),
        ),
    ];
    for (v, error) in cases {
        let before = v.clone();
        assert_eq!(checked(&v, &mut budget()), Err(error));
        assert_eq!(v, before);
    }
}

#[test]
fn shared_subtrees_use_the_longest_path_and_preserve_caller_depth() -> Result<(), Error> {
    // Visit the shared node on the short path first. The later path is depth 4.
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1), InlineRef(2)],
        },
        Kind::Emphasis {
            inline: InlineRef(3),
        },
        Kind::Strong {
            inline: InlineRef(1),
        },
        Kind::Break,
    ]);
    let mut limits = budget().limits();
    limits.depth = 3;
    let mut b = Budget::new(limits);
    assert_eq!(
        checked(&v, &mut b),
        Err(Error::Stopped(StopReason::DepthLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::DepthLimit));
    let mut b = budget();
    b.with_depth_at_least::<_, Error>(7, |b| {
        checked(&v, b)?;
        assert_eq!(b.current_depth(), 7);
        Ok(())
    })?;
    assert_eq!(b.usage().depth, 11);
    assert_eq!(b.current_depth(), 0);
    Ok(())
}

#[test]
fn links_are_not_resolved_by_shape_check() -> Result<(), Error> {
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::ExternalLink {
            uri: "javascript:must-be-rejected-by-render-preparation".into(),
            label: InlineRef(2),
        },
        Kind::Text {
            text: "label".into(),
        },
    ]);
    // A shape proof retains source content; it must not be an HTML safety proof.
    checked(&v, &mut budget())
}

fn invalid_guest() -> nepl3_core::syntax::ForeignClosure {
    use nepl3_core::{source::Digest, syntax::*, value::SchemaRef};
    // Intentionally invalid guest root. The shape checker only proves arena
    // references; the foreign boundary must independently validate this closure.
    ForeignClosure {
        syntax: ForeignSyntax {
            schema: SchemaRef {
                package: "test.invalid-guest".into(),
                revision: 1,
                digest: Digest([0; 32]),
            },
            category: "Inline".into(),
            root: NodeRef(99),
            bundle: SyntaxBundle {
                sources: vec![],
                nodes: vec![],
                origins: vec![],
                root: NodeRef(99),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef {
                id: 0,
                digest: Digest([0; 32]),
            },
        },
        owner_environment: EnvironmentEntry {
            id: 0,
            digest: Digest([0; 32]),
            value: Environment {
                bindings: vec![],
                resources: vec![],
            },
        },
        provenance: nepl3_core::syntax::OwnerProvenance::from_parts(vec![], vec![], vec![]),
    }
}

#[test]
fn foreign_indices_are_checked_without_claiming_closure_validity() -> Result<(), Error> {
    let closure = invalid_guest();
    let mut v = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::ForeignInline {
            syntax: EmbedRef(0),
        }],
        embeds: vec![closure.clone().into()],
    };
    checked(&v, &mut budget())?;
    // A real arena embed reference reaches the printer's adapter boundary.
    // Guest validity is not asserted: the standard surface cannot print it
    // and must reject before attempting to render, evaluate, or erase it.
    assert_eq!(
        nepl3_sentence_core::print::prefix(&v, &mut budget()),
        Err(nepl3_sentence_core::print::Error::AdapterRequired(
            EmbedRef(0)
        ))
    );
    v.root = Root::Sentence(SentenceRef(0));
    assert_eq!(
        checked(&v, &mut budget()),
        Err(Error::Category {
            node: 0,
            expected: Category::Sentence
        })
    );
    v.nodes = vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1), InlineRef(2)],
        },
        Kind::ForeignInline {
            syntax: EmbedRef(0),
        },
        Kind::ForeignInline {
            syntax: EmbedRef(0),
        },
    ];
    checked(&v, &mut budget())?;
    v.root = Root::Inline(InlineRef(0));
    assert_eq!(
        checked(&v, &mut budget()),
        Err(Error::Category {
            node: 0,
            expected: Category::Inline
        })
    );
    v.root = Root::Sentence(SentenceRef(0));
    v.embeds.push(closure.into());
    assert_eq!(checked(&v, &mut budget()), Err(Error::UnusedEmbed(1)));
    v.nodes[1] = Kind::ForeignInline {
        syntax: EmbedRef(2),
    };
    assert_eq!(checked(&v, &mut budget()), Err(Error::Embed(2)));
    Ok(())
}

#[test]
fn foreign_occurrences_keep_order_sharing_notes_and_budget_boundaries() -> Result<(), Error> {
    use nepl3_sentence_core::check::ForeignOccurrence;
    let v = SentenceValue {
        root: Root::Sentence(SentenceRef(4)),
        embeds: vec![invalid_guest().into(), invalid_guest().into()],
        nodes: vec![
            Kind::ForeignInline {
                syntax: EmbedRef(1),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(2),
            },
            Kind::ForeignInline {
                syntax: EmbedRef(0),
            },
            Kind::InlineAnno {
                base: InlineRef(1),
                notes: vec![InlineRef(0), InlineRef(2)],
            },
            Kind::Sentence {
                inlines: vec![InlineRef(2), InlineRef(3), InlineRef(1)],
            },
        ],
    };
    let shape = v.validate_shape(&mut budget())?;
    // Explicit child order differs from arena and embed order. Ruby readings,
    // annotation notes and repeated shared nodes are all namespace occurrences.
    let expected: Vec<_> = [
        (2, 0, 2),
        (0, 1, 4),
        (2, 0, 4),
        (0, 1, 3),
        (2, 0, 3),
        (0, 1, 3),
        (2, 0, 3),
    ]
    .map(|(node, embed, depth)| ForeignOccurrence {
        node: InlineRef(node),
        embed: EmbedRef(embed),
        depth,
    })
    .into();
    let mut measured = budget();
    assert_eq!(shape.foreign_occurrences(&mut measured)?, expected);
    assert_eq!(shape.foreign_depths(&mut budget())?, vec![4, 4]);
    let used = measured.usage();
    for (reason, amount) in [
        (StopReason::WorkLimit, used.work),
        (StopReason::AllocationLimit, used.allocation_units),
        (StopReason::DepthLimit, used.depth),
    ] {
        for limit in [0, amount - 1, amount] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("fixed resources"),
            }
            let mut b = Budget::new(limits);
            let result = shape.foreign_occurrences(&mut b);
            if limit == amount {
                assert_eq!(result?, expected);
            } else {
                assert_eq!(result, Err(Error::Stopped(reason)));
                assert_eq!(b.poll(), Err(reason));
            }
            assert_eq!(b.current_depth(), 0);
        }
    }
    let mut b = budget();
    b.with_depth_at_least::<_, Error>(7, |b| {
        assert_eq!(shape.foreign_occurrences(b)?, expected);
        assert_eq!(b.current_depth(), 7);
        Ok(())
    })?;
    assert_eq!(b.usage().depth, 11);
    b.cancel();
    assert_eq!(
        shape.foreign_occurrences(&mut b),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    // Exponential display expansion of a finite DAG is still bounded by Work.
    let mut dag = SentenceValue {
        root: Root::Inline(InlineRef(40)),
        embeds: vec![invalid_guest().into()],
        nodes: vec![Kind::ForeignInline {
            syntax: EmbedRef(0),
        }],
    };
    for parent in 1..=40 {
        dag.nodes.push(Kind::Concat {
            inlines: vec![InlineRef(parent - 1); 2],
        });
    }
    let shape = dag.validate_shape(&mut budget())?;
    let mut limits = budget().limits();
    limits.work = 1000;
    assert_eq!(
        shape.foreign_occurrences(&mut Budget::new(limits)),
        Err(Error::Stopped(StopReason::WorkLimit))
    );
    Ok(())
}

#[test]
fn stopped_budgets_return_no_proof_and_do_not_modify_input() {
    let v = value(vec![
        Kind::Sentence {
            inlines: vec![InlineRef(1)],
        },
        Kind::Break,
    ]);
    let before = v.clone();
    for resource in 0..5 {
        let mut limits = budget().limits();
        match resource {
            0 => limits.work = 0,
            1 => limits.nodes = 0,
            2 => limits.allocation_units = 0,
            3 => limits.depth = 0,
            _ => {}
        }
        let mut b = Budget::new(limits);
        if resource == 4 {
            b.stop(StopReason::Cancelled);
        }
        assert!(matches!(checked(&v, &mut b), Err(Error::Stopped(_))));
        assert!(b.poll().is_err());
        assert_eq!(v, before);
    }
}

#[test]
fn deeply_nested_sentence_is_iterative_to_check_clone_and_drop() -> Result<(), Error> {
    let count = 100_000;
    let mut nodes = Vec::with_capacity(count + 1);
    nodes.push(Kind::Sentence {
        inlines: vec![InlineRef(1)],
    });
    for i in 1..count {
        nodes.push(Kind::Emphasis {
            inline: InlineRef(i as u64 + 1),
        });
    }
    nodes.push(Kind::Text {
        text: "leaf".into(),
    });
    let v = value(nodes);
    checked(&v, &mut budget())?;
    assert_eq!(v.clone(), v);
    Ok(())
}
