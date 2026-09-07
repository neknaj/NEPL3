use super::*;
use nepl3_engine::{
    analysis::{
        BindingOptions,
        query::{QueryKind, QueryOutcome, ReferenceOptions},
        region::{query::*, *},
    },
    portable::{analysis, region},
};
use nepl3_wire::foundation::FoundationCodec;
fn err(v: impl std::fmt::Debug) -> String {
    format!("{v:?}")
}
#[test]
fn region_query_preserves_multiple_mapping_candidates_and_atomic_failures() -> Result<(), String> {
    use nepl3_core::{
        budget::StopReason,
        origin::{Mapping, MappingKind},
        view::{FallbackRole, PresentationClass},
    };
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    let compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.query-map",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    super::super::binding::with_completed_input(&compiled, "lambda x x", |_, profile, _, _| {
        let source = SourceSnapshot::new(
            SourceId("query-map-input".into()),
            0,
            "memory:query-map-input".into(),
            b"lambda x x".to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        let display = SourceSnapshot::new(
            SourceId("display".into()),
            0,
            "memory:display".into(),
            b"x".to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        for transformed in [false, true] {
            let ranges = if transformed {
                vec![(7, 10)]
            } else {
                vec![(7, 8), (9, 10)]
            };
            let mappings = ranges
                .iter()
                .map(|(start, end)| {
                    Ok(Mapping {
                        source: source.span(*start, *end).map_err(err)?,
                        target: display.span(0, 1).map_err(err)?,
                        kind: if transformed {
                            MappingKind::Transformed
                        } else {
                            MappingKind::Exact
                        },
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let fact = nepl3_reader::model::ReaderFact::Presentation {
                class: PresentationClass {
                    schema: profile
                        .registry()
                        .selected("nepl3.foundation", 1)
                        .ok_or("foundation")?
                        .clone(),
                    name: "linked".into(),
                    fallback: FallbackRole::Content,
                },
                span: display.span(0, 1).map_err(err)?,
            };
            let nepl3_engine::parse::ParseCompletion::Continue(parsed) =
                super::super::binding::parse_with_artifacts(
                    &source,
                    core::slice::from_ref(&display),
                    &mappings,
                    &[fact],
                    profile,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                )?
            else {
                return Err("parse".into());
            };
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = analysis::prepare(
                "region",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let input = region::prepare(
                &prepared,
                Some(parsed.reader_facts()),
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let request = RegionQueryRequest {
                region: RegionRequest {
                    key: input.key(),
                    source: display.reference(),
                    offset: 0,
                },
                kind: QueryKind::Definition,
            };
            let reply = query(
                &input,
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let RegionQueryOutcome::Complete {
                region: Some(region),
                queries,
            } = &reply.outcome
            else {
                return Err(err(&reply));
            };
            assert!(matches!(
                region.target.part,
                RegionPart::Presentation { .. }
            ));
            let mut ranges = Vec::new();
            for q in queries {
                let QueryOutcome::Definition {
                    selection: Some(s),
                    targets,
                } = q
                else {
                    return Err(err(q));
                };
                ranges.push((s.span.start(), s.span.end()));
                assert_eq!(targets.len(), 1);
            }
            // The display byte explicitly corresponds to both original Name occurrences.
            assert_eq!(ranges, vec![(7, 8), (9, 10)]);
            for include_definitions in [false, true] {
                let refs = RegionQueryRequest {
                    region: request.region.clone(),
                    kind: QueryKind::References(ReferenceOptions {
                        include_definitions,
                        ..Default::default()
                    }),
                };
                let result = query(
                    &input,
                    &bound,
                    &refs,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let RegionQueryOutcome::Complete { queries, .. } = &result.outcome else {
                    return Err(err(&result));
                };
                assert_eq!(queries.len(), 2);
                for q in queries {
                    let QueryOutcome::References { groups, .. } = q else {
                        return Err(err(q));
                    };
                    assert_eq!(groups.len(), 1);
                    let ranges: Vec<_> = groups[0]
                        .locations
                        .iter()
                        .map(|v| (v.span.start(), v.span.end()))
                        .collect();
                    assert_eq!(
                        ranges,
                        if include_definitions {
                            vec![(7, 8), (9, 10)]
                        } else {
                            vec![(9, 10)]
                        }
                    );
                }
                let value = region::query::reply_to_value(
                    &result,
                    &refs,
                    &input,
                    &bound,
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
                let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                assert_eq!(
                    region::query::reply_decode(
                        &value,
                        &refs,
                        &input,
                        &bound,
                        &mut c,
                        &mut budget()
                    )
                    .map_err(err)?,
                    result
                );
            }

            let value = region::query::reply_to_value(
                &reply,
                &request,
                &input,
                &bound,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            assert_eq!(
                region::query::reply_decode(
                    &value,
                    &request,
                    &input,
                    &bound,
                    &mut c,
                    &mut budget()
                )
                .map_err(err)?,
                reply
            );
            for reason in [
                StopReason::WorkLimit,
                StopReason::SourceLimit,
                StopReason::NodeLimit,
                StopReason::DepthLimit,
                StopReason::AllocationLimit,
                StopReason::Cancelled,
            ] {
                let mut limits = budget().limits();
                match reason {
                    StopReason::WorkLimit => limits.work = 0,
                    StopReason::SourceLimit => limits.source_bytes = 0,
                    StopReason::NodeLimit => limits.nodes = 0,
                    StopReason::DepthLimit => limits.depth = 0,
                    StopReason::AllocationLimit => limits.allocation_units = 0,
                    _ => {}
                }
                let mut b = nepl3_core::budget::Budget::new(limits);
                if reason == StopReason::Cancelled {
                    b.cancel();
                }
                let failed = query(
                    &input,
                    &bound,
                    &request,
                    &mut b,
                    &mut SourceAdmission::default(),
                );
                assert_eq!(failed.outcome, RegionQueryOutcome::Stopped(reason));
                assert!(failed.sources.is_empty());
                assert_eq!(b.poll(), Err(reason));
            }
            let mut stale = request.clone();
            stale.region.key.reader_facts_digest.0[0] ^= 1;
            let failed = query(
                &input,
                &bound,
                &stale,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(failed.outcome, RegionQueryOutcome::Invalid(_)));
            assert!(failed.sources.is_empty());
        }
        Ok(())
    })
}
#[test]
fn regions_drive_final_definition_and_references_without_name_lookup() -> Result<(), String> {
    let doc = document(include_bytes!(
        "../../../../conformance/fixtures/grammar/binding/execution.json"
    ))?;
    let mut compiled = nepl3_tools::bootstrap::catalog::compile(
        &doc,
        "test.region.query",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    compiled
        .package
        .forms
        .iter_mut()
        .find(|f| f.spelling == "guest")
        .ok_or("guest")?
        .selection_rules
        .push(nepl3_engine::package::SelectionRule {
            selector: nepl3_engine::package::StyleSelector::Field("value".into()),
            priority: 100,
        });
    // Expectations are original byte positions: Let init is not in the binding's scope.
    type Ranges = &'static [(u64, u64)];
    let cases: [(&str, u64, Ranges); 7] = [
        ("let x x apply x x", 6, &[]),
        ("let x x apply x x", 14, &[(4, 5)]),
        ("lambda x lambda y x", 18, &[(7, 8)]),
        ("lambda x guest x", 15, &[]),
        ("lambda あ apply あ あ", 17, &[(7, 10)]),
        ("twice x x x", 10, &[(6, 7), (8, 9)]),
        ("lettext \"\\u{78}\" x", 17, &[(8, 16)]),
    ];
    for (text, offset, expected) in cases {
        super::super::binding::with_completed_input(&compiled, text, |parsed, profile, _, _| {
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
            let bound = keyed
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let input = region::prepare(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                .map_err(err)?;
            let source = parsed
                .tree()
                .bundle
                .sources
                .iter()
                .find(|s| s.text() == text)
                .ok_or("source")?
                .reference();
            let request = RegionQueryRequest {
                region: RegionRequest {
                    key: input.key(),
                    source,
                    offset,
                },
                kind: QueryKind::Definition,
            };
            let reply = query(
                &input,
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let RegionQueryOutcome::Complete {
                region: Some(_),
                queries,
            } = &reply.outcome
            else {
                return Err(err(&reply));
            };
            if text == "lambda x guest x" {
                let RegionQueryOutcome::Complete {
                    region: Some(region),
                    ..
                } = &reply.outcome
                else {
                    return Err("region".into());
                };
                assert!(matches!(region.target.part, RegionPart::Field { .. }));
                assert_eq!(region.priority, 100);
            }
            assert_eq!(queries.len(), 1, "{text}: {queries:?}");
            let QueryOutcome::Definition {
                selection: Some(_),
                targets,
            } = &queries[0]
            else {
                return Err(err(queries));
            };
            let mut ranges: Vec<_> = targets
                .iter()
                .filter_map(|t| t.location.as_ref()?.selection.as_ref())
                .map(|s| (s.start(), s.end()))
                .collect();
            // These expectations are a set of source positions. Resolution
            // candidate order is retained verbatim and compared by wire equality below.
            ranges.sort();
            assert_eq!(ranges, expected, "{text}");
            let packet = nepl3_core::value::NdfValue::List(vec![
                analysis::request_to_value(&keyed, &mut c, &mut budget()).map_err(err)?,
                region::sidecar_value(&keyed, Some(parsed.reader_facts()), &mut c, &mut budget())
                    .map_err(err)?,
                region::query::request_to_value(
                    &request,
                    profile.registry(),
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?,
                region::query::reply_to_value(
                    &reply,
                    &request,
                    &input,
                    &bound,
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?,
            ]);
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(err)?;
            let nepl3_core::value::NdfValue::List(ref packet) =
                nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?
            else {
                return Err("packet".into());
            };
            let receiver_empty = SourceStore::default();
            let mut receiver_admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &receiver_empty, &mut receiver_admission)
                    .map_err(err)?;
            let received =
                analysis::request_decode(&packet[0], profile, &mut receiver, &mut budget())
                    .map_err(err)?;
            let received =
                analysis::prepare_received(&received, profile, &mut receiver, &mut budget())
                    .map_err(err)?;
            let receiver_bound = received
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let facts = region::sidecar_decode(&packet[1], &received, &mut receiver, &mut budget())
                .map_err(err)?;
            let receiver_input =
                region::prepare(&received, facts.as_deref(), &mut receiver, &mut budget())
                    .map_err(err)?;
            let request_wire = region::query::request_decode(
                &packet[2],
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(request_wire, request);
            let native = query(
                &receiver_input,
                &receiver_bound,
                &request_wire,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert_eq!(native.outcome, reply.outcome);
            assert_eq!(native.sources, reply.sources);
            assert_eq!(
                region::query::reply_decode(
                    &packet[3],
                    &request_wire,
                    &receiver_input,
                    &receiver_bound,
                    &mut receiver,
                    &mut budget()
                )
                .map_err(err)?,
                reply
            );
            for mutation in 0..3 {
                let mut value = packet[3].clone();
                let nepl3_core::value::NdfValue::Record(record) = &mut value else {
                    return Err("reply".into());
                };
                if mutation == 0 {
                    record.fields[4] = nepl3_core::value::NdfValue::List(vec![]);
                } else {
                    let nepl3_core::value::NdfValue::Variant(outcome) = &mut record.fields[2]
                    else {
                        return Err("outcome".into());
                    };
                    outcome.fields[if mutation == 1 { 0 } else { 1 }] = if mutation == 1 {
                        nepl3_core::value::NdfValue::None
                    } else {
                        nepl3_core::value::NdfValue::List(vec![])
                    };
                }
                let ty = nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
                    package: "nepl3.engine".into(),
                    revision: 1,
                    name: "RegionQueryReply".into(),
                });
                profile
                    .registry()
                    .validate(&ty, &value, &mut budget())
                    .map_err(err)?;
                let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
                let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
                assert!(
                    region::query::reply_decode(
                        &value,
                        &request_wire,
                        &receiver_input,
                        &receiver_bound,
                        &mut receiver,
                        &mut budget()
                    )
                    .is_err()
                );
            }
            let request = RegionQueryRequest {
                kind: QueryKind::References(ReferenceOptions::default()),
                ..request
            };
            let reply = query(
                &input,
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(
                matches!(reply.outcome, RegionQueryOutcome::Complete { .. }),
                "{reply:?}"
            );
            Ok(())
        })?;
    }
    Ok(())
}
