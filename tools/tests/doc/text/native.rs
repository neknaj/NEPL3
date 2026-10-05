use super::*;
use nepl3_core::budget::{Resource, StopReason};

#[test]
fn prepared_projection_reuses_admission_and_preserves_annotations() -> Result<(), String> {
    with_document(
        r#"sentence cons ruby text "A" text "a" cons anno text "B" cons text "note" nil nil"#,
        |doc, r| {
            let before = doc.clone();
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let mut b = budget();
            let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
            let admitted = b.usage().source_bytes;
            let work = b.usage().work;
            for (policy, expected) in [
                (BaseOnly, "AB"),
                (WithReadings, "A[a]B"),
                (WithAllNotes, "A[a]B{note}"),
            ] {
                let reply = prepared
                    .project(root(&doc)?, policy, &[], &mut b)
                    .map_err(err)?;
                assert_eq!(
                    reply.outcome,
                    PlainTextOutcome::Complete {
                        text: expected.into()
                    }
                );
                assert_eq!(reply.report.usage, b.usage());
                assert_eq!(b.usage().source_bytes, admitted);
            }
            assert!(b.usage().work > work);
            assert_eq!(doc, before);
            Ok(())
        },
    )
}

#[test]
fn prepared_projection_checks_limits_entries_and_stops() -> Result<(), String> {
    with_document(r#"sentence cons text "abc" nil"#, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let mut b = budget();
        let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
        let mut limits = b.limits();
        limits.work -= 1;
        let mut different = Budget::new(limits);
        let usage = different.usage();
        assert_eq!(
            prepared.project(root(&doc)?, BaseOnly, &[], &mut different),
            Err(text::PreparedTextError::LimitsMismatch)
        );
        assert_eq!(different.usage(), usage);
        assert_eq!(
            prepared
                .project(SentenceRef(u64::MAX), BaseOnly, &[], &mut b)
                .map_err(err)?
                .outcome,
            PlainTextOutcome::Invalid {
                error: PlainTextFailure::ExpectedSentence { node: u64::MAX }
            }
        );
        b.cancel();
        let reply = prepared
            .project(root(&doc)?, BaseOnly, &[], &mut b)
            .map_err(err)?;
        assert_eq!(
            reply.outcome,
            PlainTextOutcome::Stopped {
                reason: StopReason::Cancelled
            }
        );
        assert_eq!(reply.report.usage, b.usage());
        Ok(())
    })
}

#[test]
fn prepared_projection_respects_remaining_work() -> Result<(), String> {
    with_document(r#"sentence cons text "abc" nil"#, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let mut b = budget();
        let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
        let remaining = b.limits().work - b.usage().work;
        b.charge(Resource::Work, remaining).map_err(err)?;
        let reply = prepared
            .project(root(&doc)?, BaseOnly, &[], &mut b)
            .map_err(err)?;
        assert_eq!(
            reply.outcome,
            PlainTextOutcome::Stopped {
                reason: StopReason::WorkLimit
            }
        );
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        Ok(())
    })
}

#[test]
fn prepared_projection_preserves_output_allocation_and_caller_depth_limits() -> Result<(), String> {
    with_document(r#"sentence cons text "abc" nil"#, |doc, r| {
        for reason in [
            StopReason::OutputLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
        ] {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let mut b = budget();
            let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
            let reply = match reason {
                StopReason::OutputLimit => {
                    b.charge(
                        Resource::OutputBytes,
                        b.limits().output_bytes - b.usage().output_bytes,
                    )
                    .map_err(err)?;
                    prepared
                        .project(root(&doc)?, BaseOnly, &[], &mut b)
                        .map_err(err)?
                }
                StopReason::AllocationLimit => {
                    b.charge(
                        Resource::AllocationUnits,
                        b.limits().allocation_units - b.usage().allocation_units,
                    )
                    .map_err(err)?;
                    prepared
                        .project(root(&doc)?, BaseOnly, &[], &mut b)
                        .map_err(err)?
                }
                _ => {
                    let sentence = root(&doc)?;
                    b.with_depth_at_least(b.limits().depth, |inner| -> Result<_, StopReason> {
                        Ok(prepared.project(sentence, BaseOnly, &[], inner))
                    })
                    .map_err(err)?
                    .map_err(err)?
                }
            };
            assert_eq!(reply.outcome, PlainTextOutcome::Stopped { reason });
            assert_eq!(b.poll(), Err(reason));
            assert_eq!(b.current_depth(), 0);
            assert_eq!(reply.report.usage, b.usage());
        }
        Ok(())
    })
}

#[test]
fn prepared_projection_revalidates_resolved_guest_data() -> Result<(), String> {
    with_document(r#"sentence cons math Math add 1 2 nil"#, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let mut b = budget();
        let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
        let identity = prepared.identity().clone();
        let target = &identity.embeds[0];
        assert_eq!(
            prepared
                .project(root(&doc)?, BaseOnly, &[], &mut b)
                .map_err(err)?
                .outcome,
            PlainTextOutcome::Invalid {
                error: PlainTextFailure::UnresolvedEmbed {
                    embed: target.embed
                }
            }
        );
        let original = ResolvedInlineText {
            document_digest: identity.document_digest,
            embed: target.embed,
            guest_digest: target.guest_digest,
            text: "three".into(),
        };
        assert_eq!(
            prepared
                .project(
                    root(&doc)?,
                    BaseOnly,
                    std::slice::from_ref(&original),
                    &mut b
                )
                .map_err(err)?
                .outcome,
            PlainTextOutcome::Complete {
                text: "three".into()
            }
        );
        for reason in [
            ResolutionMismatch::Document,
            ResolutionMismatch::Guest,
            ResolutionMismatch::Embed,
            ResolutionMismatch::Duplicate,
        ] {
            let mut resolved = vec![original.clone()];
            match reason {
                ResolutionMismatch::Document => {
                    resolved[0].document_digest = Digest::of(b"wrong document")
                }
                ResolutionMismatch::Guest => resolved[0].guest_digest = Digest::of(b"wrong guest"),
                ResolutionMismatch::Embed => resolved[0].embed = EmbedRef(u64::MAX),
                ResolutionMismatch::Duplicate => resolved.push(original.clone()),
            }
            let entry = u64::from(reason == ResolutionMismatch::Duplicate);
            assert_eq!(
                prepared
                    .project(root(&doc)?, BaseOnly, &resolved, &mut b)
                    .map_err(err)?
                    .outcome,
                PlainTextOutcome::Invalid {
                    error: PlainTextFailure::InvalidResolution { entry, reason }
                }
            );
        }
        assert_eq!(prepared.identity(), &identity);
        Ok(())
    })
}

#[test]
fn prepared_projection_rejects_absolute_depth_overflow() -> Result<(), String> {
    with_document(r#"sentence cons text "abc" nil"#, |doc, r| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
        let mut limits = budget().limits();
        limits.depth = u64::MAX;
        let mut b = Budget::new(limits);
        let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
        let sentence = root(&doc)?;
        let reply = b
            .with_depth_at_least(u64::MAX, |inner| -> Result<_, StopReason> {
                Ok(prepared.project(sentence, BaseOnly, &[], inner))
            })
            .map_err(err)?
            .map_err(err)?;
        assert_eq!(
            reply.outcome,
            PlainTextOutcome::Stopped {
                reason: StopReason::DepthLimit
            }
        );
        assert_eq!(b.poll(), Err(StopReason::DepthLimit));
        assert_eq!(b.current_depth(), 0);
        Ok(())
    })
}

#[test]
fn prepared_projection_reuses_one_document_for_distinct_image_alts() -> Result<(), String> {
    with_document(
        r#"sentence cons image asset "first" none sentence cons ruby text "A" text "a" nil cons image asset "second" none sentence cons anno text "B" cons text "note" nil nil nil"#,
        |doc, r| {
            let alts: Vec<_> = doc
                .value
                .nodes
                .iter()
                .filter_map(|node| match node.kind {
                    DocKind::InlineImage { alt, .. } => Some(alt),
                    _ => None,
                })
                .collect();
            assert_eq!(alts.len(), 2);
            assert_ne!(alts[0], alts[1]);
            let sentence = root(&doc)?;
            assert!(alts.iter().all(|alt| *alt != sentence));
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(r, &empty, &mut admission).map_err(err)?;
            let mut b = budget();
            let prepared = text::prepare(&doc, r, &mut codec, &mut b).map_err(err)?;
            let admitted = b.usage().source_bytes;
            let mut texts = Vec::new();
            for alt in alts {
                match prepared
                    .project(alt, BaseOnly, &[], &mut b)
                    .map_err(err)?
                    .outcome
                {
                    PlainTextOutcome::Complete { text } => texts.push(text),
                    outcome => return Err(format!("unexpected alt outcome: {outcome:?}")),
                }
                assert_eq!(b.usage().source_bytes, admitted);
            }
            texts.sort();
            assert_eq!(texts, ["A", "B"]);
            Ok(())
        },
    )
}
