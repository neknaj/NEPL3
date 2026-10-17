use super::*;
use alloc::{format, string::String};
use nepl3_core::{
    budget::Limits,
    source::{SourceId, SourceStore},
};
use nepl3_sentence_core::literal::{self, SentenceOutcome};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        source_bytes: 1_000_000,
        depth: 1000,
        ..Limits::default()
    })
}
fn error(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn fixture() -> Result<(SchemaRegistry, SourceSnapshot, SentenceSyntax), String> {
    let mut r = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        nepl3_sentence_core::schema::descriptor(&mut budget()),
        nepl3_doc_core::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(error)?;
        r.register(
            descriptor.reference(&mut budget()).map_err(error)?,
            descriptor,
            &mut budget(),
        )
        .map_err(error)?;
    }
    r.finalize(&mut budget()).map_err(error)?;
    let source = SourceSnapshot::new(
        SourceId("decoded".into()),
        1,
        "memory:decoded".into(),
        b"\"a[x/y]{b/note}\"".to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let result = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &r,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(error)?;
    let SentenceOutcome::Matched(value) = result.outcome else {
        return Err("expected literal".into());
    };
    Ok((r, source, value.syntax))
}
fn run(
    payload: &NdfValue,
    owner: Option<&SourceSnapshot>,
    r: &SchemaRegistry,
    b: &mut Budget,
    repeat: bool,
) -> Result<DocumentSyntax, String> {
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(r, &store, &mut admission).map_err(error)?;
    let input = decode(payload, owner, r, &mut codec, b).map_err(error)?;
    if repeat {
        super::super::document(input.syntax(), r, b, codec.source_admission()).map_err(error)
    } else {
        super::super::document::decoded_document(&input, b, codec.source_admission()).map_err(error)
    }
}
#[test]
fn codec_proof_reuses_one_sentence_validation_for_both_payloads() -> Result<(), String> {
    let (r, source, syntax) = fixture()?;
    for referenced in [false, true] {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &store, &mut admission).map_err(error)?;
        let payload = if referenced {
            portable::literal::to_value(&syntax, &r, &mut codec, &mut budget())
        } else {
            portable::syntax::to_value(&syntax, &r, &mut codec, &mut budget())
        }
        .map_err(error)?;
        let owner = referenced.then_some(&source);
        let before = payload.clone();
        let mut once = budget();
        let actual = run(&payload, owner, &r, &mut once, false)?;
        let mut repeated = budget();
        let expected = run(&payload, owner, &r, &mut repeated, true)?;
        assert_eq!(actual, expected);
        assert_eq!(payload, before);
        assert!(once.usage().work < repeated.usage().work);
        assert!(once.usage().allocation_units < repeated.usage().allocation_units);
        let used = once.usage();
        for (limits, reason) in [
            (
                Limits {
                    work: used.work - 1,
                    ..budget().limits()
                },
                StopReason::WorkLimit,
            ),
            (
                Limits {
                    nodes: used.nodes - 1,
                    ..budget().limits()
                },
                StopReason::NodeLimit,
            ),
            (
                Limits {
                    allocation_units: used.allocation_units - 1,
                    ..budget().limits()
                },
                StopReason::AllocationLimit,
            ),
        ] {
            let mut b = Budget::new(limits);
            assert!(run(&payload, owner, &r, &mut b, false).is_err());
            assert_eq!(b.poll(), Err(reason));
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(run(&payload, owner, &r, &mut cancelled, false).is_err());
        assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
        let mut malformed = payload.clone();
        let NdfValue::Record(record) = &mut malformed else {
            return Err("expected record".into());
        };
        record.kind = "UnrecognizedSentencePayload".into();
        assert!(run(&malformed, owner, &r, &mut budget(), false).is_err());
    }
    // The unrestricted public entry still rejects malformed input.
    let mut malformed = syntax;
    malformed.locations.clear();
    assert!(
        super::super::document(
            &malformed,
            &r,
            &mut budget(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    Ok(())
}
