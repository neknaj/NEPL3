//! Expected byte positions come from these fixed UTF-8/CRLF source fixtures,
//! independently of the compiler's AST arena ordering.
use super::*;
use nepl3_grammar_core::compile::CompileError;
#[test]
fn compiler_locates_missing_rule_range_and_binding_payload_from_real_source() -> Result<(), String>
{
    let root = "conformance/fixtures/grammar/diagnostic";
    compile(&format!("{root}/valid.neplg"))?.map_err(|e| format!("{e:?}"))?;
    for (case, primary, related) in [
        ("missing-rule", (88, 94), None),
        ("invalid-range", (101, 104), Some((97, 100))),
        ("binding-payload", (227, 231), Some((84, 108))),
    ] {
        let result = compile(&format!("{root}/{case}.neplg"))?;
        let Err(error) = result else {
            return Err(format!("{case} accepted"));
        };
        let located = error
            .location()
            .ok_or_else(|| format!("{case} missing location: {error:?}"))?;
        assert_eq!(
            (located.primary().start(), located.primary().end()),
            primary
        );
        assert_eq!(
            located
                .related()
                .first()
                .map(|span| (span.start(), span.end())),
            related
        );
        assert_eq!(
            located.primary().snapshot_ref().source.0,
            format!("{root}/{case}.neplg")
        );
        match case {
            "missing-rule" => assert_eq!(error.cause(), &CompileError::MissingRule),
            "invalid-range" => assert_eq!(error.cause(), &CompileError::InvalidRange),
            _ => {
                assert_eq!(
                    error.cause(),
                    &CompileError::Package(nepl3_engine::package::PackageError::InvalidBinding)
                );
                assert_eq!(located.expected(), Some(&TypeDescriptor::Text));
                assert_eq!(
                    located.actual(),
                    Some(&TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)))
                );
            }
        }
    }
    Ok(())
}

#[test]
fn compiler_locations_use_utf8_raw_escape_ranges_and_typed_choice_operands() -> Result<(), String> {
    for (case, primary, related) in [
        ("missing", (104, 110), None),
        ("range", (122, 125), Some((113, 121))),
        ("choice", (129, 147), Some((112, 123))),
    ] {
        let path = format!("conformance/fixtures/grammar/diagnostic/unicode-{case}.neplg");
        let Err(error) = compile(&path)? else {
            return Err(format!("{case} accepted"));
        };
        let location = error.location().ok_or_else(|| format!("{error:?}"))?;
        assert_eq!(
            (location.primary().start(), location.primary().end()),
            primary
        );
        assert_eq!(
            location
                .related()
                .first()
                .map(|span| (span.start(), span.end())),
            related
        );
        if case == "choice" {
            assert_eq!(
                error.cause(),
                &CompileError::Reader(nepl3_reader::plan::PlanError::OutputType)
            );
            assert_eq!(location.expected(), Some(&TypeDescriptor::Unit));
            assert_eq!(location.actual(), Some(&TypeDescriptor::Text));
        }
    }
    Ok(())
}

#[test]
fn plan_progress_and_recursion_failures_retain_the_compiled_expression_source() -> Result<(), String>
{
    for (case, expected, range) in [
        (
            "nonprogress",
            nepl3_reader::plan::PlanError::NonProgress,
            (84, 99),
        ),
        (
            "left-recursion",
            nepl3_reader::plan::PlanError::LeftRecursion,
            (113, 121),
        ),
    ] {
        let Err(error) = compile(&format!(
            "conformance/fixtures/grammar/diagnostic/{case}.neplg"
        ))?
        else {
            return Err(format!("{case} accepted"));
        };
        assert_eq!(error.cause(), &CompileError::Reader(expected));
        let span = error
            .location()
            .ok_or_else(|| format!("{error:?}"))?
            .primary();
        // The recursive edge is the original `ref word` constructor.
        assert_eq!((span.start(), span.end()), range);
    }
    Ok(())
}

fn diagnostic_registry() -> Result<SchemaRegistry, String> {
    let mut registry = SchemaRegistry::default();
    let mut b = budget();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_reader::schema::descriptor(&mut b),
        nepl3_engine::schema::descriptor(&mut b),
        nepl3_grammar_core::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(|e| format!("{e:?}"))?;
        let schema = descriptor.reference(&mut b).map_err(|e| format!("{e:?}"))?;
        registry
            .register(schema, descriptor, &mut b)
            .map_err(|e| format!("{e:?}"))?;
    }
    registry.finalize(&mut b).map_err(|e| format!("{e:?}"))?;
    Ok(registry)
}
#[test]
fn compiler_diagnostic_is_typed_roundtrips_and_stops_without_losing_borrowed_location()
-> Result<(), String> {
    use nepl3_core::{
        diagnostic::Report,
        value::{NdfValue, TypedValue},
        value_codec::FoundationValueCodec,
    };
    use nepl3_grammar_core::compile::diagnostic::DiagnosticError;
    let path = "conformance/fixtures/grammar/diagnostic/binding-payload.neplg";
    let Err(error) = compile(path)? else {
        return Err("expected binding error".into());
    };
    let doc = load(path)?;
    let registry = diagnostic_registry()?;
    let mut store = SourceStore::default();
    for source in &doc.sources {
        store.insert(source.clone()).map_err(|e| format!("{e:?}"))?;
    }
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let checked = doc
        .validate(&mut b, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let source_bytes = b.usage().source_bytes;
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let diagnostic = error
        .diagnostic(&checked, &registry, &mut codec, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(diagnostic.code, "InvalidBinding");
    assert_eq!(
        diagnostic.primary.as_ref().map(|s| (s.start(), s.end())),
        Some((227, 231))
    );
    assert_eq!(
        diagnostic.related[0]
            .span
            .as_ref()
            .map(|s| (s.start(), s.end())),
        Some((84, 108))
    );
    let TypedValue::Record(arguments) = &diagnostic.arguments else {
        return Err("record arguments".into());
    };
    assert_eq!(arguments.kind, "CompileDiagnosticArguments");
    let NdfValue::Some(expected) = &arguments.fields[0] else {
        return Err("expected descriptor".into());
    };
    let NdfValue::Variant(expected) = expected.as_ref() else {
        return Err("expected variant".into());
    };
    assert_eq!(expected.variant, "Text");
    let NdfValue::Some(actual) = &arguments.fields[1] else {
        return Err("actual descriptor".into());
    };
    let NdfValue::Variant(actual) = actual.as_ref() else {
        return Err("actual variant".into());
    };
    assert_eq!(actual.variant, "List");
    let NdfValue::Variant(inner) = &actual.fields[0] else {
        return Err("element descriptor".into());
    };
    assert_eq!(inner.variant, "NdfValue");
    let report = Report {
        diagnostics: vec![diagnostic],
        usage: b.usage(),
        ..Report::default()
    };
    let encoded = codec
        .encode_report(&report, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let wire = nepl3_wire::encode(&encoded, &mut b).map_err(|e| format!("{e:?}"))?;
    let encoded = nepl3_wire::decode(&wire, &mut b).map_err(|e| format!("{e:?}"))?;
    let decoded = codec
        .decode_report(&encoded, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(report, decoded);
    assert_eq!(b.usage().source_bytes, source_bytes);
    assert_eq!(b.usage().diagnostics, 1);
    for (limits, expected) in [
        (
            Limits {
                source_bytes: 0,
                ..budget().limits()
            },
            StopReason::SourceLimit,
        ),
        (
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            StopReason::AllocationLimit,
        ),
        (
            Limits {
                diagnostics: 0,
                ..budget().limits()
            },
            StopReason::DiagnosticLimit,
        ),
    ] {
        let mut limited = Budget::new(limits);
        let mut admission = SourceAdmission::default();
        let mut codec =
            nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
        assert!(
            matches!(error.diagnostic(&checked, &registry, &mut codec, &mut limited), Err(DiagnosticError::Stopped(reason)) if reason == expected)
        );
        assert_eq!(limited.usage().diagnostics, 0);
        assert_eq!(error.location().map(|v| v.primary().start()), Some(227));
    }
    Ok(())
}

#[test]
fn document_compile_and_diagnostic_share_one_source_admission() -> Result<(), String> {
    for case in ["valid", "binding-payload"] {
        let path = format!("conformance/fixtures/grammar/diagnostic/{case}.neplg");
        let doc = load(&path)?;
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let checked = doc
            .validate(&mut b, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let bytes = doc
            .sources
            .iter()
            .map(|source| source.text().len() as u64)
            .sum::<u64>();
        assert_eq!(b.usage().source_bytes, bytes);
        let context = PackageContext {
            package: "test.diagnostic",
            state_type: &TypeDescriptor::Unit,
            reader_imports: &[],
            extensions: &[],
            classes: &[],
            views: &[],
        };
        let result = compile::package::compile_with_admission(
            &checked,
            &context,
            diagnostic_registry()?,
            &mut b,
            &mut admission,
        );
        assert_eq!(b.usage().source_bytes, bytes);
        match (case, result) {
            ("valid", Ok(compiled)) => {
                compiled
                    .package
                    .check_with_admission(&compiled.registry, &mut b, &mut admission)
                    .map_err(|e| format!("{e:?}"))?;
            }
            ("binding-payload", Err(error)) => {
                let registry = diagnostic_registry()?;
                let mut store = SourceStore::default();
                for source in &doc.sources {
                    store.insert(source.clone()).map_err(|e| format!("{e:?}"))?;
                }
                let mut codec =
                    nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
                        .map_err(|e| format!("{e:?}"))?;
                error
                    .diagnostic(&checked, &registry, &mut codec, &mut b)
                    .map_err(|e| format!("{e:?}"))?;
            }
            (_, Err(error)) => return Err(format!("unexpected {case}: {error:?}")),
            _ => return Err("invalid source accepted".into()),
        }
        assert_eq!(b.usage().source_bytes, bytes);
        let mut stopped = Budget::new(Limits {
            source_bytes: 0,
            ..budget().limits()
        });
        assert!(matches!(
            compile::package::compile_with_admission(
                &checked,
                &context,
                diagnostic_registry()?,
                &mut stopped,
                &mut SourceAdmission::default()
            ),
            Err(CompileError::Stopped(StopReason::SourceLimit))
        ));
    }
    Ok(())
}
#[test]
fn compiler_diagnostic_rejects_an_unrelated_document_and_preserves_stopped_reason()
-> Result<(), String> {
    use nepl3_grammar_core::compile::diagnostic::DiagnosticError;
    let Err(error) = compile("conformance/fixtures/grammar/diagnostic/missing-rule.neplg")? else {
        return Err("missing rule accepted".into());
    };
    let doc = load("conformance/fixtures/grammar/diagnostic/valid.neplg")?;
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let checked = doc
        .validate(&mut b, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let registry = diagnostic_registry()?;
    let store = SourceStore::default();
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    assert!(matches!(
        error.diagnostic(&checked, &registry, &mut codec, &mut b),
        Err(DiagnosticError::Validation(
            nepl3_core::diagnostic::validation::ReportValidationError::Source(
                SourceError::MissingSnapshot
            )
        ))
    ));
    assert_eq!(b.usage().diagnostics, 0);
    let error = CompileError::Stopped(StopReason::WorkLimit);
    assert!(matches!(
        error.diagnostic(&checked, &registry, &mut codec, &mut b),
        Err(DiagnosticError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    assert_eq!(b.usage().diagnostics, 0);
    Ok(())
}

#[test]
fn compiler_report_boundary_without_host_process() -> Result<(), String> {
    use nepl3_core::{diagnostic::Report, value_codec::FoundationValueCodec};
    // The seed JSON is generated from the adjacent fixed raw source. This test
    // needs no host process and runs the identical compiler/codec entry on WASI.
    let bytes =
        include_bytes!("../../../conformance/fixtures/grammar/diagnostic/binding-payload.json");
    let mut b = budget();
    let mut admission = SourceAdmission::default();
    let doc = nepl3_tools::bootstrap::load(bytes, &mut b, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        doc.sources[0].text().as_bytes(),
        include_bytes!("../../../conformance/fixtures/grammar/diagnostic/binding-payload.neplg")
    );
    let checked = doc
        .validate(&mut b, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let context = PackageContext {
        package: "test.diagnostic",
        state_type: &TypeDescriptor::Unit,
        reader_imports: &[],
        extensions: &[],
        classes: &[],
        views: &[],
    };
    let Err(error) = compile::package::compile_with_admission(
        &checked,
        &context,
        diagnostic_registry()?,
        &mut b,
        &mut admission,
    ) else {
        return Err("invalid binding accepted".into());
    };
    let mut store = SourceStore::default();
    for source in &doc.sources {
        store.insert(source.clone()).map_err(|e| format!("{e:?}"))?;
    }
    let registry = diagnostic_registry()?;
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(&registry, &store, &mut admission)
        .map_err(|e| format!("{e:?}"))?;
    let diagnostic = error
        .diagnostic(&checked, &registry, &mut codec, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        diagnostic.primary.as_ref().map(|v| (v.start(), v.end())),
        Some((227, 231))
    );
    let report = Report {
        diagnostics: vec![diagnostic],
        usage: b.usage(),
        ..Report::default()
    };
    let value = codec
        .encode_report(&report, &mut b)
        .map_err(|e| format!("{e:?}"))?;
    let wire = nepl3_wire::encode(&value, &mut b).map_err(|e| format!("{e:?}"))?;
    let value = nepl3_wire::decode(&wire, &mut b).map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        codec
            .decode_report(&value, &mut b)
            .map_err(|e| format!("{e:?}"))?,
        report
    );
    assert_eq!(b.usage().source_bytes, doc.sources[0].text().len() as u64);
    assert_eq!(b.usage().diagnostics, 1);
    Ok(())
}

#[test]
fn checked_in_seed_fixture_matches_the_real_bootstrap_adapter() -> Result<(), String> {
    let path = "conformance/fixtures/grammar/diagnostic/binding-payload.neplg";
    let generated = load(path)?;
    let saved = nepl3_tools::bootstrap::load(
        include_bytes!("../../../conformance/fixtures/grammar/diagnostic/binding-payload.json"),
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(generated, saved);
    Ok(())
}

#[test]
fn package_subjects_locate_original_category_and_mode_operands() -> Result<(), String> {
    use nepl3_engine::package::PackageError;
    for (case, range, code) in [
        ("root", (32, 38), PackageError::MissingCategory),
        ("mode", (59, 65), PackageError::MissingMode),
        ("local", (228, 234), PackageError::MissingCategory),
        ("withmode", (231, 237), PackageError::MissingMode),
        ("form-category", (193, 199), PackageError::MissingCategory),
        ("leaf-category", (156, 162), PackageError::MissingCategory),
    ] {
        let path = format!("conformance/fixtures/grammar/diagnostic/package-{case}.neplg");
        let Err(error) = compile(&path)? else {
            return Err(format!("{case} accepted"));
        };
        assert_eq!(error.cause(), &CompileError::Package(code));
        let location = error
            .location()
            .ok_or_else(|| format!("{case}: {error:?}"))?;
        assert_eq!(
            (location.primary().start(), location.primary().end()),
            range
        );
        assert_eq!(location.primary().snapshot_ref().source.0, path);
        // The independent source fixture puts the missing operand at these bytes.
        let document = load(&path)?;
        assert_eq!(
            document.sources[0]
                .slice(location.primary())
                .map_err(|e| format!("{e:?}"))?,
            "Absent"
        );
    }
    Ok(())
}

#[test]
fn detailed_package_checker_is_the_legacy_checker_with_exact_subjects() -> Result<(), String> {
    use nepl3_engine::package::{PackageError, PackageSubject};
    let compiled = compile("conformance/fixtures/grammar/diagnostic/valid.neplg")?
        .map_err(|e| format!("{e:?}"))?;
    for case in 0..4 {
        let mut package = compiled.package.clone();
        let (error, subject) = match case {
            0 => {
                package.root = "Absent".into();
                (PackageError::MissingCategory, PackageSubject::Root)
            }
            1 => {
                package.categories[0].mode = "Absent".into();
                (PackageError::MissingMode, PackageSubject::Category(0))
            }
            2 => {
                package.leaves[0].category = "Absent".into();
                (PackageError::MissingCategory, PackageSubject::Leaf(0))
            }
            _ => {
                package.modes[0].take[0].reader =
                    nepl3_reader::tokenizer::TokenReader::Rule("Absent".into());
                (
                    PackageError::Reader(nepl3_reader::plan::PlanError::Reference),
                    PackageSubject::ModeTake { mode: 0, rule: 0 },
                )
            }
        };
        let mut legacy_budget = budget();
        let mut detailed_budget = budget();
        let legacy = package
            .check(&compiled.registry, &mut legacy_budget)
            .err()
            .ok_or("legacy accepted")?;
        let detailed = package
            .check_detailed(&compiled.registry, &mut detailed_budget)
            .err()
            .ok_or("detailed accepted")?;
        assert_eq!(legacy, error);
        assert_eq!(detailed.error, error);
        assert_eq!(detailed.subject, Some(subject));
        assert_eq!(legacy_budget.usage(), detailed_budget.usage());
    }
    let mut limited = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    let error = compiled
        .package
        .check_detailed(&compiled.registry, &mut limited)
        .err()
        .ok_or("limit ignored")?;
    assert_eq!(error.error, PackageError::Stopped(StopReason::WorkLimit));
    assert_eq!(error.subject, None);
    Ok(())
}
