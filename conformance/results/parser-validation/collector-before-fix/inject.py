from pathlib import Path
p=Path(__file__).parent/'workspace/crates/foundation/reader/src/tokenizer/recovery/tests.rs'
p.write_text(p.read_text(encoding='utf-8')+'''
fn reviewer_wrapper(short: Option<usize>, stopped: bool) -> RecoverableHostReply {
    let mut saved = prefix();
    if let Some(field) = short { saved.lengths[field] = 1; }
    let accepted = prefix().restore(live()).ok().expect("fixture");
    RecoverableHostReply {
        prefix: saved,
        inner: TokenizationHostReply {
            host_error: Some(ReaderError::ProviderContract),
            reply: AcceptedTokenizationReply {
                outcome: if stopped { TokenizationOutcome::Stopped { reason: StopReason::Cancelled } } else { TokenizationOutcome::End },
                cursor: 17, new_state: None, trivia: vec![], facts: vec![], accepted,
            },
        },
    }
}
#[test]
fn reviewer_deferred_broken_prefix_not_recovered_even_when_cancelled() {
    for field in 0..4 {
        let mut b=Budget::new(prefix().limits);
        b.record_observed_usage(live().report.usage).expect("fixture");
        b.cancel();
        assert!(matches!(reviewer_wrapper(Some(field),false).reject(&b), Err(AcceptedTokenizationFailure::BrokenPrefix { observed_stop: Some(StopReason::Cancelled), .. })));
    }
}
#[test]
fn reviewer_deferred_normal_stop_keeps_live_and_nonstop_recovers() {
    let mut b=Budget::new(prefix().limits);
    b.record_observed_usage(live().report.usage).expect("fixture");
    b.cancel();
    let stopped=reviewer_wrapper(None,true).reject(&b).expect("live stopped");
    assert_eq!(stopped.cursor,17);
    assert!(matches!(stopped.outcome,TokenizationOutcome::Stopped{reason:StopReason::Cancelled}));
    assert!(matches!(reviewer_wrapper(None,false).reject(&b),Err(AcceptedTokenizationFailure::Recoverable{error:ReaderError::ProviderContract,..})));
}
''',encoding='utf-8',newline='\n')
