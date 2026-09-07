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

#[test]
fn plain_text_first_receiver_rejects_old_source_other_guest_and_owner_environment()
-> Result<(), String> {
    with_document(INPUT, |doc, r| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let prepared = text::prepare(&doc, r, &mut c, &mut budget()).map_err(err)?;
        let identity = prepared.identity().clone();
        let original = ResolvedInlineText {
            document_digest: identity.document_digest,
            embed: identity.embeds[0].embed,
            guest_digest: identity.embeds[0].guest_digest,
            text: "three".into(),
        };
        let receive = |request: &PlainTextRequest| -> Result<PlainTextOutcome, String> {
            let bytes = {
                let mut a = SourceAdmission::default();
                let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
                nepl3_wire::encode(
                    &wire::request_to_value(request, r, &mut c, &mut budget()).map_err(err)?,
                    &mut budget(),
                )
                .map_err(err)?
            };
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
            let request = wire::request_from_value(
                &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                r,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            Ok(text::plain_text(&request, r, &mut c, &mut budget())
                .map_err(err)?
                .outcome)
        };
        let newer_source = with_document(&INPUT.replace("\"A\"", "\"Z\""), |doc, _| Ok(doc))?;
        for mode in 0..3 {
            let mut changed = doc.clone();
            match mode {
                0 => {
                    // Reparse a same-length edit under the same source ID and
                    // revision. Every actual source/span now has the new digest.
                    changed = newer_source.clone();
                }
                1 => {
                    changed.value.embeds.swap(0, 1);
                }
                _ => {
                    let closure = &mut changed.value.embeds[0].closure;
                    closure.owner_environment.value.resources.push(
                        nepl3_core::syntax::ResourceContent {
                            id: "host-context".into(),
                            digest: Digest::of(b"new context"),
                            bytes: b"new context".to_vec(),
                        },
                    );
                    let digest = c
                        .environment_digest(&closure.owner_environment.value, &mut budget())
                        .map_err(err)?;
                    closure.owner_environment.digest = digest;
                    closure.syntax.environment.digest = digest;
                }
            }
            let mut fresh_admission = SourceAdmission::default();
            let mut fresh_codec =
                FoundationCodec::new(r, &empty, &mut fresh_admission).map_err(err)?;
            let actual =
                text::prepare(&changed, r, &mut fresh_codec, &mut budget()).map_err(err)?;
            assert_ne!(actual.identity().document_digest, identity.document_digest);
            let mut stale = original.clone();
            if mode > 0 {
                // Correct current document hash cannot authorize stale guest text.
                stale.document_digest = actual.identity().document_digest;
                assert_ne!(actual.identity().embeds[0].guest_digest, stale.guest_digest);
            }
            let request = PlainTextRequest {
                sentence: root(&changed)?,
                document: changed,
                policy: BaseOnly,
                resolved: vec![stale],
            };
            assert_eq!(
                receive(&request)?,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::InvalidResolution {
                        entry: 0,
                        reason: if mode == 0 {
                            ResolutionMismatch::Document
                        } else {
                            ResolutionMismatch::Guest
                        }
                    }
                }
            );
        }
        Ok(())
    })
}

#[test]
fn plain_text_preparation_stops_are_formal_and_report_overflow_requires_stopped()
-> Result<(), String> {
    with_document(r#""source""#, |doc, r| {
        let original = doc.clone();
        let empty = SourceStore::default();
        let request = PlainTextRequest {
            sentence: root(&doc)?,
            document: doc,
            policy: BaseOnly,
            resolved: vec![],
        };
        for reason in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
            StopReason::OutputLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::NodeLimit => limits.nodes = 0,
                StopReason::DepthLimit => limits.depth = 0,
                StopReason::OutputLimit => limits.output_bytes = 0,
                _ => {}
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
            let reply = text::plain_text(&request, r, &mut c, &mut b).map_err(err)?;
            assert_eq!(reply.outcome, PlainTextOutcome::Stopped { reason });
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(reply.report.usage, b.usage());
            assert_eq!(request.document, original);
        }
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let mut reply = text::plain_text(&request, r, &mut c, &mut budget()).map_err(err)?;
        reply.report.trace_overflow = Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
        assert!(matches!(
            wire::reply_to_value(&reply, &request.document, r, &mut c, &mut budget()),
            Err(nepl3_doc_core::portable::PortableError::Shape)
        ));
        for reason in [StopReason::Cancelled, StopReason::WorkLimit] {
            reply.outcome = PlainTextOutcome::Stopped { reason };
            let value = wire::reply_to_value(&reply, &request.document, r, &mut c, &mut budget())
                .map_err(err)?;
            assert_eq!(
                wire::reply_from_value(&value, &request.document, r, &mut c, &mut budget())
                    .map_err(err)?,
                reply
            );
            let mut malformed = value;
            let NdfValue::Record(record) = &mut malformed else {
                return Err("reply record".into());
            };
            let NdfValue::Variant(outcome) = &mut record.fields[0] else {
                return Err("outcome variant".into());
            };
            outcome.variant = "Complete".into();
            outcome.fields = vec![NdfValue::Text("source".into())];
            assert!(matches!(
                wire::reply_from_value(&malformed, &request.document, r, &mut c, &mut budget()),
                Err(nepl3_doc_core::portable::PortableError::Shape)
            ));
        }
        Ok(())
    })
}

#[test]
fn plain_text_preserves_output_work_allocation_depth_and_cancel_stops() -> Result<(), String> {
    with_document(r#""a[字/じ]{b/note}""#, |doc, r| {
        let before = doc.clone();
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let sentence = root(&doc)?;
        for reason in [
            StopReason::OutputLimit,
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::OutputLimit => limits.output_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::DepthLimit => limits.depth = 0,
                _ => {}
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            let reply = project(&doc, WithAllNotes, &[], r, &mut c, &mut b)?;
            assert_eq!(reply.outcome, PlainTextOutcome::Stopped { reason });
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(reply.report.usage, b.usage());
            assert_eq!(doc, before);
            let value =
                wire::reply_to_value(&reply, &doc, r, &mut c, &mut budget()).map_err(err)?;
            assert_eq!(
                wire::reply_from_value(&value, &doc, r, &mut c, &mut budget()).map_err(err)?,
                reply
            );
        }
        let mut b = budget();
        let request = PlainTextRequest {
            document: doc.clone(),
            sentence,
            policy: WithAllNotes,
            resolved: vec![],
        };
        let reply = b
            .with_depth_at_least(7, |b| text::plain_text(&request, r, &mut c, b))
            .map_err(err)?;
        assert!(matches!(reply.outcome, PlainTextOutcome::Complete { .. }));
        assert!(b.usage().depth > 7);
        assert_eq!(b.current_depth(), 0);
        Ok(())
    })
}
