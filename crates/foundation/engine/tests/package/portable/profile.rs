use super::*;
use nepl3_engine::portable::profile as exchange;

#[test]
fn profile_wire_preserves_requirements_and_checks_independent_catalog() -> TestResult {
    let (package, registry) = fixture()?;
    let mut profile = super::profile(&package, &registry)?;
    let operation = OperationRef {
        schema: package.schema.clone(),
        name: "facts".into(),
    };
    let engine = registry
        .selected("nepl3.engine", 1)
        .ok_or("engine")?
        .clone();
    let head = nepl3_engine::selection::HeadProviderRef {
        shape: OperationRef {
            schema: engine.clone(),
            name: "headShape".into(),
        },
        child_context: OperationRef {
            schema: engine.clone(),
            name: "headChildContext".into(),
        },
    };
    let provider = ProviderImplementation {
        provider: "external".into(),
        revision: 3,
        implementation_digest: Digest::of(b"independent provider manifest"),
        operations: vec![
            operation.clone(),
            head.shape.clone(),
            head.child_context.clone(),
        ],
    };
    let resource = ResourceSnapshot {
        id: "style".into(),
        bytes: b"independent resource bytes".to_vec(),
    };
    profile.providers.push(ProviderRequirement {
        provider: provider.provider.clone(),
        revision: provider.revision,
        implementation_digest: provider.implementation_digest,
        operation: operation.clone(),
    });
    profile.allowlist.push(operation);
    profile.schemas.push(engine);
    profile.schemas.push(
        registry
            .selected("nepl3.reader", 1)
            .ok_or("reader")?
            .clone(),
    );
    for operation in [&head.shape, &head.child_context] {
        profile.providers.push(ProviderRequirement {
            provider: provider.provider.clone(),
            revision: provider.revision,
            implementation_digest: provider.implementation_digest,
            operation: operation.clone(),
        });
        profile.allowlist.push(operation.clone());
    }
    profile.head_providers.push(HeadRegistration {
        alias: "Host".into(),
        category: "Expr".into(),
        provider: head,
    });
    profile.resources.push(ResourceIdentity {
        id: resource.id.clone(),
        digest: Digest::of(&resource.bytes),
    });
    profile.category_modes.push(CategoryMode {
        alias: "Guest".into(),
        category: "Expr".into(),
        mode: "Code".into(),
    });
    let packages = [&package];
    let providers = [provider];
    let resources = [resource];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &providers,
        resources: &resources,
    };
    let resolved = profile
        .resolve(&catalog, &registry, &mut budget())
        .map_err(err)?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let value = exchange::to_value(&resolved, &mut codec, &mut budget()).map_err(err)?;
    // Wire field order is the published interfaces/engine.json record order.
    let NdfValue::Record(record) = &value else {
        return Err("record".into());
    };
    assert_eq!(record.kind, "ParseProfile");
    assert_eq!(record.fields.len(), 9);
    assert_eq!(record.fields[0], NdfValue::Text("portable-tree".into()));
    let NdfValue::List(languages) = &record.fields[1] else {
        return Err("languages".into());
    };
    let NdfValue::Record(host) = &languages[0] else {
        return Err("host".into());
    };
    assert_eq!(host.kind, "LanguageRegistration");
    assert_eq!(host.fields[0], NdfValue::Text("Host".into()));
    assert_eq!(host.fields[2], NdfValue::Text("Expr".into()));
    let NdfValue::List(heads) = &record.fields[4] else {
        return Err("heads".into());
    };
    let NdfValue::Record(head) = &heads[0] else {
        return Err("head".into());
    };
    assert_eq!(head.kind, "HeadRegistration");
    assert_eq!(head.fields[0], NdfValue::Text("Host".into()));
    let NdfValue::Record(limits) = &record.fields[8] else {
        return Err("limits".into());
    };
    assert_eq!(limits.fields[1], NdfValue::U64(profile.limits.work));
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
    let decoded = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
    let received = exchange::from_value(&decoded, &catalog, &registry, &mut codec, &mut budget())
        .map_err(err)?;
    assert_eq!(received, profile);
    assert_eq!(
        received
            .resolve(&catalog, &registry, &mut budget())
            .map_err(err)?
            .digest(),
        resolved.digest()
    );

    for catalog in [
        RuntimeCatalog {
            packages: &[],
            providers: &providers,
            resources: &resources,
        },
        RuntimeCatalog {
            packages: &packages,
            providers: &[],
            resources: &resources,
        },
        RuntimeCatalog {
            packages: &packages,
            providers: &providers,
            resources: &[],
        },
    ] {
        assert!(matches!(
            exchange::from_value(&value, &catalog, &registry, &mut codec, &mut budget()),
            Err(PortableError::Profile(_))
        ));
    }
    let mut changed_provider = providers[0].clone();
    changed_provider.implementation_digest = Digest::of(b"different implementation");
    let wrong_providers = [changed_provider];
    assert!(matches!(
        exchange::from_value(
            &value,
            &RuntimeCatalog {
                packages: &packages,
                providers: &wrong_providers,
                resources: &resources
            },
            &registry,
            &mut codec,
            &mut budget()
        ),
        Err(PortableError::Profile(ProfileError::ProviderIdentity))
    ));
    let wrong_resources = [ResourceSnapshot {
        id: "style".into(),
        bytes: b"changed".to_vec(),
    }];
    assert!(matches!(
        exchange::from_value(
            &value,
            &RuntimeCatalog {
                packages: &packages,
                providers: &providers,
                resources: &wrong_resources
            },
            &registry,
            &mut codec,
            &mut budget()
        ),
        Err(PortableError::Profile(ProfileError::ResourceIdentity))
    ));
    Ok(())
}

#[test]
fn profile_wire_rejects_shape_identity_and_budget_violations() -> TestResult {
    let (package, registry) = fixture()?;
    let profile = super::profile(&package, &registry)?;
    let packages = [&package];
    let catalog = RuntimeCatalog {
        packages: &packages,
        providers: &[],
        resources: &[],
    };
    let resolved = profile
        .resolve(&catalog, &registry, &mut budget())
        .map_err(err)?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(err)?;
    let original = exchange::to_value(&resolved, &mut codec, &mut budget()).map_err(err)?;
    for case in 0..5 {
        let mut value = original.clone();
        let NdfValue::Record(record) = &mut value else {
            return Err("record".into());
        };
        match case {
            0 => {
                record.fields.pop();
            }
            1 => record.schema.digest = Digest::of(b"wrong schema"),
            2 => record.fields[0] = NdfValue::Text(String::new()),
            3 => {
                let NdfValue::List(languages) = &mut record.fields[1] else {
                    return Err("languages".into());
                };
                languages.push(languages[0].clone());
            }
            _ => {
                let NdfValue::List(languages) = &mut record.fields[1] else {
                    return Err("languages".into());
                };
                let NdfValue::Record(language) = &mut languages[0] else {
                    return Err("language".into());
                };
                let NdfValue::Record(identity) = &mut language.fields[1] else {
                    return Err("identity".into());
                };
                identity.fields[1] = NdfValue::Bytes(vec![0; 32]);
            }
        }
        assert!(
            exchange::from_value(&value, &catalog, &registry, &mut codec, &mut budget()).is_err(),
            "case {case}"
        );
    }
    for resource in ["work", "allocation", "cancel"] {
        let mut limits = budget().limits();
        if resource == "work" {
            limits.work = 0;
        }
        if resource == "allocation" {
            limits.allocation_units = 0;
        }
        let mut limited = nepl3_core::budget::Budget::new(limits);
        if resource == "cancel" {
            limited.cancel();
        }
        assert!(matches!(
            exchange::from_value(&original, &catalog, &registry, &mut codec, &mut limited),
            Err(PortableError::Stopped(_))
        ));
        assert!(limited.poll().is_err());
        assert!(matches!(
            exchange::to_value(&resolved, &mut codec, &mut limited),
            Err(PortableError::Stopped(_))
        ));
    }
    Ok(())
}
