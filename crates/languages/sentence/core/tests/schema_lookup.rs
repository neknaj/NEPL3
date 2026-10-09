use nepl3_core::{budget::*, schema::*, source::*, value::*};
use nepl3_sentence_core::{
    literal::{self, SentenceError, SentenceOutcome},
    model::*,
    portable,
    syntax::SentenceSyntax,
};
use nepl3_wire::{WireError, foundation::FoundationCodec};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        allocation_units: 10_000_000,
        nodes: 100_000,
        depth: 1000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn registry(padding: usize) -> Result<SchemaRegistry, SchemaError> {
    let mut r = SchemaRegistry::default();
    let d = nepl3_core::schema::foundation::descriptor(&mut budget())?;
    r.register(d.reference(&mut budget())?, d, &mut budget())?;
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        r.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    r.finalize(&mut budget())?;
    Ok(r)
}
fn lookup_work(padding: u64) -> u64 {
    1 + ("nepl3.foundation".len() + "nepl3.sentence".len() + 9) as u64
        + padding * (10 + "nepl3.sentence".len() as u64 + 9)
}
fn source(text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("s".into()),
        0,
        "memory:s".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
fn portable_call(
    case: u8,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), portable::Error<WireError>> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(r, &store, &mut admission).map_err(portable::Error::Foundation)?;
    let input = SentenceSyntax {
        value: SentenceValue {
            root: Root::Sentence(SentenceRef(0)),
            nodes: vec![],
            embeds: vec![],
        },
        locations: vec![],
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    match case {
        0 => portable::to_value(&input.value, r, &mut codec, b).map(|_| ()),
        1 => portable::from_value(&NdfValue::Unit, r, &mut codec, b).map(|_| ()),
        2 => portable::syntax::to_value(&input, r, &mut codec, b).map(|_| ()),
        3 => portable::syntax::from_value(&NdfValue::Unit, r, &mut codec, b).map(|_| ()),
        4 => portable::literal::to_value(&input, r, &mut codec, b).map(|_| ()),
        _ => {
            let owner =
                source("owner").map_err(|e| portable::Error::Foundation(WireError::Source(e)))?;
            portable::literal::from_value(&NdfValue::Unit, &owner, r, &mut codec, b).map(|_| ())
        }
    }
}
#[test]
fn all_portable_schema_gates_meter_unsuccessful_catalog_search() -> Result<(), SchemaError> {
    for padding in [0, 32] {
        let r = registry(padding)?;
        for case in 0..6 {
            let mut b = budget();
            assert!(matches!(
                portable_call(case, &r, &mut b),
                Err(portable::Error::Schema(SchemaError::UnknownSchema))
            ));
            assert_eq!(
                b.usage(),
                Usage {
                    work: lookup_work(padding as u64),
                    ..Usage::default()
                }
            );
            let mut limits = budget().limits();
            limits.work = lookup_work(padding as u64) - 1;
            let mut stopped = Budget::new(limits);
            assert!(matches!(
                portable_call(case, &r, &mut stopped),
                Err(portable::Error::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
            assert_eq!(stopped.usage().source_bytes, 0);
            assert_eq!(stopped.usage().allocation_units, 0);
        }
    }
    Ok(())
}

#[test]
fn literal_selection_preserves_source_admission_and_recognition_priority() -> Result<(), String> {
    let err = |e| format!("{e:?}");
    let source = source("\"").map_err(|e| format!("{e:?}"))?;
    for padding in [0, 32] {
        let r = registry(padding).map_err(err)?;
        // Recognition is one Work after the independently measured source admission.
        let mut before = budget();
        SourceAdmission::default()
            .admit_existing(&source, &mut before)
            .map_err(|e| format!("{e:?}"))?;
        let mut b = budget();
        assert!(matches!(
            literal::read(
                &source,
                0,
                1,
                true,
                &r,
                &mut b,
                &mut SourceAdmission::default()
            ),
            Err(SentenceError::Schema(SchemaError::UnknownSchema))
        ));
        let mut expected = before.usage();
        expected.work += 1 + lookup_work(padding as u64);
        assert_eq!(b.usage(), expected);
        let mut limits = budget().limits();
        limits.work = expected.work - 1;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            literal::read(
                &source,
                0,
                1,
                true,
                &r,
                &mut stopped,
                &mut SourceAdmission::default()
            ),
            Err(SentenceError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        assert_eq!(stopped.usage().source_bytes, 1);
    }
    for (text, final_input) in [("x", true), ("", false)] {
        let source = self::source(text).map_err(|e| format!("{e:?}"))?;
        let mut usages = vec![];
        for r in [SchemaRegistry::default(), registry(32).map_err(err)?] {
            let mut b = budget();
            let scan = literal::read(
                &source,
                0,
                text.len() as u64,
                final_input,
                &r,
                &mut b,
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
            assert!(matches!(
                (scan.outcome, final_input),
                (SentenceOutcome::NoMatch, true) | (SentenceOutcome::NeedMore, false)
            ));
            usages.push(b.usage());
        }
        assert_eq!(usages[0], usages[1]);
    }
    Ok(())
}

#[test]
fn changed_sentence_identity_and_stops_keep_schema_gate_priority() -> Result<(), String> {
    let mut r = registry(32).map_err(|e| format!("{e:?}"))?;
    let mut descriptor =
        nepl3_sentence_core::schema::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    descriptor.types.push(NamedType {
        name: "Extra".into(),
        shape: TypeShape::Record { fields: vec![] },
        constraints: vec![],
    });
    r.register(
        descriptor
            .reference(&mut budget())
            .map_err(|e| format!("{e:?}"))?,
        descriptor,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    r.finalize(&mut budget()).map_err(|e| format!("{e:?}"))?;
    for case in 0..6 {
        assert!(matches!(
            portable_call(case, &r, &mut budget()),
            Err(portable::Error::SchemaIdentity)
        ));
        let mut limits = budget().limits();
        limits.work = 1;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            portable_call(case, &r, &mut stopped),
            Err(portable::Error::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            portable_call(case, &r, &mut cancelled),
            Err(portable::Error::Stopped(StopReason::Cancelled))
        ));
        assert_eq!(cancelled.usage(), Usage::default());
    }
    let source = source("\"").map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        literal::read(
            &source,
            0,
            1,
            true,
            &r,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SentenceError::SchemaIdentity)
    ));
    assert!(matches!(
        literal::read(
            &source,
            0,
            1,
            true,
            &SchemaRegistry::default(),
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(SentenceError::Schema(SchemaError::Unfinalized))
    ));
    Ok(())
}
