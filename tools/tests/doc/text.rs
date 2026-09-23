use super::*;
use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_doc_core::{
    check::Category,
    lower,
    model::*,
    portable::text as wire,
    text::{
        self, AnnotationPolicy::*, PlainTextFailure, PlainTextOutcome, PlainTextRequest,
        ResolutionMismatch, ResolvedInlineText,
    },
};

fn with_document<T>(
    input: &str,
    f: impl FnOnce(DocumentSyntax, &nepl3_core::schema::SchemaRegistry) -> Result<T, String>,
) -> Result<T, String> {
    let compiled = compiled()?;
    with_input(&compiled, input, "Sentence", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let doc = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Sentence,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        f(doc, profile.registry())
    })
}
fn root(doc: &DocumentSyntax) -> Result<SentenceRef, String> {
    match doc.value.root {
        DocRoot::Sentence(s) => Ok(s),
        _ => Err("sentence root".into()),
    }
}
fn project<C: FoundationValueCodec>(
    doc: &DocumentSyntax,
    policy: text::AnnotationPolicy,
    resolved: &[ResolvedInlineText],
    r: &nepl3_core::schema::SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<text::PlainTextReply, String>
where
    C::Error: std::fmt::Debug,
{
    text::plain_text(
        &PlainTextRequest {
            document: doc.clone(),
            sentence: root(doc)?,
            policy,
            resolved: resolved.to_vec(),
        },
        r,
        c,
        b,
    )
    .map_err(err)
}
#[test]
fn plain_text_uses_explicit_labels_alt_and_author_whitespace() -> Result<(), String> {
    with_document(
        r#"sentence cons em strong anchor here ref elsewhere link external "https://example.invalid" concat cons text " L " cons text "[]" nil cons image asset "logo" none sentence cons text "alt\n " nil nil"#,
        |doc, r| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
            let mut b = budget();
            let reply = project(&doc, BaseOnly, &[], r, &mut c, &mut b)?;
            assert_eq!(
                reply.outcome,
                PlainTextOutcome::Complete {
                    text: " L []alt\n ".into()
                }
            );
            // No label lookup, URI fetch, asset load or alt inference is required.
            let expected = doc
                .sources
                .iter()
                .map(|s| s.text().len() as u64)
                .sum::<u64>();
            assert_eq!(b.usage().source_bytes, expected);
            for node in [u64::MAX, (1u64 << 32) + root(&doc)?.0] {
                let request = PlainTextRequest {
                    document: doc.clone(),
                    sentence: SentenceRef(node),
                    policy: BaseOnly,
                    resolved: vec![],
                };
                assert_eq!(
                    text::plain_text(&request, r, &mut c, &mut budget())
                        .map_err(err)?
                        .outcome,
                    PlainTextOutcome::Invalid {
                        error: PlainTextFailure::ExpectedSentence { node }
                    }
                );
            }
            Ok(())
        },
    )
}
