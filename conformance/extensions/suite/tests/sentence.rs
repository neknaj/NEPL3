//! Public-only Sentence reader -> suite adapter -> Doc plain text consumer.
use nepl3_core::{
    budget::{Budget, Limits},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
};
use nepl3_doc_core::{
    model::DocRoot,
    text::{AnnotationPolicy, PlainTextOutcome, PlainTextRequest},
};
use nepl3_sentence_core::literal::{self, SentenceOutcome};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 10_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}

fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}

#[test]
fn sentence_literal_reaches_doc_operation_through_public_adapter() -> Result<(), String> {
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_sentence_core::schema::descriptor(&mut b),
        nepl3_doc_core::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(error)?;
        registry
            .register(
                descriptor.reference(&mut b).map_err(error)?,
                descriptor,
                &mut b,
            )
            .map_err(error)?;
    }
    registry.finalize(&mut b).map_err(error)?;
    let source = SourceSnapshot::new(
        SourceId("external-sentence".into()),
        1,
        "memory:external-sentence".into(),
        "\"[漢字/かんじ]を読む。\"".as_bytes().to_vec(),
        &mut b,
    )
    .map_err(error)?;
    let parsed = literal::read(
        &source,
        0,
        source.text().len() as u64,
        true,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(error)?;
    let SentenceOutcome::Matched(parsed) = parsed.outcome else {
        return Err("expected complete Sentence literal".into());
    };
    let document = nepl3_suite::adapters::sentence::document(
        &parsed.syntax,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(error)?;
    assert_eq!(document.sources, parsed.syntax.sources);
    assert_eq!(document.origins, parsed.syntax.origins);
    let DocRoot::Sentence(sentence) = document.value.root else {
        return Err("expected Doc Sentence root".into());
    };
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(error)?;
    let reply = nepl3_doc_core::text::plain_text(
        &PlainTextRequest {
            document,
            sentence,
            policy: AnnotationPolicy::BaseOnly,
            resolved: vec![],
        },
        &registry,
        &mut codec,
        &mut b,
    )
    .map_err(error)?;
    // BaseOnly keeps the authored base and surrounding text, excluding its reading.
    assert_eq!(
        reply.outcome,
        PlainTextOutcome::Complete {
            text: "漢字を読む。".into()
        }
    );
    Ok(())
}
