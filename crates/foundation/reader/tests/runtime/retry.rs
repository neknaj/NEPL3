use super::*;

#[test]
fn rejected_read_payload_state_end_source_and_view_preserve_formal_collector_and_pending()
-> Result<(), ReaderError> {
    let (registry, schema) = registry()?;
    for mutation in 0..5 {
        let mut p = provider_plan(&schema);
        p.expressions
            .push(ReaderExpr::Seq(vec![ReaderId(0), ReaderId(0)]));
        p.rules[0].root = ReaderId(1);
        p.rules[0].output = TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
        let checked = p.check(&registry, &mut budget())?;
        let mut b = budget();
        let mut session = ReaderSession::new("retry".into(), &checked, &registry, &mut b)?;
        let input = source("ab")?;
        let mut sources = SourceStore::default();
        sources.insert(input.clone())?;
        let mut admission = SourceAdmission::default();
        let raw = context(&schema, &registry)?;
        let proof = check_context(&raw, &sources, &registry, &mut b, &mut admission)?;
        let first = session.read(
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
        let ReadReply::Await { continuation, .. } = first else {
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
        if let ProviderReply::Read(v) = &mut first
            && let ReadReply::Matched { sources, .. } = v.as_mut()
        {
            sources.push(generated.clone());
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
        let valid = terminal("b", 2, &mut b)?;
        let mut bad = match &valid {
            ProviderReply::Read(v) => ProviderReply::Read(v.clone()),
            ProviderReply::Transform(v) => ProviderReply::Transform(v.clone()),
        };
        let ProviderReply::Read(bad_reply) = &mut bad else {
            return Err(ReaderError::ProviderContract);
        };
        let ReadReply::Matched {
            value,
            new_state,
            end,
            view,
            sources: added,
            ..
        } = bad_reply.as_mut()
        else {
            return Err(ReaderError::ProviderContract);
        };
        match mutation {
            0 => *value = NdfValue::Unit,
            1 => *new_state = NdfValue::Text("wrong state".into()),
            2 => *end = 3,
            3 => {
                added.push(SourceSnapshot::new(
                    SourceId("candidate".into()),
                    0,
                    "memory:candidate".into(),
                    b"h".to_vec(),
                    &mut budget(),
                )?);
                added.push(SourceSnapshot::new(
                    SourceId("accepted".into()),
                    0,
                    "memory:wrong".into(),
                    b"g".to_vec(),
                    &mut budget(),
                )?);
            }
            _ => view.roots.push(ViewRef(99)),
        }
        let before = b.usage();
        assert!(
            session
                .resume(&continuation, bad, &sources, &mut b, &mut admission)
                .is_err()
        );
        assert_eq!(b.poll(), Ok(()));
        assert!(b.usage().work > before.work);
        assert_eq!(b.usage().diagnostics, before.diagnostics);
        assert_eq!(b.usage().events, before.events);
        let result = session.resume(&continuation, valid, &sources, &mut b, &mut admission)?;
        let ReadReply::Matched {
            value,
            sources,
            report,
            ..
        } = result
        else {
            return Err(ReaderError::ProviderContract);
        };
        assert_eq!(
            value,
            NdfValue::List(vec![NdfValue::Text("a".into()), NdfValue::Text("b".into())])
        );
        assert_eq!(sources, vec![generated]);
        assert_eq!(report.diagnostics, prefix.diagnostics);
        assert_eq!(report.events, prefix.events);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.events.len(), 1);
    }
    Ok(())
}
