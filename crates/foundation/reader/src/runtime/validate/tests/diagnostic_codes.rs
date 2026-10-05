use super::*;

fn registered() -> Result<(SchemaRegistry, SchemaRef, SchemaRef), String> {
    let mut registry = SchemaRegistry::default();
    let foundation =
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let foundation_ref = foundation
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(foundation_ref.clone(), foundation, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let reader = crate::schema::descriptor(&mut budget()).map_err(|e| format!("{e:?}"))?;
    let reader_ref = reader
        .reference(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .register(reader_ref.clone(), reader, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok((registry, reader_ref, foundation_ref))
}

#[test]
fn undeclared_reader_code_fails_domain_admission_not_generic_diagnostic_validation()
-> Result<(), String> {
    let (registry, reader, foundation) = registered()?;
    let diagnostic = Diagnostic {
        schema: reader.clone(),
        code: "InventedReaderDiagnostic".into(),
        severity: nepl3_core::diagnostic::Severity::Error,
        stage: "reader".into(),
        arguments: crate::schema::arguments(&reader, &foundation, &[], 0, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        primary: None,
        related: Vec::new(),
        fixes: Vec::new(),
    };
    let sources = SourceStore::default();
    // Generic structural/position admission intentionally has no reader-domain
    // dependency. The reader boundary must enforce its already-declared enum.
    diagnostic
        .validate(&sources, &[], &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        accepted_diagnostic(&diagnostic, &sources, &[], &registry, &mut budget()),
        Err(ReaderError::ProviderContract)
    ));
    Ok(())
}

fn diagnostic(
    reader: &SchemaRef,
    foundation: &SchemaRef,
    code: &str,
) -> Result<Diagnostic, String> {
    Ok(Diagnostic {
        schema: reader.clone(),
        code: code.into(),
        severity: nepl3_core::diagnostic::Severity::Error,
        stage: "reader".into(),
        arguments: crate::schema::arguments(reader, foundation, &[], 0, &mut budget())
            .map_err(|e| format!("{e:?}"))?,
        primary: None,
        related: Vec::new(),
        fixes: Vec::new(),
    })
}
#[test]
fn all_catalog_members_are_admitted_without_allocations_or_diagnostic_counts() -> Result<(), String>
{
    let (registry, reader, foundation) = registered()?;
    let descriptor = registry.descriptor(&reader).ok_or("descriptor")?;
    let ty = descriptor
        .types
        .iter()
        .find(|ty| ty.name == "ReaderDiagnosticCode")
        .ok_or("catalog")?;
    let nepl3_core::schema::TypeShape::Variant { variants } = &ty.shape else {
        return Err("enum".into());
    };
    assert!(!variants.is_empty());
    for variant in variants {
        let diagnostic = diagnostic(&reader, &foundation, &variant.name)?;
        let mut b = budget();
        super::super::diagnostics::code(&diagnostic, &registry, &mut b)
            .map_err(|e| format!("{e:?}"))?;
        let used = b.usage();
        assert_eq!(
            used,
            Usage {
                work: used.work,
                ..Usage::default()
            }
        );
        accepted_diagnostic(
            &diagnostic,
            &SourceStore::default(),
            &[],
            &registry,
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
#[test]
fn reader_catalog_stops_preserve_reason_and_external_domains_are_unchanged() -> Result<(), String> {
    let (registry, reader, foundation) = registered()?;
    let mut diagnostic = diagnostic(&reader, &foundation, "UnterminatedLiteral")?;
    let mut full = budget();
    super::super::diagnostics::code(&diagnostic, &registry, &mut full)
        .map_err(|e| format!("{e:?}"))?;
    let mut stopped = Budget::new(Limits {
        work: full.usage().work - 1,
        ..budget().limits()
    });
    assert_eq!(
        super::super::diagnostics::code(&diagnostic, &registry, &mut stopped),
        Err(ReaderError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        super::super::diagnostics::code(&diagnostic, &registry, &mut cancelled),
        Err(ReaderError::Stopped(StopReason::Cancelled))
    );
    diagnostic.code = "ExternalCode".into();
    diagnostic.schema = foundation;
    accepted_diagnostic(
        &diagnostic,
        &SourceStore::default(),
        &[],
        &registry,
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    diagnostic.schema = reader;
    diagnostic.schema.revision = 2;
    super::super::diagnostics::code(&diagnostic, &registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

#[test]
fn report_only_admission_checks_codes_without_recounting_diagnostics() -> Result<(), String> {
    let (registry, reader, foundation) = registered()?;
    let diagnostic = diagnostic(&reader, &foundation, "InventedReaderDiagnostic")?;
    let sources = SourceStore::default();
    let mut b = budget();
    b.charge(Resource::Diagnostics, 1)
        .map_err(|e| format!("{e:?}"))?;
    let report_value = Report {
        diagnostics: vec![diagnostic],
        usage: b.usage(),
        ..Report::default()
    };
    report_value
        .validate(&sources, &[], &registry, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        accepted_report(&report_value, &sources, &[], &registry, &mut b),
        Err(ReaderError::ProviderContract)
    );
    assert_eq!(
        report(&report_value, Usage::default(), &registry, &sources, &mut b),
        Err(ReaderError::ProviderContract)
    );
    assert_eq!(b.usage().diagnostics, 1);
    assert_eq!(b.usage().events, 0);
    Ok(())
}

#[test]
fn missing_malformed_catalog_and_wrong_selected_identity_are_rejected() -> Result<(), String> {
    let (_, reader, foundation) = registered()?;
    let mut diagnostic = diagnostic(&reader, &foundation, "ExpectedInput")?;
    for malformed in [false, true] {
        let types = if malformed {
            vec![nepl3_core::schema::NamedType {
                name: "ReaderDiagnosticCode".into(),
                shape: nepl3_core::schema::TypeShape::Record { fields: vec![] },
                constraints: vec![],
            }]
        } else {
            vec![]
        };
        let descriptor = nepl3_core::schema::SchemaDescriptor {
            package: "nepl3.reader".into(),
            revision: 1,
            types,
            operations: vec![],
        };
        let selected = descriptor
            .reference(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mut registry = SchemaRegistry::default();
        registry
            .register(selected.clone(), descriptor, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        diagnostic.schema = selected;
        assert_eq!(
            super::super::diagnostics::code(&diagnostic, &registry, &mut budget()),
            Err(ReaderError::ProviderContract)
        );
    }
    let (registry, reader, _) = registered()?;
    diagnostic.schema = reader;
    diagnostic.schema.digest.0[0] ^= 1;
    assert_eq!(
        super::super::diagnostics::code(&diagnostic, &registry, &mut budget()),
        Err(ReaderError::ProviderContract)
    );
    Ok(())
}
