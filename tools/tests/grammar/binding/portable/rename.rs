use super::*;
#[path = "rename/wire.rs"]
mod wire;
use nepl3_engine::portable::rename as wire_rename;
use nepl3_engine::{
    analysis::{BindingOptions, rename::*},
    portable::analysis as keyed,
};

fn transport(
    reply: &RenameReply,
    request: &RenameRequest,
    r: &nepl3_core::schema::SchemaRegistry,
) -> Result<(), String> {
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, &empty, &mut a).map_err(err)?;
    let request_value =
        wire_rename::request_to_value(request, r, &mut c, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&request_value, &mut budget()).map_err(err)?;
    let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let mut fresh = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(r, &empty, &mut fresh).map_err(err)?;
    let received =
        wire_rename::request_decode(&value, r, &mut receiver, &mut budget()).map_err(err)?;
    assert_eq!(&received, request);
    let value =
        wire_rename::reply_to_value(reply, &received, r, &mut c, &mut budget()).map_err(err)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
    let decoded = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let reply_received =
        wire_rename::reply_decode(&decoded, &received, r, &mut receiver, &mut budget())
            .map_err(err)?;
    assert_eq!(&reply_received, reply);
    let encoded =
        wire_rename::reply_to_value(&reply_received, &received, r, &mut receiver, &mut budget())
            .map_err(err)?;
    assert_eq!(encoded, value);
    if matches!(reply.outcome, RenameOutcome::Complete { .. }) {
        wire::boundaries(reply, request, &value, r)?;
    }
    Ok(())
}

#[test]
fn rename_reparses_candidate_and_preserves_original_entity_correspondence() -> Result<(), String> {
    let compiled = execution()?;
    for (input, offset, name, expected) in [
        (
            "let x x apply x x",
            14,
            "z",
            vec![(4, 5), (14, 15), (16, 17)],
        ),
        (
            "lambda x apply x lambda x x",
            15,
            "long",
            vec![(7, 8), (15, 16)],
        ),
        (
            "lambda あ apply あ あ",
            17,
            "名前",
            vec![(7, 10), (17, 20), (21, 24)],
        ),
        (
            "lambda x\r\napply x x",
            16,
            "y",
            vec![(7, 8), (16, 17), (18, 19)],
        ),
        (
            "lambda x apply x guest lambda x x",
            15,
            "z",
            vec![(7, 8), (15, 16)],
        ),
    ] {
        super::super::with_completed_input(&compiled, input, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let original = keyed::prepare(
                "rename",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = original
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
            let request = RenameRequest {
                key: original.key(),
                source: source.clone(),
                offset,
                new_name: name.into(),
                writable: vec![source.clone()],
            };
            let mut operation = budget();
            let mut draft =
                prepare(tree, &original, &bound, &request, &mut operation).map_err(err)?;
            let changed = draft.with_operation(|sources, b, a| {
                let source = sources.latest(&source.source_id).ok_or("candidate")?;
                assert_eq!(source.identity().revision, 1);
                super::super::parse_completed(source, profile, b, a)
            })?;
            let (next, next_bound) =
                draft.with_operation(|sources, b, a| -> Result<_, String> {
                    let mut codec =
                        FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                    let next = keyed::prepare(
                        "rename",
                        changed.tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let next_bound = next.execute(b, a).map_err(err)?;
                    Ok((next, next_bound))
                })?;
            let reply = draft.accept(&changed, &next, &next_bound);
            transport(&reply, &request, profile.registry())?;
            let RenameOutcome::Complete { edits, .. } = &reply.outcome else {
                return Err(format!("{input}: {:?}", reply.outcome));
            };
            assert_eq!(
                edits
                    .iter()
                    .map(|v| (v.span.start(), v.span.end()))
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(
                edits
                    .iter()
                    .all(|v| v.span.snapshot_ref().revision == 0 && v.replacement == name)
            );
            // A separate caller store remains at the original immutable revision
            // until the accepted transaction is explicitly applied.
            let mut store = SourceStore::default();
            for source in &reply.sources {
                store.insert(source.clone()).map_err(err)?;
            }
            store
                .apply(edits, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            assert_eq!(
                store.latest(&source.source_id).ok_or("applied")?.text(),
                changed
                    .tree()
                    .bundle
                    .sources
                    .iter()
                    .find(|v| v.identity().source == source.source_id)
                    .ok_or("parsed")?
                    .text()
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_refuses_capture_free_name_and_scope_collision_after_real_reanalysis() -> Result<(), String>
{
    let compiled = execution()?;
    for (input, offset) in [
        ("lambda x lambda y apply x y", 24),
        ("lambda x apply x y", 15),
        ("twice x y apply x y", 16),
    ] {
        super::super::with_completed_input(&compiled, input, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let original = keyed::prepare(
                "rename",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = original
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
            let request = RenameRequest {
                key: original.key(),
                source: source.clone(),
                offset,
                new_name: "y".into(),
                writable: vec![source.clone()],
            };
            let mut operation = budget();
            let mut draft =
                prepare(tree, &original, &bound, &request, &mut operation).map_err(err)?;
            let changed = draft.with_operation(|sources, b, a| {
                super::super::parse_completed(
                    sources.latest(&source.source_id).ok_or("source")?,
                    profile,
                    b,
                    a,
                )
            })?;
            let (next, next_bound) =
                draft.with_operation(|sources, b, a| -> Result<_, String> {
                    let mut codec =
                        FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                    let next = keyed::prepare(
                        "rename",
                        changed.tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut codec,
                        b,
                    )
                    .map_err(err)?;
                    let bound = next.execute(b, a).map_err(err)?;
                    Ok((next, bound))
                })?;
            let reply = draft.accept(&changed, &next, &next_bound);
            transport(&reply, &request, profile.registry())?;
            assert!(
                matches!(
                    reply.outcome,
                    RenameOutcome::Invalid(RenameError::ResolutionChanged | RenameError::Collision)
                ),
                "{input}: {:?}",
                reply.outcome
            );
            assert!(reply.sources.is_empty());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_candidate_checks_unmapped_bytes_uri_revision_and_explicit_exact_derivation()
-> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let compiled = execution()?;
    for case in 0..5 {
        super::super::with_completed_input(&compiled, "lambda x x", |base, profile, _, _| {
            let input = base
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == "lambda x x")
                .ok_or("input")?;
            let aux = SourceSnapshot::new(
                SourceId("rename-aux".into()),
                0,
                "memory:rename-aux".into(),
                if case == 4 {
                    b"x tail".to_vec()
                } else {
                    b"l a".to_vec()
                },
                &mut budget(),
            )
            .map_err(err)?;
            let start = if case == 4 { 7 } else { 0 };
            let map = Mapping {
                source: input.span(start, start + 1).map_err(err)?,
                target: aux.span(0, 1).map_err(err)?,
                kind: MappingKind::Exact,
            };
            let old = super::super::parse_completed_with_aux(
                input,
                core::slice::from_ref(&aux),
                &[map],
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let original = keyed::prepare(
                "rename",
                old.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = original
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let request = RenameRequest {
                key: original.key(),
                source: input.reference(),
                offset: 9,
                new_name: "z".into(),
                writable: vec![input.reference()],
            };
            let mut b = budget();
            let mut draft = prepare(&old, &original, &bound, &request, &mut b).map_err(err)?;
            let (candidate, wanted_aux) =
                draft.with_operation(|sources, b, _| -> Result<_, String> {
                    Ok((
                        sources
                            .latest(&input.identity().source)
                            .ok_or("candidate")?
                            .clone_with_budget(b)
                            .map_err(err)?,
                        sources
                            .latest(&aux.identity().source)
                            .ok_or("aux")?
                            .clone_with_budget(b)
                            .map_err(err)?,
                    ))
                })?;
            if case == 4 {
                assert_eq!(wanted_aux.text(), "z tail");
                assert_eq!(wanted_aux.identity().revision, 1);
            }
            let next_aux = SourceSnapshot::new(
                aux.identity().source.clone(),
                if case == 3 {
                    1
                } else {
                    wanted_aux.identity().revision
                },
                if case == 2 {
                    "memory:wrong-aux".into()
                } else {
                    wanted_aux.uri().into()
                },
                if case == 1 {
                    b"l b".to_vec()
                } else {
                    wanted_aux.text().as_bytes().to_vec()
                },
                &mut budget(),
            )
            .map_err(err)?;
            let map = Mapping {
                source: candidate.span(start, start + 1).map_err(err)?,
                target: next_aux.span(0, 1).map_err(err)?,
                kind: MappingKind::Exact,
            };
            // This is an actual different parser execution, not mutation of a
            // sealed tree. Fresh resources model a host returning the wrong
            // completed candidate; accept must still enforce exact draft bytes.
            let next_parse = super::super::parse_completed_with_aux(
                &candidate,
                &[next_aux],
                &[map],
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let next = keyed::prepare(
                "rename",
                next_parse.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let next_bound = next
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let reply = draft.accept(&next_parse, &next, &next_bound);
            if case == 0 || case == 4 {
                assert!(
                    matches!(reply.outcome, RenameOutcome::Complete { .. }),
                    "case {case}: {:?}",
                    reply.outcome
                )
            } else {
                assert!(
                    matches!(
                        reply.outcome,
                        RenameOutcome::Invalid(RenameError::RequestMismatch)
                    ),
                    "case {case}: {:?}",
                    reply.outcome
                );
                assert!(reply.sources.is_empty());
            }
            transport(&reply, &request, profile.registry())?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_rejects_stale_requests_seal_swaps_and_sticky_resource_stops() -> Result<(), String> {
    use nepl3_core::budget::Resource;
    let compiled = execution()?;
    super::super::with_completed_input(
        &compiled,
        "lambda あ apply あ あ",
        |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let old = keyed::prepare(
                "rename",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = old
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let input = parsed
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == "lambda あ apply あ あ")
                .ok_or("input")?;
            let request = RenameRequest {
                key: old.key(),
                source: input.reference(),
                offset: 17,
                new_name: "z".into(),
                writable: vec![input.reference()],
            };
            for case in 0..6 {
                let mut altered = request.clone();
                match case {
                    0 => altered.key.tree_digest = Digest::of(b"forged"),
                    1 => altered.source.revision += 1,
                    2 => altered.offset = 18,
                    3 => altered.writable.clear(),
                    4 => altered.writable.push(input.reference()),
                    _ => altered.offset = input.text().len() as u64,
                };
                assert!(
                    prepare(parsed, &old, &bound, &altered, &mut budget()).is_err(),
                    "case {case}"
                );
            }
            let duplicate = super::super::parse_completed(
                input,
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            assert!(matches!(
                prepare(&duplicate, &old, &bound, &request, &mut budget()),
                Err(RenameError::RequestMismatch)
            ));
            for (resource, reason) in [
                (Resource::Work, StopReason::WorkLimit),
                (Resource::AllocationUnits, StopReason::AllocationLimit),
                (Resource::Nodes, StopReason::NodeLimit),
                (Resource::SourceBytes, StopReason::SourceLimit),
            ] {
                let mut b = budget();
                let limit = match resource {
                    Resource::Work => b.limits().work,
                    Resource::AllocationUnits => b.limits().allocation_units,
                    Resource::Nodes => b.limits().nodes,
                    Resource::SourceBytes => b.limits().source_bytes,
                    _ => return Err("resource".into()),
                };
                b.charge(resource, limit).map_err(err)?;
                match prepare(parsed, &old, &bound, &request, &mut b) {
                    Err(error) => assert_eq!(error.stop_reason(), Some(reason)),
                    Ok(_) => return Err("unexpected draft".into()),
                };
                assert_eq!(b.poll(), Err(reason));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                prepare(parsed, &old, &bound, &request, &mut cancelled),
                Err(RenameError::Stopped(StopReason::Cancelled))
            ));
            for case in 0..3 {
                let mut b = budget();
                let mut draft = prepare(parsed, &old, &bound, &request, &mut b).map_err(err)?;
                let next_parse = draft.with_operation(|sources, b, a| {
                    super::super::parse_completed(
                        sources
                            .latest(&input.identity().source)
                            .ok_or("candidate")?,
                        profile,
                        b,
                        a,
                    )
                })?;
                let (next, next_bound) =
                    draft.with_operation(|sources, b, a| -> Result<_, String> {
                        let mut c =
                            FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                        let next = keyed::prepare(
                            if case == 2 {
                                "other-operation"
                            } else {
                                "rename"
                            },
                            next_parse.tree(),
                            BindingOptions,
                            b.limits(),
                            profile,
                            &mut c,
                            b,
                        )
                        .map_err(err)?;
                        let bound = next.execute(b, a).map_err(err)?;
                        Ok((next, bound))
                    })?;
                if case == 1 {
                    draft.with_operation(|_, b, _| b.cancel());
                }
                let reply = draft.accept(
                    if case == 0 { parsed } else { &next_parse },
                    &next,
                    &next_bound,
                );
                assert!(reply.sources.is_empty());
                if case == 1 {
                    assert!(matches!(
                        reply.outcome,
                        RenameOutcome::Stopped(StopReason::Cancelled)
                    ))
                } else {
                    assert!(matches!(
                        reply.outcome,
                        RenameOutcome::Invalid(RenameError::RequestMismatch)
                    ))
                }
                transport(&reply, &request, profile.registry())?;
            }
            Ok(())
        },
    )
}

#[test]
fn rename_refuses_ambiguous_and_encoded_names_without_inverse_encoder() -> Result<(), String> {
    let compiled = execution()?;
    for (input, offset, expected) in [
        ("twice x x x", 10, RenameError::Ambiguous),
        (
            "lettext \"\\u{78}\" x",
            17,
            RenameError::RenameNotInvertible,
        ),
        ("let x x x", 6, RenameError::Unresolved),
    ] {
        super::super::with_completed_input(&compiled, input, |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let original = keyed::prepare(
                "rename",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = original
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let source = parsed
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == input)
                .ok_or("input")?
                .reference();
            let request = RenameRequest {
                key: original.key(),
                source: source.clone(),
                offset,
                new_name: "z".into(),
                writable: vec![source],
            };
            match prepare(parsed, &original, &bound, &request, &mut budget()) {
                Err(e) => assert_eq!(e, expected),
                Ok(_) => return Err("unexpected invertible rename".into()),
            };
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_inverse_rejects_transformed_and_multiple_source_paths() -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let compiled = execution()?;
    for multiple in [false, true] {
        super::super::with_completed_input(&compiled, "lambda x x", |base, profile, _, _| {
            let input = base
                .tree()
                .bundle
                .sources
                .iter()
                .find(|v| v.text() == "lambda x x")
                .ok_or("input")?;
            let first = SourceSnapshot::new(
                SourceId("inverse-first".into()),
                0,
                "memory:inverse-first".into(),
                b"x".to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let second = SourceSnapshot::new(
                SourceId("inverse-second".into()),
                0,
                "memory:inverse-second".into(),
                b"x".to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let mut maps = vec![Mapping {
                source: first.span(0, 1).map_err(err)?,
                target: input.span(7, 8).map_err(err)?,
                kind: if multiple {
                    MappingKind::Exact
                } else {
                    MappingKind::Transformed
                },
            }];
            if multiple {
                maps.push(Mapping {
                    source: second.span(0, 1).map_err(err)?,
                    target: input.span(7, 8).map_err(err)?,
                    kind: MappingKind::Exact,
                });
            }
            let parsed = super::super::parse_completed_with_aux(
                input,
                &[first, second],
                &maps,
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let old = keyed::prepare(
                "rename",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = old
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let request = RenameRequest {
                key: old.key(),
                source: input.reference(),
                offset: 9,
                new_name: "z".into(),
                writable: vec![input.reference()],
            };
            assert!(matches!(
                prepare(&parsed, &old, &bound, &request, &mut budget()),
                Err(RenameError::RenameNotInvertible)
            ));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_reserved_heads_use_namespace_owner_and_ignore_other_selected_language()
-> Result<(), String> {
    let mut compiled = execution()?;
    let document = nepl3_tools::bootstrap::load(
        include_bytes!("../../../../../conformance/fixtures/grammar/binding/official.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    let mut other = nepl3_tools::bootstrap::catalog::compile(
        &document,
        "test.rename-other",
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    other
        .package
        .forms
        .first_mut()
        .ok_or("other form")?
        .spelling = "z".into();
    let schema = other.package.schema.clone();
    let descriptor = other
        .registry
        .descriptor(&schema)
        .ok_or("other descriptor")?
        .clone();
    compiled
        .registry
        .register(schema, descriptor, &mut budget())
        .map_err(err)?;
    compiled.registry.finalize(&mut budget()).map_err(err)?;
    for new_name in ["let", "z"] {
        super::super::with_completed_input_extra(
            &compiled,
            Some(&other.package),
            "lambda x 1",
            |parsed, profile, _, _| {
                let empty = SourceStore::default();
                let mut a = SourceAdmission::default();
                let mut c =
                    FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
                let old = keyed::prepare(
                    "rename",
                    parsed.tree(),
                    BindingOptions,
                    budget().limits(),
                    profile,
                    &mut c,
                    &mut budget(),
                )
                .map_err(err)?;
                let bound = old
                    .execute(&mut budget(), &mut SourceAdmission::default())
                    .map_err(err)?;
                let source = parsed
                    .tree()
                    .bundle
                    .sources
                    .iter()
                    .find(|v| v.text() == "lambda x 1")
                    .ok_or("input")?
                    .reference();
                let request = RenameRequest {
                    key: old.key(),
                    source: source.clone(),
                    offset: 7,
                    new_name: new_name.into(),
                    writable: vec![source.clone()],
                };
                let mut b = budget();
                let prepared = prepare(parsed, &old, &bound, &request, &mut b);
                if new_name == "let" {
                    assert!(matches!(prepared, Err(RenameError::InvalidName)));
                    return Ok(());
                }
                let mut draft = prepared.map_err(err)?;
                let next_parse = draft.with_operation(|sources, b, a| {
                    super::super::parse_completed(
                        sources.latest(&source.source_id).ok_or("candidate")?,
                        profile,
                        b,
                        a,
                    )
                })?;
                let (next, next_bound) =
                    draft.with_operation(|sources, b, a| -> Result<_, String> {
                        let mut c =
                            FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                        let next = keyed::prepare(
                            "rename",
                            next_parse.tree(),
                            BindingOptions,
                            b.limits(),
                            profile,
                            &mut c,
                            b,
                        )
                        .map_err(err)?;
                        let bound = next.execute(b, a).map_err(err)?;
                        Ok((next, bound))
                    })?;
                let reply = draft.accept(&next_parse, &next, &next_bound);
                assert!(
                    matches!(reply.outcome, RenameOutcome::Complete { .. }),
                    "{:?}",
                    reply.outcome
                );
                transport(&reply, &request, profile.registry())
            },
        )?;
    }
    Ok(())
}

#[test]
fn rename_exact_segments_preserve_unique_coverage_and_unmapped_geometry() -> Result<(), String> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let compiled = execution()?;
    for (case, old_name, new_name) in [
        (0, "ab", "cd"),
        (1, "ab", "cd"),
        (2, "ab", "cd"),
        (3, "ab", "cd"),
        (2, "あい", "漢字仮"),
    ] {
        let original_text = format!("lambda {old_name} {old_name}");
        super::super::with_completed_input(&compiled, &original_text, |base, profile, _, _| {
            let input = base
                .tree()
                .bundle
                .sources
                .iter()
                .find(|s| s.text() == original_text)
                .ok_or("input")?;
            let aux = SourceSnapshot::new(
                SourceId("name-origin".into()),
                0,
                "memory:name-origin".into(),
                old_name.as_bytes().to_vec(),
                &mut budget(),
            )
            .map_err(err)?;
            let mappings = |input: &SourceSnapshot,
                            aux: &SourceSnapshot,
                            broken: bool|
             -> Result<Vec<Mapping>, String> {
                let mut result = Vec::new();
                for start in [7, 8 + aux.text().len() as u64] {
                    for (a, z) in if case == 0 {
                        vec![(0, aux.text().len() as u64)]
                    } else if broken {
                        vec![(0, 1)]
                    } else {
                        {
                            let mut points = aux
                                .text()
                                .char_indices()
                                .map(|(i, _)| i as u64)
                                .collect::<Vec<_>>();
                            points.push(aux.text().len() as u64);
                            points.windows(2).map(|v| (v[0], v[1])).collect()
                        }
                    } {
                        result.push(Mapping {
                            source: aux.span(a, z).map_err(err)?,
                            target: input.span(start + a, start + z).map_err(err)?,
                            kind: MappingKind::Exact,
                        });
                    }
                }
                if case == 2 {
                    result.reverse();
                }
                Ok(result)
            };
            let old = super::super::parse_completed_with_aux(
                input,
                core::slice::from_ref(&aux),
                &mappings(input, &aux, case == 3)?,
                profile,
                &mut budget(),
                &mut SourceAdmission::default(),
            )?;
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let original = keyed::prepare(
                "segments",
                old.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = original
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let request = RenameRequest {
                key: original.key(),
                source: input.reference(),
                offset: 8 + old_name.len() as u64,
                new_name: new_name.into(),
                writable: vec![aux.reference()],
            };
            let mut b = budget();
            if case == 3 {
                assert!(matches!(
                    prepare(&old, &original, &bound, &request, &mut b),
                    Err(RenameError::RenameNotInvertible)
                ));
                return Ok(());
            }
            let mut draft = prepare(&old, &original, &bound, &request, &mut b).map_err(err)?;
            let candidate = draft.with_operation(|sources, b, a| -> Result<_, String> {
                let main = sources.latest(&input.identity().source).ok_or("main")?;
                let name = sources.latest(&aux.identity().source).ok_or("name")?;
                assert_eq!(main.text(), format!("lambda {new_name} {new_name}"));
                assert_eq!(name.text(), new_name);
                super::super::parse_completed_with_aux(
                    main,
                    core::slice::from_ref(name),
                    &mappings(main, name, false)?,
                    profile,
                    b,
                    a,
                )
            })?;
            let (prepared, bound) = draft.with_operation(|sources, b, a| -> Result<_, String> {
                let mut c = FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                let prepared = keyed::prepare(
                    "segments",
                    candidate.tree(),
                    BindingOptions,
                    b.limits(),
                    profile,
                    &mut c,
                    b,
                )
                .map_err(err)?;
                let bound = prepared.execute(b, a).map_err(err)?;
                Ok((prepared, bound))
            })?;
            let reply = draft.accept(&candidate, &prepared, &bound);
            let RenameOutcome::Complete { edits, .. } = &reply.outcome else {
                return Err(format!("segments {case}: {:?}", reply.outcome));
            };
            // Both uses share one original name. Segmentation and table order
            // cannot create a second edit or exempt the unmapped lambda bytes.
            assert_eq!(edits.len(), 1);
            assert_eq!(
                edits[0].span,
                aux.span(0, old_name.len() as u64).map_err(err)?
            );
            transport(&reply, &request, profile.registry())?;
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn rename_uses_custom_final_resolutions_and_refuses_missing_locations() -> Result<(), String> {
    let compiled = super::super::custom::compiled()?;
    for (input, offset, updates, source_less, accepts) in [
        ("custom x x", 9, false, false, true),
        ("custom x x", 9, false, true, false),
        ("early x z custom x x", 6, true, false, false),
        (
            "lambda y apply y early x z custom x x",
            15,
            true,
            false,
            true,
        ),
    ] {
        super::super::with_completed_input(&compiled, input, |parsed, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let old = keyed::prepare(
                "rename-custom",
                parsed.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut c,
                &mut budget(),
            )
            .map_err(err)?;
            let mut host = super::super::custom::query_host(updates, source_less);
            let bound = old
                .execute_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let BindingOutcome::Complete(analysis) = &bound.reply().outcome else {
                return Err(format!("{:?}", bound.reply()));
            };
            assert!(
                analysis
                    .facts()
                    .entities
                    .iter()
                    .any(|e| e.id == EntityId(100))
            );
            if updates {
                assert!(!analysis.result().resolution_history.is_empty());
            }
            let source = parsed
                .tree()
                .bundle
                .sources
                .iter()
                .find(|s| s.text() == input)
                .ok_or("input")?
                .reference();
            let request = RenameRequest {
                key: old.key(),
                source: source.clone(),
                offset,
                new_name: "n".into(),
                writable: vec![source.clone()],
            };
            let mut b = budget();
            let result = prepare(parsed, &old, &bound, &request, &mut b);
            if source_less {
                assert!(matches!(result, Err(RenameError::NoLocation)));
                return Ok(());
            }
            let mut draft = result.map_err(err)?;
            let changed = draft.with_operation(|sources, b, a| {
                super::super::parse_completed(
                    sources.latest(&source.source_id).ok_or("input")?,
                    profile,
                    b,
                    a,
                )
            })?;
            let (next, next_bound) =
                draft.with_operation(|sources, b, a| -> Result<_, String> {
                    let mut c =
                        FoundationCodec::new(profile.registry(), sources, a).map_err(err)?;
                    let next = keyed::prepare(
                        "rename-custom",
                        changed.tree(),
                        BindingOptions,
                        b.limits(),
                        profile,
                        &mut c,
                        b,
                    )
                    .map_err(err)?;
                    let mut host = super::super::custom::query_host(updates, source_less);
                    let reply = next.execute_with_host(&mut host, b, a).map_err(err)?;
                    Ok((next, reply))
                })?;
            let reply = draft.accept(&changed, &next, &next_bound);
            if accepts {
                assert!(
                    matches!(reply.outcome, RenameOutcome::Complete { .. }),
                    "{input}: {:?}",
                    reply.outcome
                );
            } else {
                // This provider explicitly updates existing references named x.
                // Renaming that name changes its domain decision; a raw name
                // substitution cannot preserve the accepted final resolution.
                assert!(
                    matches!(
                        reply.outcome,
                        RenameOutcome::Invalid(RenameError::ResolutionChanged)
                    ),
                    "{input}: {:?}",
                    reply.outcome
                );
                assert!(reply.sources.is_empty());
            }
            transport(&reply, &request, profile.registry())?;
            Ok(())
        })?;
    }
    Ok(())
}
