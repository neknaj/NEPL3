use super::*;
use nepl3_core::{budget::StopReason, source::SourceError};
use nepl3_engine::{
    analysis::{
        BindingOptions,
        probe::{ProbeAccessError, ProbePosition},
    },
    binding::probe::ProbeOutcome,
    portable::analysis as keyed,
};

#[test]
fn keyed_probe_preserves_foreign_and_canonical_identity_and_unused_source_closure()
-> Result<(), String> {
    let compiled = super::missing_probe::named_lambda()?;
    for input in [
        "sequence cons define a b nil lambda x",
        "let outer 1 guest lambda inner",
    ] {
        with_source_profile(&compiled, None, input, |source, profile, b, a| {
            let parsed = parse_any_with_aux(source, &[], &[], profile, b, a)?;
            let ParseCompletion::Break(ParseReply {
                outcome: ParseOutcome::Recovered { mut tree, .. },
                ..
            }) = parsed
            else {
                return Err("expected recovered input".into());
            };
            let unused = SourceSnapshot::new(
                SourceId("unused-probe-input".into()),
                0,
                "memory:unused-probe-input".into(),
                "補助".as_bytes().to_vec(),
                b,
            )
            .map_err(err)?;
            if input.contains("guest") {
                let foreign = tree
                    .bundle
                    .nodes
                    .iter_mut()
                    .flat_map(|node| node.fields.iter_mut())
                    .find_map(|field| match field {
                        nepl3_core::syntax::FieldValue::Foreign(value) => Some(value),
                        _ => None,
                    })
                    .ok_or("foreign bundle")?;
                foreign.bundle.sources.push(unused.clone());
                assert!(
                    !tree
                        .bundle
                        .sources
                        .iter()
                        .any(|s| s.identity().source == unused.identity().source)
                );
            } else {
                tree.bundle.sources.push(unused.clone());
            }
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let limits = budget().limits();
            let prepared = keyed::prepare(
                "keyed-foreign",
                &tree,
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            let key = prepared.key();
            let bound = prepared
                .probe_missing_reference(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let ProbeOutcome::Hit(hit) = &bound.reply().outcome else {
                return Err(format!("hit: {:?}", bound.reply().outcome));
            };
            let expected_site = hit.site();
            assert_eq!(
                !expected_site.owner.path.is_empty(),
                input.contains("guest")
            );
            let position_source = source.reference();
            assert!(matches!(
                bound.for_position(
                    &key,
                    &position_source,
                    input.len() as u64,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                ),
                Ok(ProbePosition::HitAtPosition(_))
            ));
            assert!(matches!(
                bound.for_position(
                    &key,
                    &unused.reference(),
                    0,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                ),
                Ok(ProbePosition::OtherHit(_))
            ));

            // An early native stop has no admitted progress sources. The keyed
            // query still validates every source retained in the input bundle.
            let mut cancelled = Budget::new(limits);
            cancelled.cancel();
            let stopped = prepared
                .probe_missing_reference(&mut cancelled, &mut SourceAdmission::default())
                .map_err(err)?;
            assert!(stopped.reply().sources().is_empty());
            assert!(matches!(
                stopped.for_position(
                    &key,
                    &unused.reference(),
                    0,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default()
                ),
                Ok(ProbePosition::Stopped(StopReason::Cancelled))
            ));
            let mut query = Budget::new(limits);
            let conflict = SourceSnapshot::new(
                unused.identity().source.clone(),
                0,
                "memory:conflicting-unused".into(),
                "補助".as_bytes().to_vec(),
                &mut query,
            )
            .map_err(err)?;
            let mut ledger = SourceAdmission::default();
            ledger.admit_existing(&conflict, &mut query).map_err(err)?;
            assert_eq!(
                stopped
                    .for_position(
                        &key,
                        &position_source,
                        input.len() as u64,
                        &mut query,
                        &mut ledger
                    )
                    .err(),
                Some(ProbeAccessError::Source(SourceError::IdentityConflict))
            );

            // Canonical metadata order is independent of in-memory storage order.
            let mut reordered = tree.clone();
            reordered.contexts.reverse();
            for context in &mut reordered.contexts {
                context.nodes.reverse();
            }
            fn reverse_sources(bundle: &mut nepl3_core::syntax::SyntaxBundle) {
                bundle.sources.reverse();
                for node in &mut bundle.nodes {
                    for field in &mut node.fields {
                        if let nepl3_core::syntax::FieldValue::Foreign(value) = field {
                            reverse_sources(&mut value.bundle);
                        }
                    }
                }
            }
            reverse_sources(&mut reordered.bundle);
            let equivalent = keyed::prepare(
                "keyed-foreign",
                &reordered,
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut budget(),
            )
            .map_err(err)?;
            assert_eq!(equivalent.key(), key);
            let equivalent = equivalent
                .probe_missing_reference(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let ProbeOutcome::Hit(equivalent_hit) = &equivalent.reply().outcome else {
                return Err("reordered hit".into());
            };
            same_site(equivalent_hit.site(), expected_site);
            assert_eq!(equivalent_hit.stages(), hit.stages());

            // First receiver obtains a prepared request from canonical bytes,
            // not an adopted raw probe reply or a claimed AnalysisKey.
            let mut transport = budget();
            let value =
                keyed::request_to_value(&prepared, &mut codec, &mut transport).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut transport).map_err(err)?;
            let packet = nepl3_wire::decode(&bytes, &mut transport).map_err(err)?;
            let mut receiver_admission = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut receiver_admission)
                    .map_err(err)?;
            let raw = keyed::request_decode(&packet, profile, &mut receiver, &mut transport)
                .map_err(err)?;
            let received = keyed::prepare_received(&raw, profile, &mut receiver, &mut transport)
                .map_err(err)?;
            assert_eq!(received.key(), key);
            let received = received
                .probe_missing_reference(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let ProbePosition::HitAtPosition(received_hit) = received
                .for_position(
                    &key,
                    &position_source,
                    input.len() as u64,
                    &mut Budget::new(limits),
                    &mut SourceAdmission::default(),
                )
                .map_err(err)?
            else {
                return Err("received position".into());
            };
            same_site(received_hit.site(), expected_site);
            assert_eq!(received_hit.stages(), hit.stages());
            Ok(())
        })?;
    }
    Ok(())
}

fn same_site(
    a: &nepl3_engine::binding::probe::MissingReferenceSite,
    b: &nepl3_engine::binding::probe::MissingReferenceSite,
) {
    assert_eq!(a.owner, b.owner);
    assert_eq!(a.name_target, b.name_target);
    assert_eq!(a.binding, b.binding);
    assert_eq!(a.execution_step, b.execution_step);
    assert_eq!(a.stage, b.stage);
    assert_eq!(a.namespace_stage, b.namespace_stage);
    assert_eq!(a.namespace, b.namespace);
    assert_eq!(a.anchor, b.anchor);
}
