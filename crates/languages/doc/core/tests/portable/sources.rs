use super::*;

#[test]
fn source_index_is_reused_within_document_encoding() -> Result<(), String> {
    let r = registry()?;
    for count in [1, 65] {
        let mut document = literal(&r)?;
        for index in 1..count {
            document.sources.push(
                SourceSnapshot::new(
                    SourceId(format!("extra/{index:04}")),
                    1,
                    "memory:extra".into(),
                    vec![b'x'; 1024],
                    &mut b(),
                )
                .map_err(err)?,
            );
        }
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
        let mut measured = b();
        let value = portable::to_value(&document, &r, &mut codec, &mut measured).map_err(err)?;
        // Measured with this exact fixture on the unchanged two-index
        // encoder at 4c8d0434. The old path is a real negative control.
        if cfg!(target_pointer_width = "64") {
            let (work, allocation) = if count == 1 {
                (31_092, 60_737)
            } else {
                (100_308, 210_497)
            };
            let mut rebuild = b();
            let mut store = SourceStore::default();
            for source in &document.sources {
                store
                    .insert_with_budget(
                        source.clone_with_budget(&mut rebuild).map_err(err)?,
                        &mut rebuild,
                    )
                    .map_err(err)?;
            }
            assert_eq!(
                measured.usage().work,
                work - rebuild.usage().work,
                "source index must not be rebuilt"
            );
            assert_eq!(
                measured.usage().allocation_units,
                allocation - rebuild.usage().allocation_units
            );
        }
        let mut incoming = SourceAdmission::default();
        let mut decoder = FoundationCodec::new(&r, &empty, &mut incoming).map_err(err)?;
        assert_eq!(
            portable::from_value(&value, &r, &mut decoder, &mut b()).map_err(err)?,
            document
        );
    }
    Ok(())
}

fn encode(
    document: &DocumentSyntax,
    r: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<NdfValue, PortableError<nepl3_wire::WireError>> {
    let empty = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec =
        FoundationCodec::new(r, &empty, &mut admission).map_err(PortableError::Foundation)?;
    portable::to_value(document, r, &mut codec, budget)
}

#[test]
fn reused_source_index_preserves_budget_edges_and_prior_usage() -> Result<(), String> {
    use nepl3_core::budget::Resource;
    let r = registry()?;
    let document = literal(&r)?;
    let mut measured = b();
    let expected = encode(&document, &r, &mut measured).map_err(err)?;
    let mut prior = b();
    prior.charge(Resource::Work, 17).map_err(err)?;
    prior.charge(Resource::AllocationUnits, 23).map_err(err)?;
    assert_eq!(encode(&document, &r, &mut prior).map_err(err)?, expected);
    assert_eq!(prior.usage().work, measured.usage().work + 17);
    assert_eq!(
        prior.usage().allocation_units,
        measured.usage().allocation_units + 23
    );
    for allocation in [false, true] {
        for short in [false, true] {
            let mut limits = b().limits();
            if allocation {
                limits.allocation_units = measured.usage().allocation_units - u64::from(short);
            } else {
                limits.work = measured.usage().work - u64::from(short);
            }
            let mut limited = Budget::new(limits);
            let result = encode(&document, &r, &mut limited);
            if short {
                let reason = if allocation {
                    StopReason::AllocationLimit
                } else {
                    StopReason::WorkLimit
                };
                assert_eq!(result, Err(PortableError::Stopped(reason)));
                assert_eq!(
                    limited.poll(),
                    Err(if allocation {
                        StopReason::AllocationLimit
                    } else {
                        StopReason::WorkLimit
                    })
                );
                assert!(encode(&document, &r, &mut limited).is_err());
            } else {
                assert_eq!(result.map_err(err)?, expected);
            }
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        encode(&document, &r, &mut cancelled),
        Err(PortableError::Stopped(StopReason::Cancelled))
    );
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}

#[test]
fn source_reuse_does_not_bypass_fresh_document_validation() -> Result<(), String> {
    let r = registry()?;
    let mut document = literal(&r)?;
    let _ = encode(&document, &r, &mut b()).map_err(err)?;
    document.sources.push(document.sources[0].clone());
    assert!(encode(&document, &r, &mut b()).is_err());
    document.sources.clear();
    assert!(encode(&document, &r, &mut b()).is_err());
    let document = literal(&r)?;
    assert!(encode(&document, &SchemaRegistry::default(), &mut b()).is_err());
    Ok(())
}

#[test]
fn reply_codecs_reuse_only_the_validated_request_source_closure() -> Result<(), String> {
    use nepl3_core::diagnostic::{Diagnostic, Report, Severity};
    use nepl3_core::value::Record;
    use nepl3_doc_core::{
        print::{PrintOutcome, PrintReply},
        text::{PlainTextOutcome, PlainTextReply},
    };
    let r = registry()?;
    let document = literal(&r)?;
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?;
    let schema = descriptor.reference(&mut b()).map_err(err)?;
    let report = Report {
        diagnostics: vec![Diagnostic {
            schema: schema.clone(),
            code: "source-reuse".into(),
            severity: Severity::Warning,
            stage: "test".into(),
            arguments: nepl3_core::value::TypedValue::Record(Record {
                schema,
                kind: "TraceOverflow".into(),
                fields: vec![NdfValue::U64(1)],
            }),
            primary: Some(document.sources[0].span(0, 1).map_err(err)?),
            related: vec![],
            fixes: vec![],
        }],
        usage: nepl3_core::budget::Usage {
            diagnostics: 1,
            ..Default::default()
        },
        ..Report::default()
    };
    let foreign = SourceSnapshot::new(
        SourceId("outside".into()),
        1,
        "memory:outside".into(),
        b"x".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let mut wider = document.clone();
    wider.sources.push(foreign.clone());
    let empty = SourceStore::default();
    for print in [false, true] {
        for outside in [false, true] {
            let mut report = report.clone();
            if outside {
                report.diagnostics[0].primary = Some(foreign.span(0, 1).map_err(err)?);
            }
            let text_reply = PlainTextReply {
                outcome: PlainTextOutcome::Stopped {
                    reason: StopReason::WorkLimit,
                },
                report: report.clone(),
            };
            let print_reply = PrintReply {
                outcome: PrintOutcome::Stopped {
                    reason: StopReason::WorkLimit,
                },
                report,
            };
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
            let encode =
                |doc: &DocumentSyntax, codec: &mut FoundationCodec<'_>, budget: &mut Budget| {
                    if print {
                        portable::print::reply_to_value(&print_reply, doc, &r, codec, budget)
                    } else {
                        portable::text::reply_to_value(&text_reply, doc, &r, codec, budget)
                    }
                };
            let mut measured = b();
            let value = encode(
                if outside { &wider } else { &document },
                &mut codec,
                &mut measured,
            )
            .map_err(err)?;

            let mut receiver = SourceAdmission::default();
            let mut decoder = FoundationCodec::new(&r, &empty, &mut receiver).map_err(err)?;
            let mut received = b();
            if print {
                let result = portable::print::reply_from_value(
                    &value,
                    &document,
                    &r,
                    &mut decoder,
                    &mut received,
                );
                if outside {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.map_err(err)?, print_reply);
                }
            } else {
                let result = portable::text::reply_from_value(
                    &value,
                    &document,
                    &r,
                    &mut decoder,
                    &mut received,
                );
                if outside {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.map_err(err)?, text_reply);
                }
            }
            if !outside && cfg!(target_pointer_width = "64") {
                // Unchanged reply adapters rebuilt one source index after full
                // validation: Work 2 and AllocationUnits 24 for this fixture.
                let (ew, ea, dw, da) = if print {
                    (5855, 6998, 5960, 2386)
                } else {
                    (5928, 7010, 6033, 2390)
                };
                assert_eq!(measured.usage().work, ew - 2, "reply source index rebuilt");
                assert_eq!(measured.usage().allocation_units, ea - 24);
                assert_eq!(received.usage().work, dw - 2, "reply source index rebuilt");
                assert_eq!(received.usage().allocation_units, da - 24);
            }
            if outside {
                // Even a codec with prior admission of the wider request must
                // not resolve an unrelated report through ambient admission.
                assert!(encode(&document, &mut codec, &mut measured).is_err());
            } else {
                for short in [false, true] {
                    let mut admission = SourceAdmission::default();
                    let mut codec =
                        FoundationCodec::new(&r, &empty, &mut admission).map_err(err)?;
                    let mut limited = Budget::new(Limits {
                        work: measured.usage().work - u64::from(short),
                        ..b().limits()
                    });
                    let mut receive_admission = SourceAdmission::default();
                    let mut receive_codec =
                        FoundationCodec::new(&r, &empty, &mut receive_admission).map_err(err)?;
                    let mut receive_limit = Budget::new(Limits {
                        work: received.usage().work - u64::from(short),
                        ..b().limits()
                    });
                    let decoded = if print {
                        portable::print::reply_from_value(
                            &value,
                            &document,
                            &r,
                            &mut receive_codec,
                            &mut receive_limit,
                        )
                        .map(|_| ())
                    } else {
                        portable::text::reply_from_value(
                            &value,
                            &document,
                            &r,
                            &mut receive_codec,
                            &mut receive_limit,
                        )
                        .map(|_| ())
                    };
                    if short {
                        assert_eq!(decoded, Err(PortableError::Stopped(StopReason::WorkLimit)));
                    } else {
                        decoded.map_err(err)?;
                    }
                    let result = encode(&document, &mut codec, &mut limited);
                    if short {
                        assert_eq!(result, Err(PortableError::Stopped(StopReason::WorkLimit)));
                    } else {
                        assert_eq!(result.map_err(err)?, value);
                    }
                }
            }
        }
    }
    Ok(())
}
