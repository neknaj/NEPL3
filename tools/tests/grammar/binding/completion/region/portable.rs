use super::*;
use nepl3_engine::portable::{PortableError, region::completion as wire};

#[test]
fn region_candidate_transport_recomputes_selection_and_scope_metadata() -> Result<(), String> {
    let compiled = execution()?;
    for (text, offset, prefix) in [
        ("lambda a lambda b probe", 18, ""),
        ("lambda あ probe", 11, "あ"),
        ("lambda a guest probe", 15, ""),
        ("guest lambda a probe", 15, ""),
        ("lambda a probe", 9, "z"),
        ("lambda a probe", 7, ""),
        ("lambda a probe", 8, ""),
        ("lambda a probe", 14, ""),
    ] {
        with_input(&compiled, text, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "region-codec",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let sender = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let receiver = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let input = wire_region::prepare(
                &prepared,
                if offset == 11 { Some(&[]) } else { None },
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let request = RegionCompletionRequest {
                region: RegionRequest {
                    key: input.key(),
                    source: tree.tree().bundle.sources[0].reference(),
                    offset,
                },
                prefix: prefix.into(),
            };
            let request_value =
                wire::request_to_value(&request, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&request_value, &mut budget()).map_err(err)?;
            let request_value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let received = wire::request_decode(
                &request_value,
                profile.registry(),
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(received.region, request.region);
            assert_eq!(received.prefix, request.prefix);
            let reply = selected::names(
                &input,
                &sender,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let packet =
                wire::reply_to_value(&reply, &request, &input, &sender, &mut codec, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&packet, &mut budget()).map_err(err)?;
            let packet = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut receiving_budget = budget();
            receiving_budget.charge(Resource::Work, 1234).map_err(err)?;
            let decoded = wire::reply_decode(
                &packet,
                &received,
                &input,
                &receiver,
                &mut codec,
                &mut receiving_budget,
            )
            .map_err(err)?;
            assert!(receiving_budget.usage().work > 1234);
            assert_eq!(decoded.report, reply.report);
            assert_eq!(decoded.sources, reply.sources);
            let (
                RegionCompletionOutcome::Complete {
                    region: old_region,
                    groups: old,
                },
                RegionCompletionOutcome::Complete {
                    region: new_region,
                    groups: new,
                },
            ) = (&reply.outcome, &decoded.outcome)
            else {
                return Err("complete".into());
            };
            assert_eq!(old_region, new_region);
            assert_eq!(old.len(), new.len());
            for (old, new) in old.iter().zip(new) {
                assert_eq!(old.occurrence, new.occurrence);
                assert_eq!(old.namespace, new.namespace);
                assert_eq!(old.report, new.report);
                assert_eq!(
                    old.candidates
                        .iter()
                        .map(|c| (&c.name, &c.resolution))
                        .collect::<Vec<_>>(),
                    new.candidates
                        .iter()
                        .map(|c| (&c.name, &c.resolution))
                        .collect::<Vec<_>>()
                );
            }
            for mode in 0..6 {
                let mut forged = packet.clone();
                let NdfValue::Record(record) = &mut forged else {
                    return Err("record".into());
                };
                match mode {
                    0 => record.schema.digest = Digest::of(b"wrong region schema"),
                    1 => record.fields[5] = NdfValue::List(Vec::new()),
                    2 => {
                        let NdfValue::List(groups) = &mut record.fields[3] else {
                            return Err("groups".into());
                        };
                        if groups.is_empty() {
                            continue;
                        }
                        groups.clear();
                    }
                    3 => {
                        let NdfValue::List(groups) = &mut record.fields[3] else {
                            return Err("groups".into());
                        };
                        if groups.is_empty() {
                            continue;
                        }
                        groups.push(groups[0].clone());
                    }
                    4 | 5 => {
                        let NdfValue::List(groups) = &mut record.fields[3] else {
                            return Err("groups".into());
                        };
                        let Some(NdfValue::Record(group)) = groups.first_mut() else {
                            continue;
                        };
                        group.fields[if mode == 4 { 1 } else { 2 }] = NdfValue::U64(u64::MAX);
                    }
                    _ => unreachable!(),
                }
                if mode == 1 && reply.sources.is_empty() {
                    continue;
                }
                assert!(
                    wire::reply_decode(
                        &forged,
                        &request,
                        &input,
                        &receiver,
                        &mut codec,
                        &mut budget()
                    )
                    .is_err(),
                    "mode {mode} text {text}"
                );
            }
            for resource in 0..5 {
                let mut limits = budget().limits();
                match resource {
                    0 => limits.work = 0,
                    1 => limits.nodes = 0,
                    2 => limits.allocation_units = 0,
                    3 => limits.depth = 0,
                    _ => {}
                }
                let mut b = nepl3_core::budget::Budget::new(limits);
                if resource == 4 {
                    b.cancel();
                }
                assert!(matches!(
                    wire::reply_decode(&packet, &request, &input, &receiver, &mut codec, &mut b),
                    Err(PortableError::Stopped(_))
                ));
                let mut b = nepl3_core::budget::Budget::new(limits);
                if resource == 4 {
                    b.cancel();
                }
                assert!(matches!(
                    wire::reply_to_value(&reply, &request, &input, &sender, &mut codec, &mut b),
                    Err(PortableError::Stopped(_))
                ));
            }
            let mut bad_report = selected::names(
                &input,
                &sender,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            bad_report.report.trace_overflow =
                Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
            assert!(matches!(
                wire::reply_to_value(
                    &bad_report,
                    &request,
                    &input,
                    &sender,
                    &mut codec,
                    &mut budget()
                ),
                Err(PortableError::Shape)
            ));
            let mut bad_group = selected::names(
                &input,
                &sender,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            if let RegionCompletionOutcome::Complete { groups, .. } = &mut bad_group.outcome
                && let Some(group) = groups.first_mut()
            {
                group.report.trace_overflow =
                    Some(nepl3_core::diagnostic::TraceOverflow { dropped: 1 });
                assert!(matches!(
                    wire::reply_to_value(
                        &bad_group,
                        &request,
                        &input,
                        &sender,
                        &mut codec,
                        &mut budget()
                    ),
                    Err(PortableError::Shape)
                ));
            }
            let mut wrong = received;
            wrong.region.key.reader_facts_digest = Digest::of(b"wrong reader facts");
            assert!(
                wire::reply_decode(
                    &packet,
                    &wrong,
                    &input,
                    &receiver,
                    &mut codec,
                    &mut budget()
                )
                .is_err()
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn region_candidate_transport_preserves_mapped_groups_and_source_admission() -> Result<(), String> {
    let compiled = custom::compiled()?;
    for (emitted, transformed) in [(false, false), (true, false), (false, true)] {
        with_input(
            &compiled,
            "custom x guest custom x x",
            |tree, profile, _, _| {
                let empty = SourceStore::default();
                let mut a = SourceAdmission::default();
                let mut codec =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                let prepared = keyed::prepare(
                    "mapped-codec",
                    tree.tree(),
                    BindingOptions,
                    budget().limits(),
                    profile,
                    &mut codec,
                    &mut budget(),
                )
                .map_err(err)?;
                let mut host = MappedReferences {
                    transformed,
                    inner: Box::new(custom::query_map_host(emitted, false)),
                };
                let bound = prepared
                    .execute_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
                    .map_err(err)?;
                let input = wire_region::prepare(&prepared, None, &mut codec, &mut budget())
                    .map_err(err)?;
                for offset in [7, 22] {
                    let request = RegionCompletionRequest {
                        region: RegionRequest {
                            key: input.key(),
                            source: tree.tree().bundle.sources[0].reference(),
                            offset,
                        },
                        prefix: "".into(),
                    };
                    let reply = selected::names(
                        &input,
                        &bound,
                        &request,
                        &mut budget(),
                        &mut SourceAdmission::default(),
                    );
                    let packet = wire::reply_to_value(
                        &reply,
                        &request,
                        &input,
                        &bound,
                        &mut codec,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    let decoded = wire::reply_decode(
                        &packet,
                        &request,
                        &input,
                        &bound,
                        &mut codec,
                        &mut budget(),
                    )
                    .map_err(err)?;
                    let RegionCompletionOutcome::Complete { groups, .. } = decoded.outcome else {
                        return Err("complete".into());
                    };
                    assert_eq!(groups.len(), 2);
                    let mut forged = packet.clone();
                    let NdfValue::Record(record) = &mut forged else {
                        return Err("record".into());
                    };
                    let NdfValue::List(groups) = &mut record.fields[3] else {
                        return Err("groups".into());
                    };
                    groups.reverse();
                    assert!(matches!(
                        wire::reply_decode(
                            &forged,
                            &request,
                            &input,
                            &bound,
                            &mut codec,
                            &mut budget()
                        ),
                        Err(PortableError::RequestMismatch)
                    ));
                    // A fresh operation must admit the generated binding closure,
                    // even though binding itself already executed successfully.
                    let mut fresh = SourceAdmission::default();
                    let mut fresh_codec =
                        FoundationCodec::new(profile.registry(), &empty, &mut fresh)
                            .map_err(err)?;
                    let mut limits = budget().limits();
                    limits.source_bytes = tree
                        .tree()
                        .bundle
                        .sources
                        .iter()
                        .map(|s| s.text().len() as u64)
                        .sum();
                    assert!(matches!(
                        wire::reply_decode(
                            &packet,
                            &request,
                            &input,
                            &bound,
                            &mut fresh_codec,
                            &mut Budget::new(limits)
                        ),
                        Err(PortableError::Stopped(StopReason::SourceLimit))
                    ));
                    let mut encode_a = SourceAdmission::default();
                    let mut encode_c =
                        FoundationCodec::new(profile.registry(), &empty, &mut encode_a)
                            .map_err(err)?;
                    assert!(matches!(
                        wire::reply_to_value(
                            &reply,
                            &request,
                            &input,
                            &bound,
                            &mut encode_c,
                            &mut Budget::new(limits)
                        ),
                        Err(PortableError::Stopped(StopReason::SourceLimit))
                    ));
                    // Unrelated host snapshots must never enter this packet's
                    // closure or inflate its fresh source-byte admission cost.
                    let mut ambient = SourceStore::default();
                    ambient
                        .insert(
                            SourceSnapshot::new(
                                nepl3_core::source::SourceId("ambient-only".into()),
                                1,
                                "private:ambient".into(),
                                vec![b'x'; 2000],
                                &mut budget(),
                            )
                            .map_err(err)?,
                        )
                        .map_err(err)?;
                    let mut clean_a = SourceAdmission::default();
                    let mut clean_c =
                        FoundationCodec::new(profile.registry(), &empty, &mut clean_a)
                            .map_err(err)?;
                    let mut clean_b = budget();
                    wire::reply_decode(
                        &packet,
                        &request,
                        &input,
                        &bound,
                        &mut clean_c,
                        &mut clean_b,
                    )
                    .map_err(err)?;
                    let source_bytes = clean_b.usage().source_bytes;
                    let mut ambient_a = SourceAdmission::default();
                    let mut ambient_c =
                        FoundationCodec::new(profile.registry(), &ambient, &mut ambient_a)
                            .map_err(err)?;
                    let mut limits = budget().limits();
                    limits.source_bytes = source_bytes;
                    let mut ambient_b = Budget::new(limits);
                    wire::reply_decode(
                        &packet,
                        &request,
                        &input,
                        &bound,
                        &mut ambient_c,
                        &mut ambient_b,
                    )
                    .map_err(err)?;
                    assert_eq!(ambient_b.usage().source_bytes, source_bytes);
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}
