use super::*;
use nepl3_engine::{
    analysis::{BindingAccessError, BindingOptions, query::*},
    portable::{analysis as keyed, query as wire_query},
};
fn transport(
    reply: &QueryReply,
    request: &QueryRequest,
    registry: &nepl3_core::schema::SchemaRegistry,
) -> Result<NdfValue, String> {
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(registry, &empty, &mut a).map_err(err)?;
    let value =
        wire_query::reply_to_value(reply, request, registry, &mut c, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(registry, &empty, &mut a).map_err(err)?;
    let decoded =
        wire_query::reply_decode(&value, request, registry, &mut c, &mut budget()).map_err(err)?;
    assert_eq!(&decoded, reply);
    Ok(value)
}

#[test]
fn keyed_definition_uses_original_ranges_and_all_identity_candidates() -> Result<(), String> {
    // These are byte positions in the original inputs, independent of allocated IDs.
    type TargetRanges = (u64, u64, u64, u64);
    let cases: [(&str, u64, &[TargetRanges]); 9] = [
        ("let x x x", 6, &[]),
        ("let x x x", 8, &[(4, 5, 0, 9)]),
        ("lambda x apply lambda x x x", 24, &[(22, 23, 15, 25)]),
        ("lambda x apply lambda x x x", 26, &[(7, 8, 0, 27)]),
        ("apply lambda x x lambda x x", 15, &[(13, 14, 6, 16)]),
        ("lambda あ apply あ あ", 17, &[(7, 10, 0, 24)]),
        ("lambda x guest lambda x x", 24, &[(22, 23, 15, 25)]),
        ("twice x x x", 10, &[(6, 7, 0, 11), (8, 9, 0, 11)]),
        ("lettext \"\\u{78}\" x", 17, &[(8, 16, 0, 18)]),
    ];
    let compiled = execution()?;
    for (input, offset, expected) in cases {
        with_input(&compiled, input, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "query",
                tree.tree(),
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
            let source = tree
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == input)
                .ok_or("source")?
                .reference();
            let request = QueryRequest {
                key: prepared.key(),
                source,
                offset,
                kind: QueryKind::Definition,
            };
            let reply = query(
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            transport(&reply, &request, profile.registry())?;
            let encoded =
                wire_query::request_to_value(&request, profile.registry(), &mut c, &mut budget())
                    .map_err(err)?;
            let bytes = nepl3_wire::encode(&encoded, &mut budget()).map_err(err)?;
            let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut fresh = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let received = wire_query::request_decode(
                &received,
                profile.registry(),
                &mut receiver,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(received, request);
            assert_eq!(
                query(
                    &bound,
                    &received,
                    &mut budget(),
                    &mut SourceAdmission::default()
                ),
                reply
            );
            let QueryOutcome::Definition { selection, targets } = &reply.outcome else {
                return Err(format!("{input}: {:?}", reply.outcome));
            };
            assert_eq!(selection.as_ref().map(|v| v.span.start()), Some(offset));
            if let Some(QuerySelection {
                resolution: ReferenceResolution::Ambiguous(ids),
                ..
            }) = selection
            {
                assert_eq!(targets.iter().map(|v| v.entity).collect::<Vec<_>>(), *ids);
            }
            let mut actual = targets
                .iter()
                .map(|v| {
                    let l = v.location.as_ref().ok_or("location")?;
                    let s = l.selection.as_ref().ok_or("selection")?;
                    Ok((s.start(), s.end(), l.range.start(), l.range.end()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            // Expected ambiguity is a set of original target locations; the API
            // preserves the resolver's explicit candidate order.
            actual.sort();
            assert_eq!(actual, expected, "{input}");
            if expected.is_empty() {
                assert!(matches!(
                    selection.as_ref().map(|v| &v.resolution),
                    Some(ReferenceResolution::Unresolved(_))
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[test]
fn keyed_references_boundary_options_and_sticky_stops() -> Result<(), String> {
    let compiled = execution()?;
    with_input(
        &compiled,
        "lambda あ apply あ あ",
        |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "query",
                tree.tree(),
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
            let source = tree
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == "lambda あ apply あ あ")
                .ok_or("source")?
                .reference();
            let mut request = QueryRequest {
                key: prepared.key(),
                source,
                offset: 17,
                kind: QueryKind::References(ReferenceOptions::default()),
            };
            for include_definitions in [false, true] {
                request.kind = QueryKind::References(ReferenceOptions {
                    include_definitions,
                    ..Default::default()
                });
                let reply = query(
                    &bound,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                transport(&reply, &request, profile.registry())?;
                let QueryOutcome::References { groups, .. } = reply.outcome else {
                    return Err("references".into());
                };
                assert_eq!(groups.len(), 1);
                let spans = groups[0]
                    .locations
                    .iter()
                    .map(|v| (v.span.start(), v.span.end()))
                    .collect::<Vec<_>>();
                assert_eq!(
                    spans,
                    if include_definitions {
                        vec![(7, 10), (17, 20), (21, 24)]
                    } else {
                        vec![(17, 20), (21, 24)]
                    }
                );
            }
            request.offset = 18;
            assert!(matches!(
                query(
                    &bound,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                QueryOutcome::Invalid(QueryError::Source(
                    nepl3_core::source::SourceError::ScalarBoundary
                ))
            ));
            for offset in [0, 20, 24] {
                request.offset = offset;
                assert!(matches!(
                    query(
                        &bound,
                        &request,
                        &mut budget(),
                        &mut SourceAdmission::default()
                    )
                    .outcome,
                    QueryOutcome::References {
                        selection: None,
                        ..
                    }
                ));
            }
            request.offset = 25;
            assert!(matches!(
                query(
                    &bound,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                QueryOutcome::Invalid(QueryError::Source(nepl3_core::source::SourceError::Bounds))
            ));
            request.offset = 17;
            for n in 0..5 {
                let mut limits = budget().limits();
                let expected = match n {
                    0 => {
                        limits.work = 0;
                        StopReason::WorkLimit
                    }
                    1 => {
                        limits.source_bytes = 0;
                        StopReason::SourceLimit
                    }
                    2 => {
                        limits.nodes = 0;
                        StopReason::NodeLimit
                    }
                    3 => {
                        limits.allocation_units = 0;
                        StopReason::AllocationLimit
                    }
                    _ => StopReason::Cancelled,
                };
                let mut b = Budget::new(limits);
                if n == 4 {
                    b.cancel();
                }
                let reply = query(&bound, &request, &mut b, &mut SourceAdmission::default());
                assert!(matches!(reply.outcome,QueryOutcome::Stopped(r)if r==expected));
                assert_eq!(b.poll(), Err(expected));
                assert_eq!(reply.report.usage, b.usage());
                assert!(reply.sources.is_empty());
                transport(&reply, &request, profile.registry())?;
            }
            request.key.request_digest = nepl3_core::source::Digest::of(b"stale");
            assert!(matches!(
                query(
                    &bound,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default()
                )
                .outcome,
                QueryOutcome::Invalid(QueryError::Access(BindingAccessError::StaleAnalysis))
            ));
            Ok(())
        },
    )
}

#[test]
fn custom_keyed_queries_use_sparse_final_ids_and_keep_absent_locations() -> Result<(), String> {
    let compiled = super::super::custom::compiled()?;
    for source_less in [false, true] {
        with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "custom-query",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let mut host = super::super::custom::query_host(true, source_less);
            let bound = prepared
                .execute_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let mut request = QueryRequest {
                key: prepared.key(),
                source: tree.tree().bundle.sources[0].reference(),
                offset: 6,
                kind: QueryKind::Definition,
            };
            let reply = query(
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let QueryOutcome::Definition {
                selection: Some(selection),
                targets,
            } = &reply.outcome
            else {
                return Err(format!("custom query {:?}", reply.outcome));
            };
            assert_eq!(
                selection.resolution,
                ReferenceResolution::Resolved(EntityId(100))
            );
            assert!(!selection.open_input);
            assert_eq!(targets.len(), 1);
            assert_eq!(targets[0].entity, EntityId(100));
            assert_eq!(targets[0].location.is_none(), source_less);
            transport(&reply, &request, profile.registry())?;
            request.kind = QueryKind::References(ReferenceOptions::default());
            let reply = query(
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let QueryOutcome::References { groups, .. } = &reply.outcome else {
                return Err("custom references".into());
            };
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].entity, EntityId(100));
            assert_eq!(
                groups[0]
                    .locations
                    .iter()
                    .map(|v| v.span.start())
                    .collect::<Vec<_>>(),
                vec![6, 19]
            );
            transport(&reply, &request, profile.registry())?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn query_role_options_ambiguity_and_open_inputs_remain_distinct() -> Result<(), String> {
    let mut compiled = execution()?;
    for input in ["imported define x x x", "twice x x x"] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "roles",
                tree.tree(),
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
            for include in [false, true] {
                let request = QueryRequest {
                    key: prepared.key(),
                    source: tree.tree().bundle.sources[0].reference(),
                    offset: if input.starts_with("twice") { 10 } else { 20 },
                    kind: QueryKind::References(ReferenceOptions {
                        include_definitions: include,
                        include_imports: include,
                        include_exports: include,
                        include_ambiguous: include,
                    }),
                };
                let reply = query(
                    &bound,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let QueryOutcome::References { groups, .. } = &reply.outcome else {
                    return Err(format!("roles {:?}", reply.outcome));
                };
                if input.starts_with("twice") {
                    assert_eq!(groups.len(), 2);
                    for group in groups {
                        assert_eq!(group.locations.len(), if include { 2 } else { 0 });
                        assert_eq!(
                            group.locations.iter().filter(|v| v.ambiguous).count(),
                            usize::from(include)
                        );
                    }
                } else {
                    assert_eq!(groups.len(), 1);
                    // Static Import introduces the child's exported Entity into
                    // visibility; it does not invent a second source occurrence.
                    assert_eq!(groups[0].locations.len(), if include { 2 } else { 1 });
                    if include {
                        assert!(
                            groups[0]
                                .locations
                                .iter()
                                .any(|v| v.role == OccurrenceRole::Export)
                        );
                    }
                }
                transport(&reply, &request, profile.registry())?;
            }
            Ok(())
        })?;
    }
    compiled.package.namespaces[0].policy = nepl3_engine::package::NamespacePolicy::Open;
    with_input(&compiled, "x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "open",
            tree.tree(),
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
        let request = QueryRequest {
            key: prepared.key(),
            source: tree.tree().bundle.sources[0].reference(),
            offset: 0,
            kind: QueryKind::Definition,
        };
        let reply = query(
            &bound,
            &request,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        assert!(
            matches!(&reply.outcome,QueryOutcome::Definition{selection:Some(v),targets}if v.open_input&&matches!(v.resolution,ReferenceResolution::Unresolved(_))&&targets.is_empty())
        );
        transport(&reply, &request, profile.registry())?;
        Ok(())
    })
}

#[test]
fn query_received_geometry_identity_sources_and_stop_mutations_are_rejected() -> Result<(), String>
{
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "mutations",
            tree.tree(),
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
        let request = QueryRequest {
            key: prepared.key(),
            source: tree.tree().bundle.sources[0].reference(),
            offset: 9,
            kind: QueryKind::Definition,
        };
        let reply = query(
            &bound,
            &request,
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        let packet = transport(&reply, &request, profile.registry())?;
        let ty = nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
            package: "nepl3.engine".into(),
            revision: 1,
            name: "QueryReply".into(),
        });
        for mutation in 0..6 {
            let mut changed = packet.clone();
            let NdfValue::Record(r) = &mut changed else {
                return Err("reply".into());
            };
            match mutation {
                0 => {
                    r.fields[3] = NdfValue::List(vec![]);
                }
                1 => {
                    let NdfValue::List(sources) = &mut r.fields[3] else {
                        return Err("sources".into());
                    };
                    sources.push(sources[0].clone());
                }
                2 => {
                    let NdfValue::Record(key) = &mut r.fields[0] else {
                        return Err("key".into());
                    };
                    key.fields[0] = NdfValue::Bytes(vec![0; 32]);
                }
                _ => {
                    let NdfValue::Variant(outcome) = &mut r.fields[1] else {
                        return Err("outcome".into());
                    };
                    if mutation == 5 {
                        let NdfValue::Some(selection) = &mut outcome.fields[0] else {
                            return Err("selection".into());
                        };
                        let NdfValue::Record(selection) = selection.as_mut() else {
                            return Err("selection".into());
                        };
                        selection.fields[3] = NdfValue::Bool(true);
                    } else {
                        let NdfValue::List(targets) = &mut outcome.fields[1] else {
                            return Err("targets".into());
                        };
                        let NdfValue::Record(target) = &mut targets[0] else {
                            return Err("target".into());
                        };
                        if mutation == 3 {
                            let NdfValue::Record(id) = &mut target.fields[0] else {
                                return Err("id".into());
                            };
                            id.fields[0] = NdfValue::U64(999);
                        } else {
                            let NdfValue::Some(location) = &mut target.fields[1] else {
                                return Err("location".into());
                            };
                            let NdfValue::Record(location) = location.as_mut() else {
                                return Err("location".into());
                            };
                            location.fields[0] = NdfValue::Text("wrong-uri".into());
                        }
                    }
                }
            }
            profile
                .registry()
                .validate(&ty, &changed, &mut budget())
                .map_err(err)?;
            // Even an ambient host copy cannot repair a removed declaration.
            let mut ambient = SourceStore::default();
            for source in &reply.sources {
                ambient.insert(source.clone()).map_err(err)?;
            }
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &ambient, &mut a).map_err(err)?;
            assert!(
                wire_query::reply_decode(
                    &changed,
                    &request,
                    profile.registry(),
                    &mut c,
                    &mut budget()
                )
                .is_err(),
                "mutation {mutation}"
            );
            assert_eq!(
                wire_query::reply_decode(
                    &packet,
                    &request,
                    profile.registry(),
                    &mut c,
                    &mut budget()
                )
                .map_err(err)?,
                reply
            );
        }
        for resource in 0..6 {
            let mut limits = budget().limits();
            let reason = match resource {
                0 => {
                    limits.work = 0;
                    StopReason::WorkLimit
                }
                1 => {
                    limits.source_bytes = 0;
                    StopReason::SourceLimit
                }
                2 => {
                    limits.nodes = 0;
                    StopReason::NodeLimit
                }
                3 => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                4 => {
                    limits.depth = 0;
                    StopReason::DepthLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut b = Budget::new(limits);
            if resource == 5 {
                b.cancel();
            }
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            assert!(
                matches!(wire_query::reply_decode(&packet,&request,profile.registry(),&mut c,&mut b),Err(nepl3_engine::portable::PortableError::Stopped(r)) if r==reason)
            );
            assert_eq!(b.poll(), Err(reason));
        }
        let mut invalid = reply.clone();
        invalid.sources.clear();
        invalid.outcome = QueryOutcome::Invalid(QueryError::Source(
            nepl3_core::source::SourceError::Stopped(StopReason::WorkLimit),
        ));
        assert!(
            wire_query::reply_to_value(
                &invalid,
                &request,
                profile.registry(),
                &mut c,
                &mut budget()
            )
            .is_err()
        );
        Ok(())
    })
}

#[test]
fn query_import_option_uses_a_real_provider_occurrence() -> Result<(), String> {
    let compiled = super::super::custom::compiled()?;
    with_input(&compiled, "custom x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "import-query",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut c,
            &mut budget(),
        )
        .map_err(err)?;
        let mut host = super::super::custom::query_host_import();
        let bound = prepared
            .execute_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        for include_imports in [false, true] {
            let request = QueryRequest {
                key: prepared.key(),
                source: tree.tree().bundle.sources[0].reference(),
                offset: 9,
                kind: QueryKind::References(ReferenceOptions {
                    include_imports,
                    ..Default::default()
                }),
            };
            let reply = query(
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            let QueryOutcome::References { groups, .. } = &reply.outcome else {
                return Err(format!("import {:?}", reply.outcome));
            };
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].entity, EntityId(100));
            assert_eq!(
                groups[0]
                    .locations
                    .iter()
                    .map(|v| (v.role, v.span.start(), v.span.end()))
                    .collect::<Vec<_>>(),
                if include_imports {
                    vec![
                        (OccurrenceRole::Import, 7, 8),
                        (OccurrenceRole::Reference, 9, 10),
                    ]
                } else {
                    vec![(OccurrenceRole::Reference, 9, 10)]
                }
            );
            transport(&reply, &request, profile.registry())?;
        }
        Ok(())
    })
}
