use super::*;

fn reply(
    relation: bool,
    schema: &SchemaRef,
    name: &str,
    span: &Span,
    usage: Usage,
) -> ProviderReply {
    let fact = if relation {
        ReaderFact::Relation {
            schema: schema.clone(),
            kind: name.into(),
            from: span.clone(),
            to: span.clone(),
        }
    } else {
        ReaderFact::Presentation {
            class: PresentationClass {
                fallback: FallbackRole::Content,
                schema: schema.clone(),
                name: name.into(),
            },
            span: span.clone(),
        }
    };
    ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text("b".into()),
        end: span.end(),
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![fact],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage,
            ..Report::default()
        },
    }))
}
#[test]
fn fact_metadata_lookup_is_metered_and_invalid_replies_remain_retryable() -> Result<(), ReaderError>
{
    for padding in [0, 32] {
        let (registry, schema) = registry_with_padding(padding)?;
        let p = provider_plan(&schema);
        let checked = p.check(&registry, &mut budget())?;
        let input = source("a")?;
        let mut sources = SourceStore::default();
        sources.insert(input.clone())?;
        let raw = context(&schema, &registry)?;
        for relation in [false, true] {
            let mut b = budget();
            let mut admission = SourceAdmission::default();
            let proof = check_context(&raw, &sources, &registry, &mut b, &mut admission)?;
            let mut session = ReaderSession::new("facts".into(), &checked, &registry, &mut b)?;
            let initial = session.read(
                "entry",
                ReadRequest {
                    snapshot: &input,
                    start: 0,
                    limit: 1,
                    final_input: true,
                    context: &proof,
                    state: &NdfValue::Unit,
                },
                &sources,
                &mut b,
                &mut admission,
            )?;
            let ReadReply::Await { continuation, .. } = initial else {
                return Err(ReaderError::NoPending);
            };
            let span = input.span(0, 1)?;
            let mut missing = schema.clone();
            missing.package = "missing".into();
            let empty = reply(relation, &missing, "", &span, b.usage());
            let before = b.usage();
            assert!(matches!(
                session.resume(&continuation, empty, &sources, &mut b, &mut admission),
                Err(ReaderError::ProviderContract)
            ));
            let empty_work = b.usage().work - before.work;
            let empty_allocation = b.usage().allocation_units - before.allocation_units;
            let bad = reply(relation, &missing, "any-vocabulary", &span, b.usage());
            let before = b.usage();
            assert!(matches!(
                session.resume(&continuation, bad, &sources, &mut b, &mut admission),
                Err(ReaderError::ProviderContract)
            ));
            let padding_work: u64 = (0..padding)
                .map(|i| format!("unrelated.{i}").len() as u64 + 7 + 9)
                .sum();
            assert_eq!(
                b.usage().work - before.work,
                empty_work + 1 + (16 + 7 + 9) + (12 + 7 + 9) + (4 + 7 + 9) + padding_work
            );
            assert_eq!(
                b.usage().allocation_units - before.allocation_units,
                empty_allocation
            );
            let mut wrong = schema.clone();
            wrong.digest.0[0] ^= 1;
            let bad = reply(relation, &wrong, "any-vocabulary", &span, b.usage());
            let before = b.usage();
            assert!(matches!(
                session.resume(&continuation, bad, &sources, &mut b, &mut admission),
                Err(ReaderError::ProviderContract)
            ));
            let padding_work: u64 = (0..padding)
                .map(|i| format!("unrelated.{i}").len() as u64 + 4 + 9)
                .sum();
            assert_eq!(
                b.usage().work - before.work,
                empty_work
                    + 1
                    + (16 + 4 + 9)
                    + (12 + 4 + 9)
                    + (4 + 4 + 9)
                    + (4 + 41)
                    + padding_work
            );
            let valid = reply(relation, &schema, "not-a-descriptor-type", &span, b.usage());
            let accepted =
                session.resume(&continuation, valid, &sources, &mut b, &mut admission)?;
            assert!(matches!(accepted,ReadReply::Matched{facts,..}if facts.len()==1));
        }
    }
    Ok(())
}

#[test]
fn fact_lookup_stop_keeps_only_the_accepted_collector_and_consumes_pending()
-> Result<(), ReaderError> {
    use nepl3_core::origin::{Mapping, MappingKind};
    let (registry, schema) = registry()?;
    let mut p = provider_plan(&schema);
    p.expressions
        .push(ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]));
    p.rules[0].root = ReaderId(1);
    p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked = p.check(&registry, &mut budget())?;
    let input = source("ab")?;
    let mut sources = SourceStore::default();
    sources.insert(input.clone())?;
    let raw = context(&schema, &registry)?;
    for relation in [false, true] {
        let mut b = budget();
        let mut admission = SourceAdmission::default();
        let proof = check_context(&raw, &sources, &registry, &mut b, &mut admission)?;
        let mut session = ReaderSession::new("facts-prefix".into(), &checked, &registry, &mut b)?;
        let initial = session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 2,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &sources,
            &mut b,
            &mut admission,
        )?;
        let ReadReply::Await { continuation, .. } = initial else {
            return Err(ReaderError::NoPending);
        };
        let generated = admission.create(
            SourceId("accepted".into()),
            0,
            "memory:accepted".into(),
            b"g".to_vec(),
            &mut b,
        )?;
        let mut first = annotated_terminal(&schema, generated.span(0, 1)?, 1, &mut b)?;
        if let ProviderReply::Read(value) = &mut first
            && let ReadReply::Matched {
                sources,
                source_maps,
                ..
            } = value.as_mut()
        {
            sources.push(generated.clone());
            source_maps.push(Mapping {
                source: input.span(0, 1)?,
                target: generated.span(0, 1)?,
                kind: MappingKind::Transformed,
            });
        }
        let next = session.resume(&continuation, first, &sources, &mut b, &mut admission)?;
        let ReadReply::Await {
            continuation,
            report: prefix,
            ..
        } = next
        else {
            return Err(ReaderError::NoPending);
        };
        assert_eq!(prefix.diagnostics.len(), 1);
        assert_eq!(prefix.events.len(), 1);
        let prior_maps = continuation.current.source_maps.clone();
        assert_eq!(prior_maps.len(), 1);
        let mut missing = schema.clone();
        missing.package = "missing".into();
        let span = input.span(1, 2)?;
        let empty = reply(relation, &missing, "", &span, b.usage());
        let before = b.usage().work;
        assert!(matches!(
            session.resume(&continuation, empty, &sources, &mut b, &mut admission),
            Err(ReaderError::ProviderContract)
        ));
        let prefix_work = b.usage().work - before;
        let remaining = b.limits().work - b.usage().work;
        b.charge(Resource::Work, remaining - prefix_work)?;
        let bad = reply(relation, &missing, "any-vocabulary", &span, b.usage());
        let stopped = session.resume(&continuation, bad, &sources, &mut b, &mut admission)?;
        let ReadReply::Stopped {
            reason,
            report,
            sources: kept,
            source_maps,
        } = stopped
        else {
            return Err(ReaderError::ProviderContract);
        };
        assert_eq!(reason, StopReason::WorkLimit);
        assert_eq!(report.usage, b.usage());
        assert_eq!(report.diagnostics, prefix.diagnostics);
        assert_eq!(report.events, prefix.events);
        assert_eq!(report.trace_overflow, prefix.trace_overflow);
        assert_eq!(kept, vec![generated]);
        assert_eq!(source_maps, prior_maps);
        assert_eq!(b.usage().work, b.limits().work);
        let valid = reply(relation, &schema, "valid", &span, b.usage());
        assert!(matches!(
            session.resume(&continuation, valid, &sources, &mut b, &mut admission),
            Err(ReaderError::NoPending)
        ));
    }
    Ok(())
}
