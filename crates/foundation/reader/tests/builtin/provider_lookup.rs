use super::*;
fn padded_registry(padding: usize) -> Result<SchemaRegistry, SchemaError> {
    let mut registry = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget())?,
        nepl3_reader::schema::descriptor(&mut budget())?,
    ] {
        registry.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    registry.finalize(&mut budget())?;
    Ok(registry)
}
#[test]
fn standard_operation_resolution_precharges_schema_catalogs() -> Result<(), ReaderError> {
    for kind in [
        BuiltinReader::Name,
        BuiltinReader::Number,
        BuiltinReader::Trivia,
    ] {
        for signature in [false, true] {
            let mut usages = vec![];
            for padding in [0, 32] {
                let registry = padded_registry(padding)?;
                let mut b = budget();
                if signature {
                    provider::signature(kind, &registry, &mut b)?;
                } else {
                    provider::operation(kind, &registry, &mut b)?;
                }
                usages.push(b.usage());
            }
            let mut expected = usages[0];
            expected.work += 32 * (10 + 12 + 9);
            assert_eq!(usages[1], expected);
        }
        let mut baseline = budget();
        provider::operation(kind, &padded_registry(0)?, &mut baseline)?;
        let mut limits = budget().limits();
        limits.work = baseline.usage().work;
        let mut stopped = Budget::new(limits);
        assert!(matches!(
            provider::operation(kind, &padded_registry(32)?, &mut stopped),
            Err(ReaderError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        assert_eq!(stopped.usage().allocation_units, 0);
    }
    Ok(())
}
#[test]
fn supported_provider_lookup_stops_before_unknown_schema() {
    let registry = SchemaRegistry::default();
    let mut limits = budget().limits();
    limits.work = 0;
    let mut stopped = Budget::new(limits);
    assert!(matches!(
        provider::operation(BuiltinReader::Name, &registry, &mut stopped),
        Err(ReaderError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(stopped.usage(), Usage::default());
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        provider::operation(BuiltinReader::Name, &registry, &mut cancelled),
        Err(ReaderError::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(cancelled.usage(), Usage::default());
    assert!(matches!(
        provider::operation(BuiltinReader::Name, &registry, &mut budget()),
        Err(ReaderError::Schema(SchemaError::UnknownSchema))
    ));
    assert!(matches!(
        provider::operation(BuiltinReader::Text, &registry, &mut cancelled),
        Err(ReaderError::ProviderContract)
    ));
}

#[test]
fn standard_read_selection_stops_before_state_rejection() -> Result<(), ReaderError> {
    let mut f = Fixture::new("alpha")?;
    f.registry = padded_registry(32)?;
    let op = provider::operation(BuiltinReader::Name, &f.registry, &mut budget())?;
    let mut setup = budget();
    let mut admission = SourceAdmission::default();
    let mut codec =
        nepl3_wire::foundation::FoundationCodec::new(&f.registry, &f.store, &mut admission)
            .map_err(|_| ReaderError::Context)?;
    let context = f
        .context
        .check(&mut codec, &f.store, &f.registry, &mut setup)
        .map_err(|_| ReaderError::Context)?;
    let state = NdfValue::Bool(true);
    let request = || ReadRequest {
        snapshot: &f.source,
        start: 0,
        limit: 5,
        final_input: true,
        context: &context,
        state: &state,
    };
    let mut limits = budget().limits();
    // Admit the dispatch header and lookup entry, but not its first comparison.
    limits.work = (op.name.len() + op.schema.package.len()) as u64 + 33 + 1;
    let mut stopped = Budget::new(limits);
    assert!(matches!(
        provider::read(
            &op,
            request(),
            &f.registry,
            &f.store,
            &mut stopped,
            &mut SourceAdmission::default()
        )?,
        ReadReply::Stopped {
            reason: StopReason::WorkLimit,
            ..
        }
    ));
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    assert_eq!(stopped.usage().allocation_units, 0);
    assert!(matches!(
        provider::read(
            &op,
            request(),
            &f.registry,
            &f.store,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(ReaderError::ProviderContract)
    ));
    Ok(())
}

#[test]
fn failed_builtin_diagnostic_selection_has_independent_catalog_fees() -> Result<(), ReaderError> {
    let mut usages = [[Usage::default(); 2]; 2];
    let mut diagnostics = vec![];
    for (p, padding) in [0, 32].into_iter().enumerate() {
        let mut f = Fixture::new("01")?;
        f.registry = padded_registry(padding)?;
        for (k, kind) in [BuiltinReader::Name, BuiltinReader::Number]
            .into_iter()
            .enumerate()
        {
            let mut b = budget();
            let reply = f.read(kind, 0, 2, true, &mut b, &mut SourceAdmission::default())?;
            if k == 0 {
                assert!(matches!(reply, ReadReply::NoMatch { .. }));
            } else {
                let ReadReply::Failed {
                    diagnostic,
                    recovery,
                    sources,
                    source_maps,
                    report,
                } = reply
                else {
                    return Err(ReaderError::ProviderContract);
                };
                assert_eq!(diagnostic.code, "InvalidNumber");
                assert_eq!(recovery, None);
                assert!(sources.is_empty() && source_maps.is_empty());
                assert_eq!(report.diagnostics, vec![diagnostic.clone()]);
                assert_eq!(report.usage, b.usage());
                assert_eq!(report.usage.diagnostics, 1);
                diagnostics.push(diagnostic);
            }
            usages[p][k] = b.usage();
        }
    }
    assert_eq!(diagnostics[0], diagnostics[1]);
    let common = usages[1][0].work - usages[0][0].work;
    // Request validation contributes the same catalog overhead to both paths.
    // Failed additionally selects reader and foundation for its diagnostic.
    assert_eq!(
        usages[1][1].work - usages[0][1].work - common,
        32 * ((10 + 12 + 9) + (10 + 16 + 9))
    );
    let mut expected = usages[0][1];
    expected.work = usages[1][1].work;
    assert_eq!(usages[1][1], expected);
    let mut f = Fixture::new("01")?;
    f.registry = padded_registry(32)?;
    let mut limits = budget().limits();
    limits.work = usages[0][1].work + common;
    let mut stopped = Budget::new(limits);
    let reply = f.read(
        BuiltinReader::Number,
        0,
        2,
        true,
        &mut stopped,
        &mut SourceAdmission::default(),
    )?;
    let ReadReply::Stopped {
        reason,
        sources,
        source_maps,
        report,
    } = reply
    else {
        return Err(ReaderError::ProviderContract);
    };
    assert_eq!(reason, StopReason::WorkLimit);
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    assert_eq!(report.usage, stopped.usage());
    assert!(sources.is_empty() && source_maps.is_empty());
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        f.read(
            BuiltinReader::Number,
            0,
            2,
            true,
            &mut cancelled,
            &mut SourceAdmission::default()
        )?,
        ReadReply::Stopped {
            reason: StopReason::Cancelled,
            ..
        }
    ));
    assert_eq!(cancelled.usage(), Usage::default());
    Ok(())
}
