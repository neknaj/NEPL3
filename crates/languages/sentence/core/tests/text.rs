use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::SourceAdmission,
};
use nepl3_sentence_core::{
    model::*,
    text::{self, AnnotationPolicy::*, Error},
};

fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        depth: 100_000,
        nodes: 1_000_000,
        output_bytes: 1_000_000,
        source_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn sample() -> SentenceValue {
    SentenceValue {
        root: Root::Sentence(SentenceRef(10)),
        embeds: vec![],
        nodes: vec![
            Kind::Text {
                text: "漢😀".into(),
            },
            Kind::Text {
                text: "かん".into(),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            Kind::Code {
                text: "x<>&".into(),
            },
            Kind::InlineAnno {
                base: InlineRef(2),
                notes: vec![InlineRef(3), InlineRef(1)],
            },
            Kind::Emphasis {
                inline: InlineRef(4),
            },
            Kind::Strong {
                inline: InlineRef(5),
            },
            Kind::ExternalLink {
                uri: "https://example.org/".into(),
                label: InlineRef(6),
            },
            Kind::Break,
            Kind::Concat {
                inlines: vec![InlineRef(7), InlineRef(8), InlineRef(0)],
            },
            Kind::Sentence {
                inlines: vec![InlineRef(9), InlineRef(3)],
            },
        ],
    }
}

#[test]
fn policies_preserve_order_labels_and_shared_occurrences() -> Result<(), Error> {
    let value = sample();
    let before = value.clone();
    let registry = SchemaRegistry::default();
    let proof = text::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())?;
    // Expectations describe the content directly, independently of a printer.
    for (policy, expected) in [
        (BaseOnly, "漢😀\n漢😀x<>&"),
        (WithReadings, "漢😀[かん]\n漢😀x<>&"),
        (WithAllNotes, "漢😀[かん]{x<>&/かん}\n漢😀x<>&"),
    ] {
        assert_eq!(proof.render(policy, &[], &mut b())?, expected);
    }
    assert_eq!(value, before);
    Ok(())
}

#[test]
fn resource_boundaries_return_no_output_and_allow_exact_usage() -> Result<(), Error> {
    let value = sample();
    let registry = SchemaRegistry::default();
    let proof = text::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())?;
    let mut measured = b();
    let expected = proof.render(WithAllNotes, &[], &mut measured)?;
    let usage = measured.usage();
    for (resource, amount, reason) in [
        (0, usage.work, StopReason::WorkLimit),
        (1, usage.allocation_units, StopReason::AllocationLimit),
        (2, usage.output_bytes, StopReason::OutputLimit),
        (3, usage.depth, StopReason::DepthLimit),
    ] {
        for limit in [0, amount - 1, amount] {
            let mut limits = b().limits();
            match resource {
                0 => limits.work = limit,
                1 => limits.allocation_units = limit,
                2 => limits.output_bytes = limit,
                _ => limits.depth = limit,
            }
            let mut budget = Budget::new(limits);
            let result = proof.render(WithAllNotes, &[], &mut budget);
            if limit == amount {
                assert_eq!(result?, expected);
            } else {
                assert_eq!(result, Err(Error::Stopped(reason)));
            }
            assert_eq!(budget.current_depth(), 0);
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        proof.render(BaseOnly, &[], &mut cancelled),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    assert!(matches!(
        text::prepare(
            &value,
            &registry,
            &mut cancelled,
            &mut SourceAdmission::default()
        ),
        Err(Error::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}

#[test]
fn deep_inline_empty_content_and_shared_expansion() -> Result<(), Error> {
    let registry = SchemaRegistry::default();
    let mut value = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        embeds: vec![],
        nodes: vec![Kind::Text {
            text: String::new(),
        }],
    };
    for i in 0..10_000 {
        value.nodes.push(Kind::Emphasis {
            inline: InlineRef(i),
        });
    }
    value.root = Root::Inline(InlineRef(10_000));
    assert_eq!(
        text::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())?.render(
            BaseOnly,
            &[],
            &mut b()
        )?,
        ""
    );
    value.nodes = vec![Kind::Text { text: "x".into() }];
    for i in 0..20 {
        value.nodes.push(Kind::Concat {
            inlines: vec![InlineRef(i), InlineRef(i)],
        });
    }
    value.root = Root::Inline(InlineRef(20));
    let proof = text::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())?;
    let mut limits = b().limits();
    limits.output_bytes = 1000;
    assert_eq!(
        proof.render(BaseOnly, &[], &mut Budget::new(limits)),
        Err(Error::Stopped(StopReason::OutputLimit))
    );
    value.nodes[0] = Kind::Emphasis {
        inline: InlineRef(20),
    };
    assert!(matches!(
        text::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default()),
        Err(Error::Shape(_))
    ));
    Ok(())
}
