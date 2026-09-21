use super::*;
use nepl3_core::{source::*, value::NdfValue};
use nepl3_engine::{parse::*, portable};
use nepl3_wire::foundation::FoundationCodec;

fn run(input: &str, final_input: bool, stopped: bool) -> Result<ParseReply, String> {
    crate::parse::with_reply(
        input,
        final_input,
        stopped,
        |reply, resolved, registry, codec| {
            if let ParseOutcome::Complete { tree, .. } | ParseOutcome::Recovered { tree, .. } =
                &reply.outcome
            {
                // Both boundaries call production adapters. NDF canonical node ordering need not equal native arena order.
                let mut transfer = budget();
                let value = portable::tree::to_value(tree, resolved, codec, &mut transfer)
                    .map_err(error)?;
                let bytes = nepl3_wire::encode(&value, &mut transfer).map_err(error)?;
                let decoded = nepl3_wire::decode(&bytes, &mut transfer).map_err(error)?;
                // The receiver has no ambient access to the sender's source store.
                let receiving_sources = SourceStore::default();
                let mut receiving_admission = SourceAdmission::default();
                let mut receiver =
                    FoundationCodec::new(registry, &receiving_sources, &mut receiving_admission)
                        .map_err(error)?;
                let restored =
                    portable::tree::from_value(&decoded, resolved, &mut receiver, &mut transfer)
                        .map_err(error)?;
                let again =
                    portable::tree::to_value(&restored, resolved, &mut receiver, &mut transfer)
                        .map_err(error)?;
                assert_eq!(value, again);
                assert_eq!(tree.bundle.sources, restored.bundle.sources);
                assert_eq!(tree.bundle.tokens, restored.bundle.tokens);
                // An input corruption must be rejected, not converted into a successful parse tree.
                assert!(
                    portable::tree::from_value(&NdfValue::Unit, resolved, codec, &mut transfer)
                        .is_err()
                );
                assert!(nepl3_wire::decode(&bytes[..bytes.len() - 1], &mut transfer).is_err());
            }
            Ok(reply)
        },
    )
}

#[test]
fn public_observer_accepts_a_different_recipient() -> Result<(), String> {
    let reply = crate::parse("hello NEPL3", true)?;
    assert!(reply.report.diagnostics.is_empty());
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err("expected complete".into());
    };
    assert_eq!(cursor, 11);
    let token = tree.bundle.tokens.last().ok_or("recipient")?;
    assert_eq!(token.payload, NdfValue::Text("NEPL3".into()));
    // The head and space occupy six ASCII bytes; the recipient occupies five.
    assert_eq!((token.head.start(), token.head.end()), (6, 11));
    assert!(matches!(
        crate::parse("hello", true)?.outcome,
        ParseOutcome::Recovered { .. }
    ));
    Ok(())
}

#[test]
fn unicode_source_and_external_kind_survive_native_and_ndf() -> Result<(), String> {
    let reply = run("hello 世界", true, false)?;
    assert!(reply.report.diagnostics.is_empty());
    let ParseOutcome::Complete { tree, cursor, .. } = reply.outcome else {
        return Err("expected complete".into());
    };
    assert_eq!(cursor, 12); // six ASCII bytes, then two three-byte UTF-8 scalars
    let root = &tree.bundle.nodes[tree.bundle.root.0 as usize];
    assert_eq!(root.schema.package, "org.example.hello");
    assert_eq!(root.kind, "Greeting");
    assert_eq!(root.fields.len(), 1);
    let token = tree.bundle.tokens.last().ok_or("recipient")?;
    assert_eq!(token.payload, NdfValue::Text("世界".into()));
    assert_eq!((token.head.start(), token.head.end()), (6, 12));
    assert_eq!(tree.bundle.sources[0].identity().revision, 7);
    assert!(
        tree.bundle
            .origins
            .contains(&nepl3_core::origin::Origin::Direct(token.head.clone()))
    );
    Ok(())
}

#[test]
fn unknown_head_is_recovery_not_invented_zero_arity() -> Result<(), String> {
    let reply = run("goodbye 世界", true, false)?;
    assert!(
        reply
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "UnparsedInput" && diagnostic.stage == "parse")
    );
    assert!(matches!(reply.outcome, ParseOutcome::Recovered { .. }));
    Ok(())
}

#[test]
fn unfinished_source_and_cancellation_remain_distinct() -> Result<(), String> {
    assert!(matches!(
        run("hello ", false, false)?.outcome,
        ParseOutcome::NeedMore { .. }
    ));
    assert!(matches!(
        run("hello 世界", true, true)?.outcome,
        ParseOutcome::Stopped {
            reason: nepl3_core::budget::StopReason::Cancelled,
            ..
        }
    ));
    Ok(())
}
