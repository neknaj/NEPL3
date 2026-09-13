use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_sentence_core::{
    model::*,
    print::{self, Error},
};
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 2_000_000,
        work: 100_000_000,
        depth: 100_000,
        nodes: 2_000_000,
        allocation_units: 200_000_000,
        output_bytes: 2_000_000,
        diagnostics: 100,
        events: 100,
    })
}
#[test]
fn prefix_retains_standard_constructors_and_order() -> Result<(), Error> {
    let value = SentenceValue {
        root: Root::Sentence(SentenceRef(10)),
        embeds: vec![],
        nodes: vec![
            Kind::Text {
                text: "漢\n[字]{語}/\"\\\0😀".into(),
            },
            Kind::Text {
                text: "reading".into(),
            },
            Kind::Ruby {
                base: InlineRef(0),
                reading: InlineRef(1),
            },
            Kind::InlineAnno {
                base: InlineRef(2),
                notes: vec![InlineRef(1), InlineRef(0)],
            },
            Kind::Code { text: "x".into() },
            Kind::Emphasis {
                inline: InlineRef(4),
            },
            Kind::Strong {
                inline: InlineRef(5),
            },
            Kind::Break,
            Kind::ExternalLink {
                uri: "https://example.invalid/".into(),
                label: InlineRef(6),
            },
            Kind::Concat {
                inlines: vec![InlineRef(7), InlineRef(8)],
            },
            Kind::Sentence {
                inlines: vec![InlineRef(3), InlineRef(9)],
            },
        ],
    };
    // Independent surface expectation: lists keep order, Concat/Break stay
    // constructors, and BuiltinText does not interpret sentence Ruby delimiters.
    let text = r#"text "漢\n[字]{語}/\"\\\u{0}😀""#;
    assert_eq!(
        print::prefix(&value, &mut b())?,
        format!(
            "sentence cons anno ruby {text} text \"reading\" cons text \"reading\" cons {text} nil cons concat cons break cons link \"https://example.invalid/\" strong em code \"x\" nil nil"
        )
    );
    Ok(())
}
#[test]
fn empty_values_inline_entry_and_deep_nonrecursive_output() -> Result<(), Error> {
    let empty = SentenceValue {
        root: Root::Sentence(SentenceRef(0)),
        nodes: vec![Kind::Sentence { inlines: vec![] }],
        embeds: vec![],
    };
    assert_eq!(print::prefix(&empty, &mut b())?, "sentence nil");
    let mut v = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::Text {
            text: String::new(),
        }],
        embeds: vec![],
    };
    for i in 0..10_000 {
        v.nodes.push(Kind::Emphasis {
            inline: InlineRef(i),
        });
    }
    v.root = Root::Inline(InlineRef(10_000));
    assert_eq!(
        print::prefix(&v, &mut b())?,
        format!("{}text \"\"", "em ".repeat(10_000))
    );
    Ok(())
}
#[test]
fn shared_expansion_is_bounded_and_invalid_graphs_never_print() {
    let mut v = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::Text { text: "x".into() }],
        embeds: vec![],
    };
    // 21 arena nodes describe over a million occurrences: charge emitted data,
    // not only the compact input. Stop before returning any partial string.
    for i in 0..20 {
        v.nodes.push(Kind::Concat {
            inlines: vec![InlineRef(i), InlineRef(i)],
        });
    }
    v.root = Root::Inline(InlineRef(20));
    let mut limits = b().limits();
    limits.output_bytes = 1000;
    assert_eq!(
        print::prefix(&v, &mut Budget::new(limits)),
        Err(Error::Stopped(StopReason::OutputLimit))
    );
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        print::prefix(&v, &mut cancelled),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    v.nodes[0] = Kind::Emphasis {
        inline: InlineRef(20),
    };
    assert!(matches!(print::prefix(&v, &mut b()), Err(Error::Shape(_))));
}
