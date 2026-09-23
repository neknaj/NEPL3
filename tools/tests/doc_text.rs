use nepl3_core::value_codec::FoundationValueCodec;
use nepl3_core::{budget::*, source::*, value::NdfValue};
use nepl3_doc_core::{
    check::Category,
    lower,
    model::*,
    portable::text as wire,
    text::{
        self, AnnotationPolicy::*, PlainTextFailure, PlainTextOutcome, PlainTextRequest,
        ResolvedInlineText,
    },
};
use nepl3_tools::doc::source::{budget, compiled, err, with_input};
use nepl3_wire::foundation::FoundationCodec;

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
