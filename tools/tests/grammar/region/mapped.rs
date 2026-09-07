use super::*;
use nepl3_core::{
    origin::{Mapping, MappingKind},
    value::NdfValue,
    view::{Trivia, TriviaKind},
};
use nepl3_engine::{
    analysis::{BindingOptions, region::*},
    parse::ParseCompletion,
    portable::{analysis, region},
};
use nepl3_reader::model::ReaderFact;
use nepl3_wire::foundation::FoundationCodec;
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
#[test]
fn foreign_maps_stay_owner_local_and_compose_caller_depth() -> Result<(), String> {
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.owner",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    super::super::binding::with_completed_input(
        &compiled,
        "lambda x guest x",
        |parsed, profile, _, _| {
            let mut tree = parsed.tree().clone();
            let host = tree
                .bundle
                .sources
                .iter()
                .find(|s| s.text() == "lambda x guest x")
                .ok_or("input")?
                .clone();
            let aux = SourceSnapshot::new(
                SourceId("guest-map".into()),
                0,
                "memory:guest-map".into(),
                host.text().as_bytes().to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let guest = tree
                .bundle
                .nodes
                .iter_mut()
                .flat_map(|n| n.fields.iter_mut())
                .find_map(|f| match f {
                    nepl3_core::syntax::FieldValue::Foreign(f) => Some(&mut f.bundle),
                    _ => None,
                })
                .ok_or("guest")?;
            guest.sources.push(aux.clone());
            guest.source_maps.push(Mapping {
                source: aux.span(0, 16).map_err(err)?,
                target: host.span(0, 16).map_err(err)?,
                kind: MappingKind::Exact,
            });
            tree.validate(profile, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let binding = analysis::prepare(
                "region",
                &tree,
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let prepared =
                region::prepare(&binding, None, &mut codec, &mut budget()).map_err(err)?;
            // Only the guest's final x occupies this owner. Its map cannot project the host lambda.
            for offset in [0, 15, 16] {
                let request = RegionRequest {
                    key: prepared.key(),
                    source: aux.reference(),
                    offset,
                };
                let reply = regions(
                    &prepared,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let RegionOutcome::Complete {
                    selection,
                    regions: out,
                } = &reply.outcome
                else {
                    return Err(err(&reply));
                };
                assert!(
                    out.iter()
                        .all(|v| (v.span.start(), v.span.end()) == (15, 16))
                );
                assert_eq!(selection.is_some(), offset == 15);
                let value =
                    region::reply_to_value(&reply, &request, &prepared, &mut codec, &mut budget())
                        .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
                let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                assert_eq!(
                    region::reply_decode(&value, &request, &prepared, &mut codec, &mut budget())
                        .map_err(err)?,
                    reply
                );
            }
            let request = RegionRequest {
                key: prepared.key(),
                source: aux.reference(),
                offset: 15,
            };
            let mut base = budget();
            assert!(matches!(
                regions(
                    &prepared,
                    &request,
                    &mut base,
                    &mut SourceAdmission::default()
                )
                .outcome,
                RegionOutcome::Complete { .. }
            ));
            let mut nested = budget();
            nested
                .with_depth_at_least(7, |b| -> Result<(), nepl3_core::budget::StopReason> {
                    assert!(matches!(
                        regions(&prepared, &request, b, &mut SourceAdmission::default()).outcome,
                        RegionOutcome::Complete { .. }
                    ));
                    Ok(())
                })
                .map_err(err)?;
            assert_eq!(nested.usage().depth, base.usage().depth + 7);
            assert_eq!(nested.current_depth(), 0);
            Ok(())
        },
    )
}
#[test]
fn formally_accepted_mapped_capture_and_trivia_republication_survive_wire() -> Result<(), String> {
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.mapped",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    super::super::binding::with_completed_input(&compiled, "lambda x x", |_, profile, _, _| {
        let source = SourceSnapshot::new(
            SourceId("region-input".into()),
            0,
            "memory:region-input".into(),
            b"lambda x x".to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        let derived = SourceSnapshot::new(
            SourceId("captured".into()),
            0,
            "memory:captured".into(),
            b"lambda".to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        for (split, transformed) in [(false, false), (true, false), (false, true)] {
            let ranges: Vec<_> = if split {
                (0..6).map(|i| (i, i + 1)).collect()
            } else {
                vec![(0, 6)]
            };
            let maps = ranges
                .iter()
                .map(|(start, end)| {
                    Ok(Mapping {
                        source: source.span(*start, *end).map_err(err)?,
                        target: derived
                            .span(*start, if transformed { 1 } else { *end })
                            .map_err(err)?,
                        kind: if transformed {
                            MappingKind::Transformed
                        } else {
                            MappingKind::Exact
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let fact = ReaderFact::Capture {
                name: "mapped".into(),
                span: derived
                    .span(0, if transformed { 1 } else { 6 })
                    .map_err(err)?,
            };
            let presentation = ReaderFact::Presentation {
                class: nepl3_core::view::PresentationClass {
                    schema: profile
                        .registry()
                        .selected("nepl3.foundation", 1)
                        .ok_or("foundation")?
                        .clone(),
                    name: "body".into(),
                    fallback: nepl3_core::view::FallbackRole::Content,
                },
                span: source.span(9, 10).map_err(err)?,
            };
            let ParseCompletion::Continue(parsed) = super::super::binding::parse_with_artifacts(
                &source,
                core::slice::from_ref(&derived),
                &maps,
                &[fact, presentation],
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?
            else {
                return Err("mapped capture did not parse".into());
            };
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
            let request = RegionRequest {
                key: input.key(),
                source: source.reference(),
                offset: 2,
            };
            let reply = regions(
                &input,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let RegionOutcome::Complete {
                selection: Some(index),
                regions: out,
            } = &reply.outcome
            else {
                return Err(err(&reply));
            };
            assert!(matches!(
                out[*index as usize].target.part,
                RegionPart::Capture { .. }
            ));
            assert_eq!(
                out[*index as usize].mapping,
                if transformed {
                    RegionMapping::Transformed
                } else {
                    RegionMapping::Exact
                }
            );
            assert!(
                out.iter()
                    .any(|v| matches!(v.target.part, RegionPart::Presentation { .. })
                        && v.span.start() == 9
                        && v.span.end() == 10)
            );
            assert_eq!(
                (
                    out[*index as usize].span.start(),
                    out[*index as usize].span.end()
                ),
                (0, 6)
            );
            assert_eq!(
                out[*index as usize].logical_span.snapshot_ref(),
                derived.identity()
            );
            let sidecar =
                region::sidecar_value(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&sidecar, &mut budget()).map_err(err)?;
            let sidecar = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let received =
                region::sidecar_decode(&sidecar, &keyed, &mut c, &mut budget()).map_err(err)?;
            assert_eq!(received.as_deref(), Some(parsed.reader_facts()));
            let mut forged = parsed.reader_facts().to_vec();
            let batch = forged
                .iter_mut()
                .find(|v| !v.trivia.is_empty())
                .ok_or("leading trivia")?;
            batch.trivia = vec![Trivia {
                span: source.span(0, 6).map_err(err)?,
                kind: TriviaKind::Skipped,
            }];
            batch.facts.push(ReaderFact::Capture {
                name: "forged".into(),
                span: source.span(0, 6).map_err(err)?,
            });
            assert!(region::prepare(&keyed, Some(&forged), &mut c, &mut budget()).is_err());
            let mut duplicate = parsed.reader_facts().to_vec();
            duplicate.push(duplicate[0].clone());
            assert!(region::prepare(&keyed, Some(&duplicate), &mut c, &mut budget()).is_err());
            let mut wrong_owner = parsed.reader_facts().to_vec();
            wrong_owner[0].node = Some(nepl3_core::syntax::NodeRef(u64::MAX));
            assert!(region::prepare(&keyed, Some(&wrong_owner), &mut c, &mut budget()).is_err());
            let mut wrong_source = parsed.reader_facts().to_vec();
            let other = SourceSnapshot::new(
                SourceId("captured".into()),
                1,
                "memory:captured".into(),
                b"lambda".to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            wrong_source[0].facts[0] = ReaderFact::Capture {
                name: "mapped".into(),
                span: other.span(0, 6).map_err(err)?,
            };
            assert!(region::prepare(&keyed, Some(&wrong_source), &mut c, &mut budget()).is_err());
            // Schema-valid wire changes cannot forge owner, rank, selection, or
            // replace a missing declared source using the ambient store.
            let packet = region::reply_to_value(&reply, &request, &input, &mut c, &mut budget())
                .map_err(err)?;
            for mutation in 0..4 {
                let mut value = packet.clone();
                let NdfValue::Record(record) = &mut value else {
                    return Err("record".into());
                };
                if mutation == 0 {
                    record.fields[4] = NdfValue::List(vec![]);
                } else {
                    let NdfValue::Variant(outcome) = &mut record.fields[2] else {
                        return Err("outcome".into());
                    };
                    if mutation == 1 {
                        outcome.fields[0] = NdfValue::None;
                    } else {
                        let NdfValue::List(regions) = &mut outcome.fields[1] else {
                            return Err("regions".into());
                        };
                        let NdfValue::Record(region) = &mut regions[0] else {
                            return Err("region".into());
                        };
                        region.fields[if mutation == 2 { 4 } else { 6 }] = NdfValue::U64(u64::MAX);
                    }
                }
                let ty = TypeDescriptor::Named(TypeRef {
                    package: "nepl3.engine".into(),
                    revision: 1,
                    name: "RegionReply".into(),
                });
                profile
                    .registry()
                    .validate(&ty, &value, &mut budget())
                    .map_err(err)?;
                assert!(
                    region::reply_decode(&value, &request, &input, &mut c, &mut budget()).is_err()
                );
            }
        }
        Ok(())
    })
}
