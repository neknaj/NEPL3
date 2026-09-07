use super::*;
use nepl3_engine::{
    analysis::{BindingOptions, region::*},
    parse::{ParseCompletion, ParseOutcome},
    portable::{analysis, region},
};
use nepl3_wire::foundation::FoundationCodec;
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
fn formal() -> Result<CompiledLanguage, String> {
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.formal",
        &mut budget(),
        &mut SourceAdmission::default(),
    )
}
#[test]
fn region_first_receiver_rebuilds_tree_and_sidecar_without_sender_sources() -> Result<(), String> {
    let compiled = formal()?;
    for text in [
        "lambda x guest x",
        "lettext \"\\u{78}\" x",
        "lambda あ apply あ あ",
    ] {
        super::super::binding::with_completed_input(&compiled, text, |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut c =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let keyed = analysis::prepare(
                "region",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let input = region::prepare(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                .map_err(err)?;
            let request = RegionRequest {
                key: input.key(),
                source: parsed.tree().bundle.sources[0].reference(),
                offset: 7,
            };
            let reply = regions(
                &input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(
                matches!(reply.outcome, RegionOutcome::Complete { .. }),
                "{reply:?}"
            );
            let tree = analysis::request_to_value(&keyed, &mut c, &mut budget()).map_err(err)?;
            let sidecar =
                region::sidecar_value(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                    .map_err(err)?;
            let request_value =
                region::request_to_value(&request, profile.registry(), &mut c, &mut budget())
                    .map_err(err)?;
            let result = region::reply_to_value(&reply, &request, &input, &mut c, &mut budget())
                .map_err(err)?;
            let packet =
                nepl3_core::value::NdfValue::List(vec![tree, sidecar, request_value, result]);
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(err)?;
            let nepl3_core::value::NdfValue::List(ref packet) =
                nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?
            else {
                return Err("packet".into());
            };
            let receiver_empty = SourceStore::default();
            let mut receiver_admission = SourceAdmission::default();
            let mut c =
                FoundationCodec::new(profile.registry(), &receiver_empty, &mut receiver_admission)
                    .map_err(err)?;
            let tree = analysis::request_decode(&packet[0], profile, &mut c, &mut budget())
                .map_err(err)?;
            let keyed =
                analysis::prepare_received(&tree, profile, &mut c, &mut budget()).map_err(err)?;
            let sidecar =
                region::sidecar_decode(&packet[1], &keyed, &mut c, &mut budget()).map_err(err)?;
            let input =
                region::prepare(&keyed, sidecar.as_deref(), &mut c, &mut budget()).map_err(err)?;
            let request =
                region::request_decode(&packet[2], profile.registry(), &mut c, &mut budget())
                    .map_err(err)?;
            assert_eq!(input.key(), request.key);
            let native = regions(
                &input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert_eq!(native.outcome, reply.outcome);
            assert_eq!(native.sources, reply.sources);
            assert_eq!(
                region::reply_decode(&packet[3], &request, &input, &mut c, &mut budget())
                    .map_err(err)?,
                reply
            );
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn region_recovery_utf8_end_stale_and_stops_do_not_invent_children() -> Result<(), String> {
    let compiled = formal()?;
    super::super::binding::with_completed_input(
        &compiled,
        "lambda あ あ",
        |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let keyed = analysis::prepare(
                "region",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let input = region::prepare(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                .map_err(err)?;
            let source = parsed.tree().bundle.sources[0].reference();
            for (offset, selected) in [(7, true), (10, false), (11, true), (14, false)] {
                let r = RegionRequest {
                    key: input.key(),
                    source: source.clone(),
                    offset,
                };
                let reply = regions(&input, &r, &mut budget(), &mut SourceAdmission::default());
                let RegionOutcome::Complete {
                    selection,
                    regions: out,
                } = reply.outcome
                else {
                    return Err(err(reply));
                };
                // Whitespace may select the enclosing form but never the preceding name.
                if let Some(i) = selection {
                    assert_eq!(
                        out[i as usize].span.end() - out[i as usize].span.start() == 3,
                        selected
                    );
                } else {
                    assert!(!selected);
                }
            }
            for offset in [8, 9, 12, 15] {
                let r = RegionRequest {
                    key: input.key(),
                    source: source.clone(),
                    offset,
                };
                assert!(matches!(
                    regions(&input, &r, &mut budget(), &mut SourceAdmission::default()).outcome,
                    RegionOutcome::Invalid(RegionError::Source(_))
                ));
            }
            let r = RegionRequest {
                key: input.key(),
                source: source.clone(),
                offset: 7,
            };
            for reason in [
                StopReason::SourceLimit,
                StopReason::WorkLimit,
                StopReason::DepthLimit,
                StopReason::NodeLimit,
                StopReason::AllocationLimit,
            ] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::SourceLimit => limits.source_bytes = 0,
                    StopReason::WorkLimit => limits.work = 0,
                    StopReason::DepthLimit => limits.depth = 0,
                    StopReason::NodeLimit => limits.nodes = 0,
                    StopReason::AllocationLimit => limits.allocation_units = 0,
                    _ => return Err("resource".into()),
                };
                let mut b = Budget::new(limits);
                let result = regions(&input, &r, &mut b, &mut SourceAdmission::default());
                assert_eq!(result.outcome, RegionOutcome::Stopped(reason));
                assert!(result.sources.is_empty());
                assert_eq!(b.poll(), Err(reason));
            }
            let mut stale = r.clone();
            stale.source.revision += 1;
            assert!(matches!(
                regions(
                    &input,
                    &stale,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                RegionOutcome::Invalid(_)
            ));
            stale = r.clone();
            stale.key.reader_facts_digest = Digest::of(b"other");
            assert!(matches!(
                regions(
                    &input,
                    &stale,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                RegionOutcome::Invalid(_)
            ));
            let mut cancelled = budget();
            cancelled.cancel();
            assert_eq!(
                regions(&input, &r, &mut cancelled, &mut SourceAdmission::default()).outcome,
                RegionOutcome::Stopped(StopReason::Cancelled)
            );
            let source = SourceSnapshot::new(
                SourceId("unknown".into()),
                0,
                "memory:unknown".into(),
                b"apply ? x".to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let ParseCompletion::Break(reply) = super::super::binding::parse_any_with_aux(
                &source,
                &[],
                &[],
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?
            else {
                return Err("unknown head completed".into());
            };
            let ParseOutcome::Recovered { tree, facts, .. } = &reply.outcome else {
                return Err(err(&reply.outcome));
            };
            let keyed = analysis::prepare(
                "recovery",
                tree,
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let input = region::prepare(&keyed, Some(facts), &mut c, &mut budget()).map_err(err)?;
            let result = regions(
                &input,
                &RegionRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: 6,
                },
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let RegionOutcome::Complete {
                selection: Some(i),
                regions: out,
            } = &result.outcome
            else {
                return Err(err(&result));
            };
            assert_eq!(out[*i as usize].target.part, RegionPart::Recovery);
            assert_eq!(
                (out[*i as usize].span.start(), out[*i as usize].span.end()),
                (6, 9)
            );
            Ok(())
        },
    )
}
