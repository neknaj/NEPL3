use super::*;
use nepl3_core::{
    budget::{Resource, StopReason},
    diagnostic::OperationResult,
    operation::OperationReply,
    value::{OperationRef, TypedValue},
    value_codec::FoundationValueCodec,
};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    portable::read::sender,
};
fn err(e: impl std::fmt::Debug) -> String {
    format!("{e:?}")
}
fn fixture_text(
    text: &str,
    final_input: bool,
) -> Result<
    (
        SchemaRegistry,
        SourceStore,
        OwnedReadRequest,
        TypedValue,
        OperationRef,
    ),
    String,
> {
    let (schema, registry, _, mut request) = fixture()?;
    let source = SourceSnapshot::new(
        SourceId("input".into()),
        1,
        "memory:input".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    request.snapshot = source.reference();
    request.limit = text.len() as u64;
    request.final_input = final_input;
    for existing in &mut request.sources {
        if existing.identity().source.0 == "input" {
            *existing = source.clone();
        }
    }
    let mut sources = SourceStore::default();
    for source in &request.sources {
        sources.insert(source.clone()).map_err(err)?;
    }
    sources
        .insert(
            SourceSnapshot::new(
                SourceId("ambient".into()),
                1,
                "memory:ambient".into(),
                vec![b'x'; 4096],
                &mut budget(),
            )
            .map_err(err)?,
        )
        .map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut a).map_err(err)?;
    let NdfValue::Record(ref record) = request_to_value(
        &request,
        &schema,
        &mut codec,
        &sources,
        &registry,
        &mut budget(),
    )
    .map_err(err)?
    else {
        return Err("request record".into());
    };
    let op = provider::operation(BuiltinReader::Name, &registry, &mut budget()).map_err(err)?;
    Ok((
        registry,
        sources,
        request,
        TypedValue::Record(record.clone()),
        op,
    ))
}
#[test]
fn issued_name_preserves_outcome_reports_and_narrows_encoding_sources() -> Result<(), String> {
    for (text, final_input, kind) in [("変数 ", true, 0), ("9", true, 1), ("変数", false, 2)] {
        let (r, s, request, input, op) = fixture_text(text, final_input)?;
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&r, &s, &mut admission).map_err(err)?;
        let mut execution = budget();
        execution.charge(Resource::Work, 23).map_err(err)?;
        let issued =
            sender::execute_name(&op, &input, &r, &s, &mut codec, &mut execution).map_err(err)?;
        match (issued.reply(), kind) {
            (
                ReadReply::Matched {
                    value: NdfValue::Text(value),
                    end,
                    ..
                },
                0,
            ) => {
                assert_eq!(value, "変数");
                assert_eq!(*end, 6);
            }
            (ReadReply::NoMatch { furthest, .. }, 1) => assert_eq!(*furthest, 0),
            (ReadReply::NeedMore { .. }, 2) => {}
            _ => return Err("wrong builtin outcome".into()),
        };
        let before = execution.usage();
        assert_eq!(issued.execution_usage(), before);
        let mut encoding = budget();
        let mut fresh = SourceAdmission::default();
        let mut output_codec = FoundationCodec::new(&r, &s, &mut fresh).map_err(err)?;
        let encoded = issued
            .to_operation_reply(&mut output_codec, &mut encoding)
            .map_err(err)?;
        assert_eq!(
            encoding.usage().source_bytes,
            request
                .sources
                .iter()
                .map(|s| s.text().len() as u64)
                .sum::<u64>()
        );
        let OperationReply::Result(OperationResult::Complete { report, .. }) = &encoded else {
            return Err("complete envelope".into());
        };
        assert_eq!(report.usage, before);
        let OperationReply::Result(OperationResult::Complete {
            value: TypedValue::Variant(v),
            report,
        }) = &encoded
        else {
            return Err("typed reply".into());
        };
        let report_index = match v.variant.as_str() {
            "Matched" => 7,
            "NoMatch" => 2,
            "NeedMore" => 1,
            _ => return Err("outcome variant".into()),
        };
        let inner = output_codec
            .decode_report(&v.fields[report_index], &mut budget())
            .map_err(err)?;
        assert_eq!(&inner, report);
        assert_eq!(execution.usage(), before);
        let mut short = Budget::new(Limits {
            work: 0,
            ..budget().limits()
        });
        assert!(
            issued
                .to_operation_reply(&mut output_codec, &mut short)
                .is_err()
        );
        assert_eq!(execution.usage(), before);
        assert_eq!(
            issued
                .to_operation_reply(&mut output_codec, &mut budget())
                .map_err(err)?,
            encoded
        );
    }
    Ok(())
}
#[test]
fn name_sender_rejects_operation_state_bounds_and_source_authority() -> Result<(), String> {
    let (r, s, request, input, op) = fixture_text("変数", true)?;
    let run = |operation: &OperationRef,
               input: &TypedValue,
               sources: &SourceStore|
     -> Result<(), PortableError<nepl3_wire::WireError>> {
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(PortableError::Boundary)?;
        sender::execute_name(operation, input, &r, sources, &mut c, &mut budget()).map(|_| ())
    };
    for name in ["builtinNumber", "builtinTrivia", "missing"] {
        let mut bad = op.clone();
        bad.name = name.into();
        assert!(matches!(
            run(&bad, &input, &s),
            Err(PortableError::Reader(
                nepl3_reader::runtime::ReaderError::ProviderContract
            ))
        ));
    }
    let mut bad = op.clone();
    bad.schema.digest.0[0] ^= 1;
    assert!(matches!(
        run(&bad, &input, &s),
        Err(PortableError::Reader(
            nepl3_reader::runtime::ReaderError::ProviderContract
        ))
    ));
    for mutation in 0..3 {
        let mut bad = input.clone();
        let TypedValue::Record(rec) = &mut bad else {
            return Err("record".into());
        };
        match mutation {
            0 => rec.fields[6] = NdfValue::Text("state".into()),
            1 => rec.fields[2] = NdfValue::U64(1),
            _ => rec.fields[3] = NdfValue::U64(2),
        };
        assert!(
            matches!(
                run(&op, &bad, &s),
                Err(PortableError::Schema(
                    nepl3_core::schema::SchemaError::WrongType
                ))
            ) && mutation == 0
                || matches!(
                    run(&op, &bad, &s),
                    Err(PortableError::Source(
                        nepl3_core::source::SourceError::ScalarBoundary
                    ))
                ) && mutation != 0
        );
    }
    assert!(matches!(
        run(&op, &input, &SourceStore::default()),
        Err(PortableError::UndeclaredSource)
    ));
    // Context origin remains visible in the ambient codec, but is absent from the request table.
    let mut bad = input.clone();
    let TypedValue::Record(rec) = &mut bad else {
        return Err("record".into());
    };
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let declared: Vec<_> = request
        .sources
        .iter()
        .filter(|s| s.identity().source.0 == "input")
        .cloned()
        .collect();
    rec.fields[1] = c.encode_sources(&declared, &mut budget()).map_err(err)?;
    assert!(run(&op, &bad, &s).is_err());
    // An unused declaration is not accepted merely because it is carried in the input.
    let mut bad = input.clone();
    let TypedValue::Record(rec) = &mut bad else {
        return Err("record".into());
    };
    let mut declared = request.sources.clone();
    declared.push(
        SourceSnapshot::new(
            SourceId("unauthorized".into()),
            1,
            "memory:unauthorized".into(),
            b"hidden".to_vec(),
            &mut budget(),
        )
        .map_err(err)?,
    );
    rec.fields[1] = c.encode_sources(&declared, &mut budget()).map_err(err)?;
    assert!(run(&op, &bad, &s).is_err());
    for environment in [false, true] {
        let mut bad = input.clone();
        let TypedValue::Record(record) = &mut bad else {
            return Err("request record".into());
        };
        let NdfValue::Record(context) = &mut record.fields[5] else {
            return Err("context record".into());
        };
        let NdfValue::Record(identity) = &mut context.fields[if environment { 3 } else { 0 }]
        else {
            return Err("context identity".into());
        };
        let NdfValue::Bytes(digest) = &mut identity.fields[if environment { 1 } else { 2 }] else {
            return Err("digest".into());
        };
        digest[0] ^= 1;
        assert!(matches!(
            run(&op, &bad, &s),
            Err(PortableError::Context(_)) | Err(PortableError::Boundary(_))
        ));
    }
    let mut bad = input.clone();
    let TypedValue::Record(record) = &mut bad else {
        return Err("request record".into());
    };
    let NdfValue::Record(reference) = &mut record.fields[0] else {
        return Err("source reference".into());
    };
    let NdfValue::Bytes(digest) = &mut reference.fields[2] else {
        return Err("source digest".into());
    };
    digest[0] ^= 1;
    assert!(matches!(
        run(&op, &bad, &s),
        Err(PortableError::UndeclaredSource)
    ));
    // Same identity bytes with a different locator, and same revision with different content.
    for (uri, text) in [("memory:other", "変数"), ("memory:input", "異体")] {
        let mut authority = SourceStore::default();
        for source in &request.sources {
            let selected = if source.identity().source.0 == "input" {
                SourceSnapshot::new(
                    SourceId("input".into()),
                    1,
                    uri.into(),
                    text.as_bytes().to_vec(),
                    &mut budget(),
                )
                .map_err(err)?
            } else {
                source.clone()
            };
            authority.insert(selected).map_err(err)?;
        }
        assert!(matches!(
            run(&op, &input, &authority),
            Err(PortableError::UndeclaredSource)
        ));
    }
    Ok(())
}
#[test]
fn name_sender_preserves_execution_stops_and_separate_encoding_retry() -> Result<(), String> {
    let (r, s, _, input, op) = fixture_text("変数", true)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let mut full = budget();
    let _ = sender::execute_name(&op, &input, &r, &s, &mut c, &mut full).map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let mut stopped = budget();
    stopped.cancel();
    let before = stopped.usage();
    assert!(matches!(
        sender::execute_name(&op, &input, &r, &s, &mut c, &mut stopped),
        Err(PortableError::Stopped(StopReason::Cancelled))
    ));
    assert_eq!(stopped.usage(), before);
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let mut limited = Budget::new(Limits {
        work: full.usage().work - 1,
        ..budget().limits()
    });
    let issued = sender::execute_name(&op, &input, &r, &s, &mut c, &mut limited).map_err(err)?;
    assert!(matches!(
        issued.reply(),
        ReadReply::Stopped {
            reason: StopReason::WorkLimit,
            ..
        }
    ));
    let usage = limited.usage();
    assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
    let mut encoding_stop = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(
        issued
            .to_operation_reply(&mut c, &mut encoding_stop)
            .is_err()
    );
    assert_eq!(limited.usage(), usage);
    assert!(matches!(
        issued.reply(),
        ReadReply::Stopped {
            reason: StopReason::WorkLimit,
            ..
        }
    ));
    let encoded = issued
        .to_operation_reply(&mut c, &mut budget())
        .map_err(err)?;
    assert!(
        matches!(encoded,OperationReply::Result(OperationResult::Stopped{reason:StopReason::WorkLimit,report,..}) if report.usage==usage)
    );
    assert_eq!(limited.usage(), usage);
    assert_eq!(issued.execution_usage(), usage);
    Ok(())
}

#[test]
fn sender_selection_uses_the_distinct_execution_and_encoding_budgets() -> Result<(), String> {
    let (r, s, _, input, op) = fixture_text("alpha", true)?;
    let mut setup = budget();
    let signature = provider::signature(BuiltinReader::Name, &r, &mut setup).map_err(err)?;
    let header = (op.name.len()
        + op.schema.package.len()
        + signature.operation.name.len()
        + signature.operation.schema.package.len()) as u64
        + 33;
    let mut limits = budget().limits();
    limits.work = setup.usage().work + header + 1;
    let mut short_execution = Budget::new(limits);
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    assert!(matches!(
        sender::execute_name(&op, &input, &r, &s, &mut c, &mut short_execution),
        Err(PortableError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(short_execution.poll(), Err(StopReason::WorkLimit));
    // Selection must stop before cloning the input or admitting its sources.
    assert_eq!(
        short_execution.usage().allocation_units,
        setup.usage().allocation_units
    );
    assert_eq!(short_execution.usage().source_bytes, 0);
    let mut execution = budget();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &s, &mut a).map_err(err)?;
    let issued = sender::execute_name(&op, &input, &r, &s, &mut c, &mut execution).map_err(err)?;
    let usage = execution.usage();
    let reply = issued.reply().clone();
    let mut limits = budget().limits();
    limits.work = 1;
    let mut encoding = Budget::new(limits);
    assert!(matches!(
        issued.to_operation_reply(&mut c, &mut encoding),
        Err(PortableError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(
        encoding.usage(),
        nepl3_core::budget::Usage {
            work: 1,
            ..Default::default()
        }
    );
    assert_eq!(encoding.poll(), Err(StopReason::WorkLimit));
    assert_eq!(issued.execution_usage(), usage);
    assert_eq!(execution.usage(), usage);
    assert_eq!(issued.reply(), &reply);
    let encoded = issued
        .to_operation_reply(&mut c, &mut budget())
        .map_err(err)?;
    let OperationReply::Result(OperationResult::Complete { report, .. }) = encoded else {
        return Err("complete".into());
    };
    assert_eq!(report.usage, usage);
    assert_eq!(issued.reply(), &reply);
    Ok(())
}
