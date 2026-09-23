use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_core::{budget::*, source::*, value::NdfValue};
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
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;

fn with_document<T>(
    input: &str,
    f: impl FnOnce(DocumentSyntax, &nepl3_core::schema::SchemaRegistry) -> Result<T, String>,
) -> Result<T, String> {
    with_document_root(input, "Sentence", Category::Sentence, f)
}
fn with_document_root<T>(
    input: &str,
    category: &str,
    root: Category,
    f: impl FnOnce(DocumentSyntax, &nepl3_core::schema::SchemaRegistry) -> Result<T, String>,
) -> Result<T, String> {
    let compiled = compiled()?;
    with_input(&compiled, input, category, |tree, profile, b, a| {
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
            root,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        f(doc, profile.registry())
    })
}
#[test]
fn nested_foreign_text_obeys_sentence_policy_before_doc_transport() -> Result<(), String> {
    use nepl3_sentence_core::{model::EmbedRef as SentenceEmbed, text as st};
    let compiled = compiled()?;
    let input = r#"sentence sentence cons ruby text "A" math add 1 2 cons anno text "B" cons math mul 3 4 cons ruby text "C" text "see" nil cons break cons code "[]{}" nil"#;
    with_document(input, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let [embed] = doc.value.embeds.as_slice() else {
            return Err("one Sentence".into());
        };
        let syntax = nepl3_suite::adapters::document::sentence::lower(
            embed,
            embed.schema(),
            &[nepl3_sentence_core::lower::ForeignInlineForm {
                kind: "Form:InlineMath",
                guest_schema: &compiled.others[0].schema,
                guest_category: "Expr",
            }],
            r,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        assert_eq!(syntax.value.embeds.len(), 2);
        let prepared =
            st::prepare(&syntax.value, r, &mut budget(), codec.source_admission()).map_err(err)?;
        let resolved = [
            prepared
                .resolve(SentenceEmbed(0), "three", &mut budget())
                .map_err(err)?,
            prepared
                .resolve(SentenceEmbed(1), "twelve", &mut budget())
                .map_err(err)?,
        ];
        // Omitted annotations still cannot carry duplicate or wrong-scope data.
        let duplicate = [
            prepared
                .resolve(SentenceEmbed(1), "x", &mut budget())
                .map_err(err)?,
            prepared
                .resolve(SentenceEmbed(1), "y", &mut budget())
                .map_err(err)?,
        ];
        assert_eq!(
            prepared.render(st::AnnotationPolicy::BaseOnly, &duplicate, &mut budget()),
            Err(st::Error::Duplicate(SentenceEmbed(1)))
        );
        let other_value = syntax.value.clone();
        let other =
            st::prepare(&other_value, r, &mut budget(), codec.source_admission()).map_err(err)?;
        let foreign = other
            .resolve(SentenceEmbed(1), "x", &mut budget())
            .map_err(err)?;
        assert_eq!(
            prepared.render(st::AnnotationPolicy::BaseOnly, &[foreign], &mut budget()),
            Err(st::Error::WrongScope)
        );
        let identity = text::prepare(&doc, r, &mut codec, &mut budget())
            .map_err(err)?
            .identity()
            .clone();
        assert_eq!(identity.embeds.len(), 1);
        for (policy, inner, count, expected) in [
            (BaseOnly, st::AnnotationPolicy::BaseOnly, 0, "AB\n[]{}"),
            (
                WithReadings,
                st::AnnotationPolicy::WithReadings,
                1,
                "A[three]B\n[]{}",
            ),
            (
                WithAllNotes,
                st::AnnotationPolicy::WithAllNotes,
                2,
                "A[three]B{twelve/C[see]}\n[]{}",
            ),
        ] {
            let output = prepared
                .render(inner, &resolved[..count], &mut budget())
                .map_err(err)?;
            assert_eq!(output, expected);
            if count > 0 {
                assert_eq!(
                    prepared.render(inner, &resolved[..count - 1], &mut budget()),
                    Err(st::Error::Unresolved(SentenceEmbed((count - 1) as u64)))
                );
            }
            let target = &identity.embeds[0];
            let request = PlainTextRequest {
                document: doc.clone(),
                sentence: root(&doc)?,
                policy,
                resolved: vec![ResolvedInlineText {
                    document_digest: identity.document_digest,
                    embed: target.embed,
                    guest_digest: target.guest_digest,
                    policy,
                    text: output,
                }],
            };
            let value =
                wire::request_to_value(&request, r, &mut codec, &mut budget()).map_err(err)?;
            for reason in [
                ResolutionMismatch::Document,
                ResolutionMismatch::Guest,
                ResolutionMismatch::Embed,
                ResolutionMismatch::Duplicate,
                ResolutionMismatch::Policy,
            ] {
                let mut invalid = request.clone();
                let entry = match reason {
                    ResolutionMismatch::Document => {
                        invalid.resolved[0].document_digest = Digest::of(b"other");
                        0
                    }
                    ResolutionMismatch::Guest => {
                        invalid.resolved[0].guest_digest = Digest::of(b"other");
                        0
                    }
                    ResolutionMismatch::Embed => {
                        invalid.resolved[0].embed = EmbedRef(u64::MAX);
                        0
                    }
                    ResolutionMismatch::Duplicate => {
                        invalid.resolved.push(invalid.resolved[0].clone());
                        1
                    }
                    ResolutionMismatch::Policy => {
                        invalid.resolved[0].policy = if policy == BaseOnly {
                            WithReadings
                        } else {
                            BaseOnly
                        };
                        0
                    }
                };
                assert_eq!(
                    text::plain_text(&invalid, r, &mut codec, &mut budget())
                        .map_err(err)?
                        .outcome,
                    PlainTextOutcome::Invalid {
                        error: PlainTextFailure::InvalidResolution { entry, reason }
                    }
                );
            }
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let mut a = SourceAdmission::default();
            let mut receiver = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
            let received = wire::request_from_value(
                &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                r,
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(
                text::prepare(&received.document, r, &mut receiver, &mut budget())
                    .map_err(err)?
                    .identity(),
                &identity
            );
            let reply =
                text::plain_text(&received, r, &mut receiver, &mut budget()).map_err(err)?;
            assert_eq!(
                reply.outcome,
                PlainTextOutcome::Complete {
                    text: expected.into()
                }
            );
            let wire_reply =
                wire::reply_to_value(&reply, &received.document, r, &mut receiver, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&wire_reply, &mut budget()).map_err(err)?;
            assert_eq!(
                wire::reply_from_value(
                    &nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?,
                    &received.document,
                    r,
                    &mut receiver,
                    &mut budget()
                )
                .map_err(err)?,
                reply
            );
            for length in [31, 33] {
                let mut malformed = value.clone();
                let NdfValue::Record(record) = &mut malformed else {
                    return Err("request".into());
                };
                let NdfValue::List(entries) = &mut record.fields[3] else {
                    return Err("resolved".into());
                };
                let NdfValue::Record(entry) = &mut entries[0] else {
                    return Err("entry".into());
                };
                entry.fields[0] = NdfValue::Bytes(vec![0; length]);
                assert!(matches!(
                    wire::request_from_value(&malformed, r, &mut receiver, &mut budget()),
                    Err(nepl3_doc_core::portable::PortableError::Shape)
                ));
            }
        }
        Ok(())
    })
}

#[test]
fn independent_sentence_text_is_bound_to_doc_policy_and_portable_identity() -> Result<(), String> {
    with_document(r#"sentence "a[字/じ]{b/note}""#, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        for (policy, expected) in [
            (BaseOnly, "a字b"),
            (WithReadings, "a字[じ]b"),
            (WithAllNotes, "a字[じ]b{note}"),
        ] {
            let resolved = resolve_sentence(&doc, policy, r, &mut codec)?;
            assert_eq!(resolved.len(), 1);
            let request = PlainTextRequest {
                document: doc.clone(),
                sentence: root(&doc)?,
                policy,
                resolved,
            };
            let value =
                wire::request_to_value(&request, r, &mut codec, &mut budget()).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut admission = SourceAdmission::default();
            let mut receiver = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let mut received =
                wire::request_from_value(&value, r, &mut receiver, &mut budget()).map_err(err)?;
            let reply =
                text::plain_text(&received, r, &mut receiver, &mut budget()).map_err(err)?;
            assert_eq!(
                reply.outcome,
                PlainTextOutcome::Complete {
                    text: expected.into()
                }
            );
            received.policy = if policy == BaseOnly {
                WithReadings
            } else {
                BaseOnly
            };
            assert_eq!(
                text::plain_text(&received, r, &mut receiver, &mut budget())
                    .map_err(err)?
                    .outcome,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::InvalidResolution {
                        entry: 0,
                        reason: text::ResolutionMismatch::Policy
                    }
                }
            );
            received.policy = policy;
            let embed = received.resolved[0].embed;
            received.resolved.clear();
            assert_eq!(
                text::plain_text(&received, r, &mut receiver, &mut budget())
                    .map_err(err)?
                    .outcome,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::UnresolvedEmbed { embed }
                }
            );
        }
        Ok(())
    })
}
fn root(doc: &DocumentSyntax) -> Result<SentenceRef, String> {
    match doc.value.root {
        DocRoot::Sentence(s) => Ok(s),
        _ => Err("sentence root".into()),
    }
}
// The host projects the selected independent Sentence before invoking Doc's
// portable text operation. The returned data carries scope and policy, not a
// transferable validation proof or permission to evaluate nested guests.
fn resolve_sentence<C: FoundationValueCodec>(
    doc: &DocumentSyntax,
    policy: text::AnnotationPolicy,
    r: &nepl3_core::schema::SchemaRegistry,
    codec: &mut C,
) -> Result<Vec<ResolvedInlineText>, String>
where
    C::Error: std::fmt::Debug,
{
    use nepl3_sentence_core::text as sentence_text;
    let mut b = budget();
    let prepared = text::prepare(doc, r, codec, &mut b).map_err(err)?;
    let identity = prepared.identity();
    let mut resolved = Vec::new();
    for target in &identity.embeds {
        let embed = &doc.value.embeds[target.embed.0 as usize];
        let syntax = nepl3_suite::adapters::document::sentence::lower(
            embed,
            embed.schema(),
            &[],
            r,
            codec,
            &mut b,
        )
        .map_err(err)?;
        let prepared = sentence_text::prepare(&syntax.value, r, &mut b, codec.source_admission())
            .map_err(err)?;
        let selected = match policy {
            BaseOnly => sentence_text::AnnotationPolicy::BaseOnly,
            WithReadings => sentence_text::AnnotationPolicy::WithReadings,
            WithAllNotes => sentence_text::AnnotationPolicy::WithAllNotes,
        };
        let text = prepared.render(selected, &[], &mut b).map_err(err)?;
        resolved.push(ResolvedInlineText {
            document_digest: identity.document_digest,
            embed: target.embed,
            guest_digest: target.guest_digest,
            policy,
            text,
        });
    }
    Ok(resolved)
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
fn plain_text_preparation_stops_are_formal_and_report_overflow_requires_stopped()
-> Result<(), String> {
    with_document(r#"sentence "source""#, |doc, r| {
        let original = doc.clone();
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let resolved = resolve_sentence(&doc, BaseOnly, r, &mut codec)?;
        let request = PlainTextRequest {
            sentence: root(&doc)?,
            document: doc,
            policy: BaseOnly,
            resolved,
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
    with_document(r#"sentence "a[字/じ]{b/note}""#, |doc, r| {
        let before = doc.clone();
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let sentence = root(&doc)?;
        let resolved = resolve_sentence(&doc, WithAllNotes, r, &mut c)?;
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
            let reply = project(&doc, WithAllNotes, &resolved, r, &mut c, &mut b)?;
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
            resolved,
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

#[test]
fn plain_text_first_receiver_rejects_old_source_other_guest_and_owner_environment()
-> Result<(), String> {
    let input = r#"paragraph cons sentence "A" cons sentence "B" nil"#;
    with_document_root(input, "Block", Category::Block, |doc, r| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
        let prepared = text::prepare(&doc, r, &mut c, &mut budget()).map_err(err)?;
        let identity = prepared.identity().clone();
        assert_eq!(identity.embeds.len(), 2);
        let sentence = SentenceRef(doc.value.nodes.iter().position(|node|
            matches!(node.kind, DocKind::Sentence { syntax } if syntax == identity.embeds[0].embed)
        ).ok_or("first Sentence")? as u64);
        let original = ResolvedInlineText {
            document_digest: identity.document_digest,
            embed: identity.embeds[0].embed,
            guest_digest: identity.embeds[0].guest_digest,
            policy: BaseOnly,
            text: "A".into(),
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
        assert_eq!(
            receive(&PlainTextRequest {
                document: doc.clone(),
                sentence,
                policy: BaseOnly,
                resolved: vec![original.clone()]
            })?,
            PlainTextOutcome::Complete { text: "A".into() }
        );
        let newer_source = with_document_root(
            r#"paragraph cons sentence "Z" cons sentence "B" nil"#,
            "Block",
            Category::Block,
            |doc, _| Ok(doc),
        )?;
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
                    let DocContent::Syntax { closure } = &mut changed.value.embeds[0].content
                    else {
                        return Err("Sentence syntax closure".into());
                    };
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
                sentence,
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
