use super::*;
use nepl3_engine::profile::{
    ParseProfile, ProviderImplementation, ProviderRequirement, RuntimeCatalog,
};
use nepl3_suite::profile::{Error, NativeOperations};

fn declared(request: &Invoke, implementation: Digest) -> (ParseProfile, ProviderImplementation) {
    let profile = ParseProfile {
        id: "test.native-operations".into(),
        languages: vec![],
        schemas: vec![request.operation.schema.clone()],
        category_modes: vec![],
        head_providers: vec![],
        providers: vec![ProviderRequirement {
            provider: "test.native".into(),
            revision: 1,
            implementation_digest: implementation,
            operation: request.operation.clone(),
        }],
        allowlist: vec![request.operation.clone()],
        resources: vec![],
        limits: budget().limits(),
    };
    let host = ProviderImplementation {
        provider: "test.native".into(),
        revision: 1,
        implementation_digest: implementation,
        operations: vec![request.operation.clone()],
    };
    (profile, host)
}

fn binding<'a>(
    operation: &'a OperationRef,
    grants: &'a Grants<'a>,
    implementation: Digest,
) -> scheduler::Registration<'a> {
    scheduler::Registration {
        invoke: suspending::Registration {
            operation,
            implementation,
            invoke: &invoke,
        },
        resume: resume::Registration {
            operation,
            implementation,
            resume: &resume,
        },
        grants,
        context: &context,
    }
}

#[test]
fn resolved_profile_runs_recursive_native_operations() -> Result<(), String> {
    let (registry, mut request) = fixture()?;
    if let TypedValue::Record(record) = &mut request.input {
        record.fields[0] = NdfValue::U64(2);
    }
    let implementation = Digest::of(b"profile fixture v1");
    let (profile, host) = declared(&request, implementation);
    let hosts = [host];
    let resolved = profile
        .resolve(
            &RuntimeCatalog {
                packages: &[],
                providers: &hosts,
                resources: &[],
            },
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let bindings = [binding(&request.operation, &grants, implementation)];
    let native =
        NativeOperations::new(&resolved, &bindings, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut awaits = Vec::new();
    let mut cancellations = Vec::new();
    let mut execution = budget();
    execution
        .charge(Resource::Work, 7)
        .map_err(|e| format!("{e:?}"))?;
    let result = native
        .run(
            &request,
            &mut execution,
            &mut budget(),
            |id, _| awaits.push(id),
            |id| cancellations.push(id),
        )
        .map_err(|e| format!("{e:?}"))?;
    let OperationResult::Complete { value, .. } = result else {
        return Err("expected complete recursion".into());
    };
    // Two decrements and two Resume increments preserve the independent input 2.
    assert_eq!(number(&value), Some(2));
    assert_eq!(awaits, [1, 2]);
    assert!(cancellations.is_empty());
    assert!(execution.usage().work > 7);
    Ok(())
}

#[test]
fn native_profile_rejects_unapproved_or_inconsistent_bindings() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let implementation = Digest::of(b"profile fixture v1");
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    for case in 0..4 {
        let (mut profile, host) = declared(&request, implementation);
        if case == 0 {
            profile.allowlist.clear();
        }
        let hosts = [host];
        let resolved = profile
            .resolve(
                &RuntimeCatalog {
                    packages: &[],
                    providers: &hosts,
                    resources: &[],
                },
                &registry,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
        let mut bindings = vec![binding(&request.operation, &grants, implementation)];
        match case {
            1 => {
                bindings[0].invoke.implementation = Digest::of(b"other");
                bindings[0].resume.implementation = Digest::of(b"other");
            }
            2 => {
                bindings[0].resume.implementation = Digest::of(b"other");
            }
            3 => bindings.push(binding(&request.operation, &grants, implementation)),
            _ => {}
        }
        let result = NativeOperations::new(&resolved, &bindings, &mut budget());
        assert!(matches!(
            (case, result),
            (
                0,
                Err(Error::Profile(
                    nepl3_engine::profile::ProfileError::NotAllowed
                ))
            ) | (1, Err(Error::Implementation))
                | (2, Err(Error::Registration))
                | (3, Err(Error::Duplicate))
        ));
    }
    Ok(())
}

#[test]
fn native_profile_enforces_all_resource_ceilings_and_validation_stop() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let implementation = Digest::of(b"profile fixture v1");
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let bindings = [binding(&request.operation, &grants, implementation)];
    for field in 0..8 {
        let (mut profile, host) = declared(&request, implementation);
        let limit = match field {
            0 => &mut profile.limits.source_bytes,
            1 => &mut profile.limits.work,
            2 => &mut profile.limits.depth,
            3 => &mut profile.limits.nodes,
            4 => &mut profile.limits.allocation_units,
            5 => &mut profile.limits.output_bytes,
            6 => &mut profile.limits.diagnostics,
            _ => &mut profile.limits.events,
        };
        *limit -= 1;
        let hosts = [host];
        let resolved = profile
            .resolve(
                &RuntimeCatalog {
                    packages: &[],
                    providers: &hosts,
                    resources: &[],
                },
                &registry,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
        let native = NativeOperations::new(&resolved, &bindings, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        for root_exceeds in [false, true] {
            let mut call = request.clone();
            call.limits = if root_exceeds {
                request.limits
            } else {
                profile.limits
            };
            let mut execution = Budget::new(if root_exceeds {
                profile.limits
            } else {
                budget().limits()
            });
            assert!(matches!(
                native.run(&call, &mut execution, &mut budget(), |_, _| {}, |_| {}),
                Err(Error::Limits)
            ));
            assert_eq!(execution.usage().work, 0);
        }
        let mut validation = budget();
        validation.stop(StopReason::Cancelled);
        assert!(matches!(
            NativeOperations::new(&resolved, &bindings, &mut validation),
            Err(Error::Stopped(StopReason::Cancelled))
        ));
    }
    Ok(())
}

#[test]
fn native_profile_binds_root_children_and_resume_to_profile_and_host() -> Result<(), String> {
    use core::cell::RefCell;
    let (registry, mut request) = fixture()?;
    if let TypedValue::Record(record) = &mut request.input {
        record.fields[0] = NdfValue::U64(1);
    }
    let implementation = Digest::of(b"profile fixture v1");
    let sources = SourceStore::default();
    let grants = Grants::new(&request.environment, &sources, &[], &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut identities = Vec::new();
    for case in 0..3 {
        let (mut profile, host) = declared(&request, implementation);
        if case == 1 {
            profile.id = "test.other-profile".into();
        }
        let host_digest = Digest::of(if case == 2 { b"host B" } else { b"host A" });
        let hosts = [host];
        let resolved = profile
            .resolve(
                &RuntimeCatalog {
                    packages: &[],
                    providers: &hosts,
                    resources: &[],
                },
                &registry,
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?;
        let observed = RefCell::new(Vec::new());
        let on_invoke =
            |call: &Invoke, digest: Digest, registry: &SchemaRegistry, budget: &mut Budget| {
                observed.borrow_mut().push((call.request_id, digest));
                invoke(call, digest, registry, budget)
            };
        let on_resume =
            |call: &Invoke, resumed: &Resume, registry: &SchemaRegistry, budget: &mut Budget| {
                observed
                    .borrow_mut()
                    .push((call.request_id, resumed.continuation.snapshot_digest));
                resume(call, resumed, registry, budget)
            };
        let host_context = |_: &Invoke, _: Digest, budget: &mut Budget| {
            budget.charge(Resource::Work, 1)?;
            Ok(host_digest)
        };
        let mut registration = binding(&request.operation, &grants, implementation);
        registration.invoke.invoke = &on_invoke;
        registration.resume.resume = &on_resume;
        registration.context = &host_context;
        let registrations = [registration];
        let native = NativeOperations::new(&resolved, &registrations, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let result = native
            .run(&request, &mut budget(), &mut budget(), |_, _| {}, |_| {})
            .map_err(|e| format!("{e:?}"))?;
        assert!(matches!(result, OperationResult::Complete { .. }));
        let observed = observed.borrow();
        assert_eq!(observed.len(), 3);
        assert_eq!(observed[0].0, request.request_id);
        assert_eq!(observed[1].0, request.request_id + 1);
        assert_eq!(observed[2].0, request.request_id);
        // Fixed host context in this fixture: all three stages must carry the
        // same scoped identity, including the actual resumed continuation.
        assert_eq!(observed[0].1, observed[1].1);
        assert_eq!(observed[0].1, observed[2].1);
        assert_ne!(observed[0].1, host_digest);
        identities.push(observed[0].1);
    }
    assert_ne!(identities[0], identities[1]);
    assert_ne!(identities[0], identities[2]);
    Ok(())
}
