//! Owned native contexts and symbolic catalog boundaries do not grant execution.
use super::*;
use compile::{NamedView, ReaderImport, profile::CompileProfile};
use nepl3_core::value::{KindRef, NdfValue, OperationRef, SchemaRef};
use nepl3_engine::{package::ExtensionRequirement, portable::package as engine_codec};
use nepl3_reader::{
    plan::{ProviderKind, ProviderSignature},
    portable::plan as reader_codec,
};
use nepl3_wire::foundation::FoundationCodec;

fn error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    let mut b = budget();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b).map_err(error)?,
        nepl3_reader::schema::descriptor(&mut b).map_err(error)?,
        nepl3_engine::schema::descriptor(&mut b).map_err(error)?,
        nepl3_grammar_core::schema::descriptor(&mut b).map_err(error)?,
    ] {
        let reference = descriptor.reference(&mut b).map_err(error)?;
        registry
            .register(reference, descriptor, &mut b)
            .map_err(error)?;
    }
    registry.finalize(&mut b).map_err(error)?;
    Ok(registry)
}
fn profile() -> CompileProfile {
    let symbolic = SchemaRef {
        package: "not.registered".into(),
        revision: 37,
        digest: Digest::of(b"symbolic"),
    };
    let operation = OperationRef {
        schema: symbolic.clone(),
        name: "unknown".into(),
    };
    CompileProfile {
        package: "test.owned".into(),
        state_type: TypeDescriptor::Unit,
        reader_imports: vec![ReaderImport {
            provider: "unused.reader".into(),
            signature: ProviderSignature {
                operation: operation.clone(),
                kind: ProviderKind::Read,
                value_input: TypeDescriptor::Named(TypeRef {
                    package: "not.registered".into(),
                    revision: 37,
                    name: "OpaqueInput".into(),
                }),
                value_output: TypeDescriptor::Text,
                pure: true,
                state_type: TypeDescriptor::Unit,
                continuation_type: TypeDescriptor::Unit,
            },
        }],
        extensions: vec![ExtensionRequirement {
            alias: "original.alias".into(),
            provider: "unused.extension".into(),
            signature: "v1".into(),
            operation,
            input: TypeDescriptor::Named(TypeRef {
                package: "not.registered".into(),
                revision: 37,
                name: "OpaqueInput".into(),
            }),
            output: TypeDescriptor::Unit,
            pure: false,
        }],
        classes: vec![NamedClass {
            name: "unused.class".into(),
            class: PresentationClass {
                schema: symbolic.clone(),
                name: "unknown".into(),
                fallback: FallbackRole::Content,
            },
        }],
        views: vec![NamedView {
            name: "unused.view".into(),
            kind: KindRef {
                schema: symbolic,
                local_kind: u64::MAX,
            },
        }],
    }
}
#[test]
fn owned_context_borrows_all_six_fields_without_rewriting() {
    let profile = profile();
    let context = profile.as_context();
    assert_eq!(context.package, profile.package);
    assert!(core::ptr::eq(
        context.package.as_ptr(),
        profile.package.as_ptr()
    ));
    assert!(core::ptr::eq(context.state_type, &profile.state_type));
    assert!(core::ptr::eq(
        context.reader_imports,
        profile.reader_imports.as_slice()
    ));
    assert!(core::ptr::eq(
        context.extensions,
        profile.extensions.as_slice()
    ));
    assert!(core::ptr::eq(context.classes, profile.classes.as_slice()));
    assert!(core::ptr::eq(context.views, profile.views.as_slice()));
    assert_eq!(context.extensions[0].alias, "original.alias");
}
#[test]
fn symbolic_catalog_contracts_survive_cold_cbor_without_resolution() -> Result<(), String> {
    let registry = registry()?;
    let profile = profile();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let signature = &profile.reader_imports[0].signature;
    let extension = &profile.extensions[0];
    assert!(signature.check(&registry, &mut budget()).is_err());
    let signature_value =
        reader_codec::signature_to_value(signature, &registry, &mut codec, &mut budget())
            .map_err(error)?;
    let extension_value =
        engine_codec::extension_to_value(extension, &registry, &mut codec, &mut budget())
            .map_err(error)?;
    for (value, name) in [
        (&signature_value, "ProviderSignature"),
        (&extension_value, "ExtensionRequirement"),
    ] {
        let NdfValue::Record(record) = value else {
            return Err("record".into());
        };
        assert_eq!(record.kind, name);
        assert_eq!(record.fields.len(), 7);
    }
    let signature_bytes = nepl3_wire::encode(&signature_value, &mut budget()).map_err(error)?;
    let extension_bytes = nepl3_wire::encode(&extension_value, &mut budget()).map_err(error)?;
    let receiver_registry = self::registry()?;
    let receiver_store = SourceStore::default();
    let mut receiver_admission = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(&receiver_registry, &receiver_store, &mut receiver_admission)
            .map_err(error)?;
    let signature_value = nepl3_wire::decode(&signature_bytes, &mut budget()).map_err(error)?;
    let extension_value = nepl3_wire::decode(&extension_bytes, &mut budget()).map_err(error)?;
    assert_eq!(
        reader_codec::signature_from_value(
            &signature_value,
            &receiver_registry,
            &mut receiver,
            &mut budget()
        )
        .map_err(error)?,
        *signature
    );
    assert_eq!(
        engine_codec::extension_from_value(
            &extension_value,
            &receiver_registry,
            &mut receiver,
            &mut budget()
        )
        .map_err(error)?,
        *extension
    );
    assert!(receiver_registry.selected("not.registered", 37).is_none());
    Ok(())
}
#[test]
fn catalog_boundaries_reject_wrong_identity_and_preserve_stops() -> Result<(), String> {
    let registry = registry()?;
    let profile = profile();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let mut signature = reader_codec::signature_to_value(
        &profile.reader_imports[0].signature,
        &registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(error)?;
    let mut extension = engine_codec::extension_to_value(
        &profile.extensions[0],
        &registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(error)?;
    let mut zero = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    });
    assert!(matches!(
        reader_codec::signature_from_value(&signature, &registry, &mut codec, &mut zero),
        Err(nepl3_reader::portable::PortableError::Stopped(_))
    ));
    let mut zero = Budget::new(Limits {
        work: 0,
        ..Limits::default()
    });
    assert!(matches!(
        engine_codec::extension_from_value(&extension, &registry, &mut codec, &mut zero),
        Err(nepl3_engine::portable::PortableError::Stopped(_))
    ));
    if let NdfValue::Record(record) = &mut signature {
        record.schema.digest = Digest::of(b"forged");
    }
    if let NdfValue::Record(record) = &mut extension {
        record.fields.pop();
    }
    assert!(
        reader_codec::signature_from_value(&signature, &registry, &mut codec, &mut budget())
            .is_err()
    );
    assert!(
        engine_codec::extension_from_value(&extension, &registry, &mut codec, &mut budget())
            .is_err()
    );
    Ok(())
}

#[test]
fn borrowed_profile_preserves_native_success_errors_and_budget_usage() -> Result<(), String> {
    let doc = load("conformance/fixtures/grammar/diagnostic/valid.neplg")?;
    let checked = doc
        .validate(&mut budget(), &mut SourceAdmission::default())
        .map_err(error)?;
    for case in 0..4 {
        let mut profile = profile();
        // Deliberately unused symbolic catalogs are accepted by the native compiler.
        if case == 1 {
            profile.reader_imports[0].provider.clear();
        }
        if case == 2 {
            profile.extensions.push(profile.extensions[0].clone());
        }
        let manual = PackageContext {
            package: &profile.package,
            state_type: &profile.state_type,
            reader_imports: &profile.reader_imports,
            extensions: &profile.extensions,
            classes: &profile.classes,
            views: &profile.views,
        };
        let mut left_budget = budget();
        let mut right_budget = budget();
        if case == 3 {
            left_budget = Budget::new(Limits {
                work: 0,
                ..Limits::default()
            });
            right_budget = Budget::new(Limits {
                work: 0,
                ..Limits::default()
            });
        }
        let left = compile::package::compile(&checked, &manual, registry()?, &mut left_budget);
        let right = compile::package::compile(
            &checked,
            &profile.as_context(),
            registry()?,
            &mut right_budget,
        );
        assert_eq!(left_budget.usage(), right_budget.usage());
        match (left, right) {
            (Ok(left), Ok(right)) => {
                assert_eq!(case, 0);
                assert_eq!(left.package, right.package);
                assert_eq!(
                    left.registry.selected("test.owned", 1),
                    right.registry.selected("test.owned", 1)
                );
            }
            (Err(left), Err(right)) => {
                assert_ne!(case, 0);
                match case {
                    1 => assert_eq!(
                        left.cause(),
                        &compile::CompileError::InvalidCatalogName(compile::Catalog::Imports)
                    ),
                    2 => assert!(matches!(
                        left.cause(),
                        compile::CompileError::Declaration {
                            reason: compile::DeclarationError::DuplicateName,
                            ..
                        }
                    )),
                    3 => assert_eq!(
                        left.cause(),
                        &compile::CompileError::Stopped(StopReason::WorkLimit)
                    ),
                    _ => return Err("unexpected rejection".into()),
                }
                assert_eq!(left, right);
            }
            _ => return Err("owned/native result differs".into()),
        }
    }
    Ok(())
}

#[test]
fn symbolic_catalog_encode_decode_allocation_stops_are_typed() -> Result<(), String> {
    let registry = registry()?;
    let profile = profile();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let signature = reader_codec::signature_to_value(
        &profile.reader_imports[0].signature,
        &registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(error)?;
    let extension = engine_codec::extension_to_value(
        &profile.extensions[0],
        &registry,
        &mut codec,
        &mut budget(),
    )
    .map_err(error)?;
    let limited = || {
        Budget::new(Limits {
            allocation_units: 0,
            ..budget().limits()
        })
    };
    assert!(matches!(
        reader_codec::signature_to_value(
            &profile.reader_imports[0].signature,
            &registry,
            &mut codec,
            &mut limited()
        ),
        Err(nepl3_reader::portable::PortableError::Stopped(
            StopReason::AllocationLimit
        ))
    ));
    assert!(matches!(
        reader_codec::signature_from_value(&signature, &registry, &mut codec, &mut limited()),
        Err(nepl3_reader::portable::PortableError::Stopped(
            StopReason::AllocationLimit
        ))
    ));
    assert!(matches!(
        engine_codec::extension_to_value(
            &profile.extensions[0],
            &registry,
            &mut codec,
            &mut limited()
        ),
        Err(nepl3_engine::portable::PortableError::Stopped(
            StopReason::AllocationLimit
        ))
    ));
    assert!(matches!(
        engine_codec::extension_from_value(&extension, &registry, &mut codec, &mut limited()),
        Err(nepl3_engine::portable::PortableError::Stopped(
            StopReason::AllocationLimit
        ))
    ));
    for mutate_identity in [false, true] {
        let mut signature = signature.clone();
        let mut extension = extension.clone();
        for value in [&mut signature, &mut extension] {
            let NdfValue::Record(record) = value else {
                return Err("record".into());
            };
            if mutate_identity {
                record.schema.digest = Digest::of(b"forged");
            } else {
                record.fields.pop();
            }
        }
        assert!(
            reader_codec::signature_from_value(&signature, &registry, &mut codec, &mut budget())
                .is_err()
        );
        assert!(
            engine_codec::extension_from_value(&extension, &registry, &mut codec, &mut budget())
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn complete_profile_cold_roundtrip_preserves_order_and_native_compilation() -> Result<(), String> {
    use compile::profile::portable;
    let registry = registry()?;
    let mut profile = profile();
    // Multiple unsorted entries make reordering detectable.
    let mut second = profile.reader_imports[0].clone();
    second.provider = "aaa.reader".into();
    profile.reader_imports.push(second);
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let value =
        portable::to_value(&profile, &registry, &mut codec, &mut budget()).map_err(error)?;
    let NdfValue::Record(record) = &value else {
        return Err("profile record".into());
    };
    assert_eq!(record.kind, "CompileProfile");
    assert_eq!(
        record.schema,
        *registry.selected("nepl3.grammar", 1).ok_or("grammar")?
    );
    assert_eq!(record.fields.len(), 6);
    assert_eq!(record.fields[0], NdfValue::Text(profile.package.clone()));
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
    let receiver_registry = self::registry()?;
    let receiver_store = SourceStore::default();
    let mut receiver_admission = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(&receiver_registry, &receiver_store, &mut receiver_admission)
            .map_err(error)?;
    let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let restored =
        portable::from_value(&received, &receiver_registry, &mut receiver, &mut budget())
            .map_err(error)?;
    assert_eq!(restored, profile);
    assert_eq!(
        portable::to_value(&restored, &receiver_registry, &mut receiver, &mut budget())
            .map_err(error)?,
        value
    );
    let doc = load("conformance/fixtures/grammar/diagnostic/valid.neplg")?;
    let checked = doc
        .validate(&mut budget(), &mut SourceAdmission::default())
        .map_err(error)?;
    let before =
        compile::package::compile(&checked, &profile.as_context(), registry, &mut budget())
            .map_err(error)?;
    let after = compile::package::compile(
        &checked,
        &restored.as_context(),
        receiver_registry,
        &mut budget(),
    )
    .map_err(error)?;
    assert_eq!(before.package, after.package);
    Ok(())
}
#[test]
fn profile_codec_rejects_structural_forgery_but_preserves_invalid_catalog_data()
-> Result<(), String> {
    use compile::profile::portable;
    let registry = registry()?;
    let mut profile = profile();
    profile.reader_imports[0].provider.clear();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let value =
        portable::to_value(&profile, &registry, &mut codec, &mut budget()).map_err(error)?;
    assert_eq!(
        portable::from_value(&value, &registry, &mut codec, &mut budget()).map_err(error)?,
        profile
    );
    for case in 0..4 {
        let mut bad = value.clone();
        let NdfValue::Record(record) = &mut bad else {
            return Err("profile record".into());
        };
        match case {
            0 => record.schema.digest = Digest::of(b"forged"),
            1 => {
                record.fields.pop();
            }
            2 => record.fields[2] = NdfValue::U64(0),
            _ => {
                let NdfValue::List(imports) = &mut record.fields[2] else {
                    return Err("imports".into());
                };
                let NdfValue::Record(import) = &mut imports[0] else {
                    return Err("import".into());
                };
                import.fields[0] = NdfValue::U64(0);
            }
        }
        assert!(portable::from_value(&bad, &registry, &mut codec, &mut budget()).is_err());
    }
    Ok(())
}
#[test]
fn profile_codec_preserves_each_resource_stop_and_source_neutrality() -> Result<(), String> {
    use compile::profile::portable::{self, ProfileError};
    let registry = registry()?;
    let profile = profile();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let mut encode_budget = budget();
    let value =
        portable::to_value(&profile, &registry, &mut codec, &mut encode_budget).map_err(error)?;
    let mut decode_budget = budget();
    portable::from_value(&value, &registry, &mut codec, &mut decode_budget).map_err(error)?;
    for usage in [encode_budget.usage(), decode_budget.usage()] {
        assert_eq!(usage.source_bytes, 0);
        assert_eq!(usage.output_bytes, 0);
    }
    for encode in [false, true] {
        let usage = if encode {
            encode_budget.usage()
        } else {
            decode_budget.usage()
        };
        let exact = Limits {
            work: usage.work,
            allocation_units: usage.allocation_units,
            nodes: usage.nodes,
            depth: usage.depth,
            ..Limits::default()
        };
        let mut b = Budget::new(exact);
        if encode {
            portable::to_value(&profile, &registry, &mut codec, &mut b).map_err(error)?;
        } else {
            portable::from_value(&value, &registry, &mut codec, &mut b).map_err(error)?;
        }
        assert_eq!(b.usage(), usage);
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
        ] {
            let mut limits = exact;
            match reason {
                StopReason::WorkLimit => {
                    assert!(limits.work > 0);
                    limits.work -= 1;
                }
                StopReason::AllocationLimit => {
                    assert!(limits.allocation_units > 0);
                    limits.allocation_units -= 1;
                }
                StopReason::NodeLimit => {
                    assert!(limits.nodes > 0);
                    limits.nodes -= 1;
                }
                StopReason::DepthLimit => {
                    assert!(limits.depth > 0);
                    limits.depth -= 1;
                }
                _ => {}
            }
            let mut b = Budget::new(limits);
            let result = if encode {
                portable::to_value(&profile, &registry, &mut codec, &mut b).map(|_| ())
            } else {
                portable::from_value(&value, &registry, &mut codec, &mut b).map(|_| ())
            };
            assert!(
                matches!(result, Err(ProfileError::Stopped(actual)) if actual == reason),
                "one-short {reason:?}: {result:?}"
            );
        }
    }
    for encode in [false, true] {
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::NodeLimit => limits.nodes = 0,
                StopReason::DepthLimit => limits.depth = 0,
                _ => {}
            }
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            let result = if encode {
                portable::to_value(&profile, &registry, &mut codec, &mut b).map(|_| ())
            } else {
                portable::from_value(&value, &registry, &mut codec, &mut b).map(|_| ())
            };
            assert!(
                matches!(result, Err(ProfileError::Stopped(actual)) if actual == reason),
                "{reason:?}: {result:?}"
            );
        }
    }
    Ok(())
}
