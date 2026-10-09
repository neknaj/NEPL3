use super::*;

#[test]
fn provider_expectations_are_checked_for_both_nonmatching_outcomes() -> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for streaming in [false, true] {
        for mode in 0..3 {
            let invalid = mode != 0;
            let p = provider_plan(&schema);
            let checked = p.check(&registry, &mut budget())?;
            let mut b = budget();
            let mut session =
                ReaderSession::new("expectation".into(), &checked, &registry, &mut b)?;
            let source = source("a")?;
            let mut store = SourceStore::default();
            store.insert(source.clone())?;
            let mut admission = SourceAdmission::default();
            let raw = context(&schema, &registry)?;
            let proof = check_context(&raw, &store, &registry, &mut b, &mut admission)?;
            let initial = session.read(
                "entry",
                ReadRequest {
                    snapshot: &source,
                    start: 0,
                    limit: 1,
                    final_input: !streaming,
                    context: &proof,
                    state: &NdfValue::Unit,
                },
                &store,
                &mut b,
                &mut admission,
            )?;
            let ReadReply::Await { continuation, .. } = initial else {
                return Err(ReaderError::NoPending);
            };
            let provider = reply(&schema, streaming, invalid, &b);
            let before = b.usage().work;
            let result = session.resume(&continuation, provider, &store, &mut b, &mut admission);
            let result = if invalid {
                assert!(matches!(result, Err(ReaderError::ProviderContract)));
                let checked_work = b.usage().work - before;
                if mode == 2 {
                    // Calibrate the unchanged boundary prefix with the rejected
                    // reply. Leave one less Work than its complete lookup; the
                    // final operation comparison must now stop. Unit tests
                    // independently account every candidate in that lookup.
                    let remaining = b.limits().work - b.usage().work;
                    b.charge(Resource::Work, remaining - (checked_work - 1))?;
                    let provider = reply(&schema, streaming, true, &b);
                    let stopped =
                        session.resume(&continuation, provider, &store, &mut b, &mut admission)?;
                    let ReadReply::Stopped {
                        reason,
                        report,
                        sources,
                        source_maps,
                    } = stopped
                    else {
                        return Err(ReaderError::Context);
                    };
                    assert_eq!(reason, StopReason::WorkLimit);
                    assert!(sources.is_empty() && source_maps.is_empty());
                    assert!(report.diagnostics.is_empty() && report.events.is_empty());
                    assert_eq!(report.usage, b.usage());
                    let provider = reply(&schema, streaming, false, &b);
                    assert!(matches!(
                        session.resume(&continuation, provider, &store, &mut b, &mut admission),
                        Err(ReaderError::NoPending)
                    ));
                    continue;
                }
                // A semantic rejection must retain the same pending slot.
                let corrected = reply(&schema, streaming, false, &b);
                session.resume(&continuation, corrected, &store, &mut b, &mut admission)
            } else {
                result
            };
            match result? {
                ReadReply::NeedMore { expected, .. } if streaming => assert_eq!(expected.len(), 1),
                ReadReply::NoMatch { expected, .. } if !streaming => assert_eq!(expected.len(), 1),
                _ => return Err(ReaderError::Context),
            }
        }
    }
    Ok(())
}

fn reply(schema: &SchemaRef, streaming: bool, invalid: bool, b: &Budget) -> ProviderReply {
    let expected = vec![Expectation::Provider {
        operation: OperationRef {
            schema: schema.clone(),
            name: if invalid { "missing" } else { "read" }.into(),
        },
        arguments: TypedValue::Record(Record {
            schema: schema.clone(),
            kind: "Node".into(),
            fields: vec![],
        }),
    }];
    let report = Report {
        usage: b.usage(),
        ..Report::default()
    };
    ProviderReply::Read(Box::new(if streaming {
        ReadReply::NeedMore {
            sources: vec![],
            source_maps: vec![],
            expected,
            report,
        }
    } else {
        ReadReply::NoMatch {
            sources: vec![],
            source_maps: vec![],
            furthest: 0,
            expected,
            report,
        }
    }))
}
