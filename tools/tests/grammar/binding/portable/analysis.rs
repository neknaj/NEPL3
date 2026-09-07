use super::*;
use nepl3_engine::{
    analysis::{BindingAccessError, BindingOptions},
    portable::analysis as keyed,
};

#[test]
fn keyed_invalid_reply_and_access_errors_keep_typed_causes() -> Result<(), String> {
    let compiled = super::super::global::compiled()?;
    with_input(&compiled, "lambda x lambda x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let limits = budget().limits();
        let request = keyed::prepare(
            "duplicate",
            tree.tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let reply = request
            .execute(&mut Budget::new(limits), &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            reply.reply().outcome,
            BindingOutcome::Invalid {
                error: BindingError::DuplicateGlobal,
                ..
            }
        ));
        let source = tree.tree().bundle.sources[0].reference();
        assert!(matches!(
            reply.for_source(&request.key(), &source, &mut budget()),
            Err(BindingAccessError::Incomplete)
        ));
        let value = keyed::reply_to_value(&reply, profile.registry(), &mut codec, &mut budget())
            .map_err(err)?;
        let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
        let packet = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        let decoded =
            keyed::reply_decode(&packet, &request, &mut codec, &mut budget()).map_err(err)?;
        assert!(matches!(
            decoded.reply.outcome,
            DecodedBindingOutcome::Invalid {
                failure: BindingError::DuplicateGlobal,
                ..
            }
        ));
        assert_eq!(decoded.reply.report, reply.reply().report);
        let mut forged = packet.clone();
        let NdfValue::Record(outer) = &mut forged else {
            return Err("reply".into());
        };
        let NdfValue::Record(key) = &mut outer.fields[0] else {
            return Err("key".into());
        };
        key.fields[2] = NdfValue::Bytes(vec![0; 32]);
        assert!(matches!(
            keyed::reply_decode(&forged, &request, &mut codec, &mut budget()),
            Err(nepl3_engine::portable::PortableError::RequestMismatch)
        ));
        let mut errors = vec![
            BindingAccessError::LimitsMismatch,
            BindingAccessError::StaleAnalysis,
            BindingAccessError::MissingSource,
            BindingAccessError::Incomplete,
        ];
        for reason in [
            StopReason::Cancelled,
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ] {
            errors.push(BindingAccessError::Stopped(reason));
        }
        for error in errors {
            let value = keyed::access_error_to_value(error, profile.registry(), &mut budget())
                .map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            assert_eq!(
                keyed::access_error_from_value(&value, profile.registry(), &mut budget())
                    .map_err(err)?,
                error
            );
        }
        Ok(())
    })
}

#[test]
fn prepared_binding_request_first_receiver_and_keyed_reply_preserve_identity() -> Result<(), String>
{
    let compiled = execution()?;
    for input in [
        "lambda x apply guest x x",
        "lambda x guest x",
        "lettext \"\\u{78}\" x",
    ] {
        with_input(&compiled, input, |tree, profile, _, _| {
            let empty = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
            let mut preparing = budget();
            let limits = budget().limits();
            let issued = keyed::prepare(
                "keyed",
                tree.tree(),
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut preparing,
            )
            .map_err(err)?;
            let key = issued.key();
            let native = issued
                .execute(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            let expected_native = native
                .reply()
                .facts()
                .ok_or("native complete facts")?
                .clone();
            // Metadata storage order is not part of canonical tree identity.
            // Stable generated IDs must identify the same targets nonetheless.
            let mut reordered = tree.tree().clone();
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
                "keyed",
                &reordered,
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut preparing,
            )
            .map_err(err)?;
            assert_eq!(equivalent.key(), key);
            let equivalent = equivalent
                .execute(&mut Budget::new(limits), &mut SourceAdmission::default())
                .map_err(err)?;
            assert_eq!(
                equivalent.reply().facts().ok_or("reordered facts")?,
                &expected_native
            );
            let value =
                keyed::request_to_value(&issued, &mut codec, &mut preparing).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut preparing).map_err(err)?;
            // The receiver is given only canonical bytes, its selected profile, and
            // a fresh empty store/admission. It receives no original request proof.
            let mut fresh = SourceAdmission::default();
            let mut receiver =
                FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
            let mut transport = budget();
            let packet = nepl3_wire::decode(&bytes, &mut transport).map_err(err)?;
            let raw = keyed::request_decode(&packet, profile, &mut receiver, &mut transport)
                .map_err(err)?;
            assert_eq!(raw.key, key);
            let request = keyed::prepare_received(&raw, profile, &mut receiver, &mut transport)
                .map_err(err)?;
            let mut executing = Budget::new(limits);
            let bound = request
                .execute(&mut executing, &mut SourceAdmission::default())
                .map_err(err)?;
            let source = raw.tree.bundle.sources[0].reference();
            let analysis = bound
                .for_source(&key, &source, &mut budget())
                .map_err(err)?;
            assert_eq!(analysis.facts().analysis_id, "keyed");
            assert_eq!(*analysis.facts(), expected_native);
            let reply_value =
                keyed::reply_to_value(&bound, profile.registry(), &mut receiver, &mut transport)
                    .map_err(err)?;
            let reply_bytes = nepl3_wire::encode(&reply_value, &mut transport).map_err(err)?;
            let reply_packet = nepl3_wire::decode(&reply_bytes, &mut transport).map_err(err)?;
            let received =
                keyed::reply_decode(&reply_packet, &request, &mut receiver, &mut transport)
                    .map_err(err)?;
            assert_eq!(received.key, key);
            let DecodedBindingOutcome::Complete(result) = received.reply.outcome else {
                return Err("complete".into());
            };
            assert_eq!(result.facts, *analysis.facts());
            assert_eq!(received.reply.report, bound.reply().report);
            let mut stale = key;
            stale.profile_digest = Digest::of(b"different selected profile");
            assert!(matches!(
                bound.for_source(&stale, &source, &mut budget()),
                Err(BindingAccessError::StaleAnalysis)
            ));
            let mut revision = source.clone();
            revision.revision += 1;
            assert!(matches!(
                bound.for_source(&key, &revision, &mut budget()),
                Err(BindingAccessError::MissingSource)
            ));
            let mut smaller = limits;
            smaller.work -= 1;
            assert!(matches!(
                request.execute(&mut Budget::new(smaller), &mut SourceAdmission::default()),
                Err(BindingAccessError::LimitsMismatch)
            ));
            let mut cancelled = Budget::new(limits);
            cancelled.cancel();
            let stopped = request
                .execute(&mut cancelled, &mut SourceAdmission::default())
                .map_err(err)?;
            assert!(matches!(
                stopped.reply().outcome,
                BindingOutcome::Stopped {
                    reason: StopReason::Cancelled,
                    ..
                }
            ));
            assert_eq!(stopped.reply().report.usage, cancelled.usage());
            let stopped_value =
                keyed::reply_to_value(&stopped, profile.registry(), &mut receiver, &mut transport)
                    .map_err(err)?;
            let stopped_received =
                keyed::reply_decode(&stopped_value, &request, &mut receiver, &mut transport)
                    .map_err(err)?;
            assert!(matches!(
                stopped_received.reply.outcome,
                DecodedBindingOutcome::Stopped {
                    reason: StopReason::Cancelled,
                    ..
                }
            ));
            for changed_field in [0, 3, 4] {
                let mut changed = packet.clone();
                let NdfValue::Record(request) = &mut changed else {
                    return Err("request".into());
                };
                match changed_field {
                    0 => request.fields[0] = NdfValue::Text("other-analysis".into()),
                    3 => {
                        let NdfValue::Record(limits) = &mut request.fields[3] else {
                            return Err("limits".into());
                        };
                        limits.fields[1] = NdfValue::U64(1);
                    }
                    _ => {
                        let NdfValue::Record(key) = &mut request.fields[4] else {
                            return Err("key".into());
                        };
                        key.fields[0] = NdfValue::Bytes(vec![0; 32]);
                    }
                }
                profile
                    .registry()
                    .validate(
                        &nepl3_core::schema::TypeDescriptor::Named(nepl3_core::schema::TypeRef {
                            package: "nepl3.engine".into(),
                            revision: 1,
                            name: "BindingRequest".into(),
                        }),
                        &changed,
                        &mut budget(),
                    )
                    .map_err(err)?;
                assert!(
                    keyed::request_decode(&changed, profile, &mut receiver, &mut budget()).is_err()
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn preparation_binds_conditions_and_uri_and_preserves_typed_stops() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let limits = budget().limits();
        let original = keyed::prepare(
            "same",
            tree.tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?
        .key();
        let renamed = keyed::prepare(
            "other",
            tree.tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?
        .key();
        assert_eq!(renamed.tree_digest, original.tree_digest);
        assert_ne!(renamed.request_digest, original.request_digest);
        let mut smaller = limits;
        smaller.work -= 1;
        assert_ne!(
            keyed::prepare(
                "same",
                tree.tree(),
                BindingOptions,
                smaller,
                profile,
                &mut codec,
                &mut budget()
            )
            .map_err(err)?
            .key()
            .request_digest,
            original.request_digest
        );
        let mut modified = tree.tree().clone();
        let old = &modified.bundle.sources[0];
        let changed = SourceSnapshot::new(
            old.identity().source.clone(),
            old.identity().revision,
            "memory:other-uri".into(),
            old.text().as_bytes().to_vec(),
            &mut budget(),
        )
        .map_err(err)?;
        modified.bundle.sources[0] = changed;
        let mut fresh = SourceAdmission::default();
        let mut other =
            FoundationCodec::new(profile.registry(), &empty, &mut fresh).map_err(err)?;
        let changed = keyed::prepare(
            "same",
            &modified,
            BindingOptions,
            limits,
            profile,
            &mut other,
            &mut budget(),
        )
        .map_err(err)?
        .key();
        assert_ne!(changed.tree_digest, original.tree_digest);
        for mode in 0..6 {
            let mut cap = limits;
            let reason = match mode {
                0 => {
                    cap.work = 0;
                    StopReason::WorkLimit
                }
                1 => {
                    cap.source_bytes = 0;
                    StopReason::SourceLimit
                }
                2 => {
                    cap.allocation_units = 0;
                    StopReason::AllocationLimit
                }
                3 => {
                    cap.nodes = 0;
                    StopReason::NodeLimit
                }
                4 => {
                    cap.depth = 0;
                    StopReason::DepthLimit
                }
                _ => StopReason::Cancelled,
            };
            let mut b = Budget::new(cap);
            if mode == 5 {
                b.cancel();
            }
            let mut a = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            assert!(
                matches!(keyed::prepare("same",tree.tree(),BindingOptions,limits,profile,&mut codec,&mut b),Err(nepl3_engine::portable::PortableError::Stopped(r)) if r==reason)
            );
            assert_eq!(b.poll(), Err(reason));
        }
        Ok(())
    })
}

#[test]
fn selected_profile_provider_resource_and_execution_changes_invalidate_bound_results()
-> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let limits = budget().limits();
        let request = keyed::prepare(
            "same",
            tree.tree(),
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let original = request.key();
        let bound = request
            .execute(&mut Budget::new(limits), &mut SourceAdmission::default())
            .map_err(err)?;
        let source = tree.tree().bundle.sources[0].reference();
        for mode in 0..4 {
            let mut selected = profile.profile().clone();
            let mut package = compiled.package.clone();
            let mut resources = vec![];
            match mode {
                0 => selected.id = "different profile".into(),
                1 => {
                    // The alternate catalog names real test implementation bytes.
                    for provider in &mut selected.providers {
                        provider.implementation_digest = Digest::of(include_bytes!("analysis.rs"));
                    }
                }
                2 => {
                    resources.push(ResourceSnapshot {
                        id: "explicit-resource".into(),
                        bytes: b"resource-v2".to_vec(),
                    });
                    selected.resources.push(ResourceIdentity {
                        id: "explicit-resource".into(),
                        digest: Digest::of(b"resource-v2"),
                    });
                }
                _ => package.forms.swap(0, 1),
            }
            let providers: Vec<_> = selected
                .providers
                .iter()
                .map(|p| ProviderImplementation {
                    provider: p.provider.clone(),
                    revision: p.revision,
                    implementation_digest: p.implementation_digest,
                    operations: vec![p.operation.clone()],
                })
                .collect();
            let packages = [&package];
            let changed = selected
                .resolve(
                    &RuntimeCatalog {
                        packages: &packages,
                        providers: &providers,
                        resources: &resources,
                    },
                    profile.registry(),
                    &mut budget(),
                )
                .map_err(err)?;
            let mut input = tree.tree().clone();
            input.profile_digest = changed.digest();
            if mode == 3 {
                assert_eq!(changed.digest(), profile.digest());
                for context in &mut input.contexts {
                    for selection in &mut context.nodes {
                        selection.execution_digest = changed
                            .execution_digest(&selection.entry.alias, &mut budget())
                            .map_err(err)?;
                        if let nepl3_engine::selection::ShapeSelection::Form { index } =
                            &mut selection.shape
                            && *index < 2
                        {
                            *index = 1 - *index;
                        }
                    }
                }
            }
            let mut a = SourceAdmission::default();
            let mut other =
                FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            let prepared = keyed::prepare(
                "same",
                &input,
                BindingOptions,
                limits,
                &changed,
                &mut other,
                &mut budget(),
            )
            .map_err(err)?;
            let key = prepared.key();
            assert_eq!(key.request_digest, original.request_digest);
            if mode == 3 {
                assert_ne!(key.execution_digest, original.execution_digest);
            } else {
                assert_ne!(key.profile_digest, original.profile_digest);
            }
            assert!(matches!(
                bound.for_source(&key, &source, &mut budget()),
                Err(BindingAccessError::StaleAnalysis)
            ));
        }
        // Explicitly sharing the preparation operation's admission and Budget
        // must not count the same input snapshots again during execution.
        let mut shared_budget = Budget::new(limits);
        let mut shared_admission = SourceAdmission::default();
        let shared_request = {
            let mut shared_codec =
                FoundationCodec::new(profile.registry(), &empty, &mut shared_admission)
                    .map_err(err)?;
            keyed::prepare(
                "shared-pipeline",
                tree.tree(),
                BindingOptions,
                limits,
                profile,
                &mut shared_codec,
                &mut shared_budget,
            )
            .map_err(err)?
        };
        let admitted_bytes = shared_budget.usage().source_bytes;
        let shared_result = shared_request
            .execute(&mut shared_budget, &mut shared_admission)
            .map_err(err)?;
        assert!(matches!(
            shared_result.reply().outcome,
            BindingOutcome::Complete(_)
        ));
        assert_eq!(shared_budget.usage().source_bytes, admitted_bytes);
        let mut too_large = limits;
        too_large.work += 1;
        assert!(
            keyed::prepare(
                "same",
                tree.tree(),
                BindingOptions,
                too_large,
                profile,
                &mut codec,
                &mut budget()
            )
            .is_err()
        );
        let long_id = "x".repeat(100_000);
        let mut low = limits;
        low.work = 10_000;
        let mut trial = Budget::new(low);
        assert!(matches!(
            keyed::prepare(
                &long_id,
                tree.tree(),
                BindingOptions,
                limits,
                profile,
                &mut codec,
                &mut trial
            ),
            Err(nepl3_engine::portable::PortableError::Stopped(
                StopReason::WorkLimit
            ))
        ));
        // The long identity itself must not be copied before its Work admission.
        assert!(trial.usage().allocation_units < 100_000);
        // The tree's explicitly carried environment also participates in its key.
        let mut input = tree.tree().clone();
        let environment = input.bundle.environments.first_mut().ok_or("environment")?;
        environment
            .value
            .resources
            .push(nepl3_core::syntax::ResourceContent {
                id: "environment-resource".into(),
                digest: Digest::of(b"environment-v2"),
                bytes: b"environment-v2".to_vec(),
            });
        environment.digest = nepl3_wire::environment::environment_digest(
            &environment.value,
            profile
                .registry()
                .selected("nepl3.foundation", 1)
                .ok_or("foundation")?,
            profile.registry(),
            &mut budget(),
        )
        .map_err(err)?;
        let changed = keyed::prepare(
            "same",
            &input,
            BindingOptions,
            limits,
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?
        .key();
        assert_ne!(changed.tree_digest, original.tree_digest);
        assert!(matches!(
            bound.for_source(&changed, &source, &mut budget()),
            Err(BindingAccessError::StaleAnalysis)
        ));
        Ok(())
    })
}
