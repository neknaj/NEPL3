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
const INPUT: &str = r#"sentence cons ruby text "A" math Math add 1 2 cons anno text "B" cons math Math mul 3 4 cons ruby text "C" text "see" nil cons break cons code "[]{}" nil"#;

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

#[test]
fn plain_text_projects_only_policy_selected_embeds_and_roundtrips_typed_requests()
-> Result<(), String> {
    with_document(INPUT, |doc, r| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let mut b = budget();
        let prepared = text::prepare(&doc, r, &mut c, &mut b).map_err(err)?;
        let sentence = root(&doc)?;
        let identity = prepared.identity();
        assert_eq!(identity.embeds.len(), 2);
        let resolved: Vec<_> = identity
            .embeds
            .iter()
            .zip(["three", "twelve"])
            .map(|(target, text)| ResolvedInlineText {
                document_digest: identity.document_digest,
                embed: target.embed,
                guest_digest: target.guest_digest,
                text: text.into(),
            })
            .collect();
        // These expectations are the fixed projection contract, not a pretty
        // printer oracle: untouched brackets and LF are deliberately retained.
        for (policy, provided, expected) in [
            (BaseOnly, &resolved[..0], "AB\n[]{}"),
            (WithReadings, &resolved[..1], "A[three]B\n[]{}"),
            (
                WithAllNotes,
                &resolved[..],
                "A[three]B{twelve/C[see]}\n[]{}",
            ),
        ] {
            let reply = project(&doc, policy, provided, r, &mut c, &mut b)?;
            assert_eq!(
                reply.outcome,
                PlainTextOutcome::Complete {
                    text: expected.into()
                }
            );
            let request = PlainTextRequest {
                document: doc.clone(),
                sentence,
                policy,
                resolved: provided.to_vec(),
            };
            let value = wire::request_to_value(&request, r, &mut c, &mut b).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut b).map_err(err)?;
            let mut admission = SourceAdmission::default();
            let mut receiver = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let mut received_budget = budget();
            let received = wire::request_from_value(
                &nepl3_wire::decode(&bytes, &mut received_budget).map_err(err)?,
                r,
                &mut receiver,
                &mut received_budget,
            )
            .map_err(err)?;
            let actual = text::prepare(&received.document, r, &mut receiver, &mut received_budget)
                .map_err(err)?;
            assert_eq!(actual.identity(), identity);
            let actual_reply =
                text::plain_text(&received, r, &mut receiver, &mut received_budget).map_err(err)?;
            assert_eq!(actual_reply.outcome, reply.outcome);
            let value = wire::reply_to_value(
                &actual_reply,
                &received.document,
                r,
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let result = wire::reply_from_value(
                &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                &received.document,
                r,
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(result, actual_reply);
            if policy == WithReadings {
                for length in [31, 33] {
                    let mut malformed =
                        wire::request_to_value(&request, r, &mut c, &mut b).map_err(err)?;
                    let NdfValue::Record(record) = &mut malformed else {
                        return Err("request record".into());
                    };
                    let NdfValue::List(entries) = &mut record.fields[3] else {
                        return Err("resolved list".into());
                    };
                    let NdfValue::Record(entry) = &mut entries[0] else {
                        return Err("resolved record".into());
                    };
                    entry.fields[0] = NdfValue::Bytes(vec![0; length]);
                    assert!(matches!(
                        wire::request_from_value(&malformed, r, &mut c, &mut budget()),
                        Err(nepl3_doc_core::portable::PortableError::Shape)
                    ));
                }
            }
        }
        for (policy, entries, missing) in [
            (WithReadings, &resolved[..0], 0),
            (WithAllNotes, &resolved[..1], 1),
        ] {
            assert_eq!(
                project(&doc, policy, entries, r, &mut c, &mut b)?.outcome,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::UnresolvedEmbed {
                        embed: resolved[missing].embed
                    }
                }
            );
        }
        // All supplied entries are checked, including notes omitted by policy.
        for reason in [
            ResolutionMismatch::Document,
            ResolutionMismatch::Guest,
            ResolutionMismatch::Embed,
            ResolutionMismatch::Duplicate,
        ] {
            let mut invalid = resolved.clone();
            let entry = match reason {
                ResolutionMismatch::Document => {
                    invalid[1].document_digest = Digest::of(b"other document");
                    1
                }
                ResolutionMismatch::Guest => {
                    invalid[1].guest_digest = Digest::of(b"other guest");
                    1
                }
                ResolutionMismatch::Embed => {
                    invalid[1].embed = EmbedRef(u64::MAX);
                    1
                }
                ResolutionMismatch::Duplicate => {
                    invalid.push(invalid[0].clone());
                    2
                }
            };
            assert_eq!(
                project(&doc, BaseOnly, &invalid, r, &mut c, &mut b)?.outcome,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::InvalidResolution { entry, reason }
                }
            );
        }
        Ok(())
    })
}
