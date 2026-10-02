use super::*;
use nepl3_engine::{
    analysis::region::{
        RegionRequest,
        completion::{self as selected, *},
    },
    portable::region as wire_region,
};

#[test]
fn region_name_candidates_preserve_owner_prefix_and_empty_states() -> Result<(), String> {
    let compiled = execution()?;
    for (text, offset, prefix, expected_groups, expected) in [
        ("lambda a probe", 9, "", 1, vec!["a"]),
        ("lambda a lambda b probe", 18, "", 1, vec!["b", "a"]),
        ("lambda a guest probe", 15, "", 1, vec![]),
        ("guest lambda a probe", 15, "", 1, vec!["a"]),
        ("lambda あ probe", 11, "あ", 1, vec!["あ"]),
        ("lambda a probe", 9, "z", 1, vec![]),
        ("lambda a probe", 7, "", 0, vec![]),
        ("lambda a probe", 0, "", 0, vec![]),
        ("lambda a probe", 8, "", 0, vec![]),
        ("lambda a probe", 14, "", 0, vec![]),
    ] {
        with_input(&compiled, text, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut a = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "region-completion",
                tree.tree(),
                BindingOptions,
                budget().limits(),
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let bound = prepared
                .execute(&mut budget(), &mut SourceAdmission::default())
                .map_err(err)?;
            let input =
                wire_region::prepare(&prepared, None, &mut codec, &mut budget()).map_err(err)?;
            let request = RegionCompletionRequest {
                region: RegionRequest {
                    key: input.key(),
                    source: tree.tree().bundle.sources[0].reference(),
                    offset,
                },
                prefix: prefix.into(),
            };
            let mut b = budget();
            let reply = selected::names(
                &input,
                &bound,
                &request,
                &mut b,
                &mut SourceAdmission::default(),
            );
            assert_eq!(reply.key, input.key());
            assert_eq!(reply.report.usage, b.usage());
            let RegionCompletionOutcome::Complete { groups, .. } = reply.outcome else {
                return Err(format!("completion failed: {text}/{offset}"));
            };
            assert_eq!(groups.len(), expected_groups, "{text}/{offset}");
            if let Some(group) = groups.first() {
                assert_eq!(
                    group
                        .candidates
                        .iter()
                        .map(|v| v.name.as_str())
                        .collect::<Vec<_>>(),
                    expected,
                    "{text}"
                );
                assert_eq!(group.key, bound.key());
            }
            let mut limits = budget().limits();
            limits.work = b.usage().work - 1;
            let stopped = selected::names(
                &input,
                &bound,
                &request,
                &mut Budget::new(limits),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(
                stopped.outcome,
                RegionCompletionOutcome::Stopped(StopReason::WorkLimit)
            ));
            assert!(stopped.sources.is_empty());
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn region_name_candidates_reject_stale_keys_sources_and_invalid_utf8_positions()
-> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda あ probe", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "region-completion",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bound = prepared
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let input =
            wire_region::prepare(&prepared, None, &mut codec, &mut budget()).map_err(err)?;
        for mode in 0..8 {
            let mut request = RegionCompletionRequest {
                region: RegionRequest {
                    key: input.key(),
                    source: tree.tree().bundle.sources[0].reference(),
                    offset: 11,
                },
                prefix: "".into(),
            };
            let stale = Digest::of(b"stale");
            match mode {
                0 => request.region.key.analysis.tree_digest = stale,
                1 => request.region.key.analysis.profile_digest = stale,
                2 => request.region.key.analysis.execution_digest = stale,
                3 => request.region.key.analysis.request_digest = stale,
                4 => request.region.key.reader_facts_digest = stale,
                5 => request.region.source.digest = stale,
                6 => request.region.offset = 8, // inside the three-byte あ
                _ => request.region.offset = 100,
            }
            let reply = selected::names(
                &input,
                &bound,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(
                matches!(reply.outcome, RegionCompletionOutcome::Invalid(_)),
                "case {mode}"
            );
            assert!(reply.sources.is_empty());
        }
        Ok(())
    })
}

#[test]
fn region_candidates_keep_custom_final_resolution_separate_from_captured_visibility()
-> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "region-custom-names",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bound = prepared
            .execute_with_host(
                &mut custom::query_host(true, true),
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(err)?;
        let input =
            wire_region::prepare(&prepared, None, &mut codec, &mut budget()).map_err(err)?;
        for (offset, expected) in [(6, vec![]), (19, vec!["x", "z"])] {
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
            let RegionCompletionOutcome::Complete { groups, .. } = reply.outcome else {
                return Err("custom completion".into());
            };
            assert_eq!(groups.len(), 1);
            assert_eq!(
                groups[0]
                    .candidates
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            if let Some(candidate) = groups[0].candidates.first() {
                assert_eq!(
                    candidate.resolution,
                    ReferenceResolution::Resolved(EntityId(100))
                );
            }
        }
        let request = RegionCompletionRequest {
            region: RegionRequest {
                key: input.key(),
                source: tree.tree().bundle.sources[0].reference(),
                offset: 19,
            },
            prefix: "".into(),
        };
        // Invalid and stopped native analyses never become empty successful groups.
        let invalid = prepared
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            invalid.reply().outcome,
            BindingOutcome::Invalid { .. }
        ));
        let mut cancelled = budget();
        cancelled.cancel();
        let stopped = prepared
            .execute(&mut cancelled, &mut SourceAdmission::default())
            .map_err(err)?;
        for incomplete in [&invalid, &stopped] {
            let reply = selected::names(
                &input,
                incomplete,
                &request,
                &mut budget(),
                &mut SourceAdmission::default(),
            );
            assert!(matches!(reply.outcome, RegionCompletionOutcome::Invalid(_)));
            assert!(reply.sources.is_empty());
        }
        for resource in 0..4 {
            let mut limits = budget().limits();
            let reason = match resource {
                0 => {
                    limits.nodes = 0;
                    StopReason::NodeLimit
                }
                1 => {
                    limits.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                2 => {
                    limits.source_bytes = 0;
                    StopReason::SourceLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut b = Budget::new(limits);
            if resource == 3 {
                b.cancel();
            }
            let reply = selected::names(
                &input,
                &bound,
                &request,
                &mut b,
                &mut SourceAdmission::default(),
            );
            assert!(matches!(reply.outcome, RegionCompletionOutcome::Stopped(r) if r == reason));
            assert!(reply.sources.is_empty());
        }
        Ok(())
    })
}

struct MappedReferences {
    transformed: bool,
    inner: Box<dyn BindingHost>,
}
impl BindingHost for MappedReferences {
    fn authorize(
        &mut self,
        call: &BindingCall<'_>,
        b: &mut Budget,
    ) -> Result<Option<FactAuthority>, BindingError> {
        self.inner.authorize(call, b)
    }
    fn facts(
        &mut self,
        provider: &ProviderRequirement,
        request: &nepl3_engine::facts::CheckedFactsView<'_, '_>,
        emit: &mut nepl3_engine::facts::FactsEmitter<'_>,
    ) -> Result<CustomOutcome, BindingError> {
        let CustomOutcome::Complete(mut delta) = self.inner.facts(provider, request, emit)? else {
            return Err(BindingError::ProviderInvalid);
        };
        if self.transformed {
            for map in &mut delta.source_maps {
                map.kind = nepl3_core::origin::MappingKind::Transformed;
            }
        }
        let occurrence = delta.occurrences.first_mut().ok_or(BindingError::Target)?;
        occurrence.role = OccurrenceRole::Reference;
        emit.budget().charge(
            nepl3_core::budget::Resource::Work,
            occurrence.name.len() as u64 + 1,
        )?;
        emit.budget().charge(
            nepl3_core::budget::Resource::AllocationUnits,
            occurrence.name.len() as u64 + core::mem::size_of::<Occurrence>() as u64,
        )?;
        let mut second = Occurrence {
            id: occurrence.id,
            scope: occurrence.scope,
            namespace: occurrence.namespace,
            name: occurrence.name.clone(),
            role: occurrence.role,
            span: occurrence.span.clone_with_budget(emit.budget())?,
            origin: occurrence.origin,
            resolution: occurrence.resolution.clone_with_budget(emit.budget())?,
        };
        second.id.0 += 1;
        delta.occurrences.push(second);
        Ok(CustomOutcome::Complete(delta))
    }
}

#[test]
fn mapped_region_candidates_keep_groups_and_require_analysis_source_admission() -> Result<(), String>
{
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
                    "mapped-completion",
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
                if !matches!(bound.reply().outcome, BindingOutcome::Complete(_)) {
                    return Err(err(bound.reply()));
                }
                let input = wire_region::prepare(&prepared, None, &mut codec, &mut budget())
                    .map_err(err)?;
                for (offset, first_id, entity) in [(7, 200, 100), (22, 300, 200)] {
                    let request = RegionCompletionRequest {
                        region: RegionRequest {
                            key: input.key(),
                            source: tree.tree().bundle.sources[0].reference(),
                            offset,
                        },
                        prefix: "".into(),
                    };
                    let mut b = budget();
                    let mut admission = SourceAdmission::default();
                    let result = selected::names(&input, &bound, &request, &mut b, &mut admission);
                    let RegionCompletionOutcome::Complete { groups, .. } = result.outcome else {
                        return Err("mapped completion".into());
                    };
                    assert_eq!(groups.len(), 2);
                    assert_eq!(
                        groups.iter().map(|g| g.occurrence.0).collect::<Vec<_>>(),
                        vec![first_id, first_id + 1]
                    );
                    for group in groups {
                        assert_eq!(group.candidates.len(), 1);
                        assert_eq!(
                            group.candidates[0].resolution,
                            ReferenceResolution::Resolved(EntityId(entity))
                        );
                    }
                    assert!(result.sources.len() > tree.tree().bundle.sources.len());
                    let generated = bound
                        .reply()
                        .sources()
                        .iter()
                        .find(|s| s.identity() != tree.tree().bundle.sources[0].identity())
                        .ok_or("generated source")?;
                    let conflict = SourceSnapshot::new(
                        generated.identity().source.clone(),
                        generated.identity().revision,
                        generated.uri().into(),
                        b"different".to_vec(),
                        &mut budget(),
                    )
                    .map_err(err)?;
                    let mut conflicting = SourceAdmission::default();
                    conflicting
                        .admit_existing(&conflict, &mut budget())
                        .map_err(err)?;
                    let rejected =
                        selected::names(&input, &bound, &request, &mut budget(), &mut conflicting);
                    assert!(matches!(
                        rejected.outcome,
                        RegionCompletionOutcome::Invalid(_)
                    ));
                    assert!(rejected.sources.is_empty());
                    let charged = b.usage().source_bytes;
                    let again = selected::names(&input, &bound, &request, &mut b, &mut admission);
                    assert!(matches!(
                        again.outcome,
                        RegionCompletionOutcome::Complete { .. }
                    ));
                    assert_eq!(b.usage().source_bytes, charged);
                    let mut limits = budget().limits();
                    limits.source_bytes = tree
                        .tree()
                        .bundle
                        .sources
                        .iter()
                        .map(|s| s.text().len() as u64)
                        .sum();
                    let stopped = selected::names(
                        &input,
                        &bound,
                        &request,
                        &mut Budget::new(limits),
                        &mut SourceAdmission::default(),
                    );
                    assert!(matches!(
                        stopped.outcome,
                        RegionCompletionOutcome::Stopped(StopReason::SourceLimit)
                    ));
                    assert!(stopped.sources.is_empty());
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}

#[test]
fn region_candidates_follow_foreign_field_priority_and_restore_caller_depth() -> Result<(), String>
{
    use nepl3_engine::{
        analysis::region::RegionPart,
        package::{SelectionRule, StyleSelector},
    };
    let mut compiled = execution()?;
    let guest = compiled
        .package
        .forms
        .iter_mut()
        .find(|f| f.spelling == "guest")
        .ok_or("Guest form")?;
    guest.selection_rules.push(SelectionRule {
        selector: StyleSelector::Field("value".into()),
        priority: 100,
    });
    let text = "lambda host guest probe";
    with_input(&compiled, text, |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "foreign-field-completion",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bound = prepared
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let input =
            wire_region::prepare(&prepared, None, &mut codec, &mut budget()).map_err(err)?;
        let request = RegionCompletionRequest {
            region: RegionRequest {
                key: input.key(),
                source: tree.tree().bundle.sources[0].reference(),
                offset: text.rfind("probe").ok_or("probe")? as u64,
            },
            prefix: "".into(),
        };
        let mut b = budget();
        let reply = b
            .with_depth(|b| {
                let caller = b.current_depth();
                let result =
                    selected::names(&input, &bound, &request, b, &mut SourceAdmission::default());
                assert_eq!(b.current_depth(), caller);
                Ok::<_, StopReason>(result)
            })
            .map_err(err)?;
        assert_eq!(b.current_depth(), 0);
        let RegionCompletionOutcome::Complete {
            region: Some(region),
            groups,
        } = reply.outcome
        else {
            return Err("foreign selected completion".into());
        };
        assert!(matches!(region.target.part, RegionPart::Field { .. }));
        assert_eq!(region.priority, 100);
        assert_eq!(groups.len(), 1);
        assert_eq!(
            groups[0]
                .candidates
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            Vec::<&str>::new()
        );
        Ok(())
    })
}

#[path = "region/portable.rs"]
mod portable;
