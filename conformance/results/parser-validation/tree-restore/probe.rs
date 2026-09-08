#[test]
fn review_complete_restore_public_paths_and_private_validation_fault() -> TestResult {
    use std::sync::atomic::Ordering::Relaxed;
    REVIEW_MODE.store(1, Relaxed);
    REVIEW_OK.store(0, Relaxed);
    REVIEW_STOP.store(0, Relaxed);
    REVIEW_ERROR.store(0, Relaxed);
    // Expected structures derive from formal fixed arity, recovery and explicit
    // generated text inputs; the scratch observer compares the entire progress.
    for (input, text, provider) in [
        ("let x y", false, true),
        ("let", false, false),
        ("let \"x\\n\" y", true, true),
        ("let x let y let z q", false, true),
    ] {
        let make = |cap, work| Scenario { text, provider, cap, work,
            native: Some(host::Action::Serve), ..Scenario::default() };
        let complete = run_scenario(input, true, make(None,None))?;
        assert!(matches!(complete.outcome, ParseOutcome::Complete {..}|ParseOutcome::Recovered {..}));
        let needed=complete.report.usage;
        for part in 0..=96u64 {
            for resource in 0..2 {
                let cap = if resource==0 { Some(needed.allocation_units*part/96) } else { None };
                let work = if resource==1 { Some(needed.work*part/96) } else { None };
                let r=run_scenario(input,true,make(cap,work))?;
                if let Some(cap)=cap { assert!(r.report.usage.allocation_units<=cap); }
                if let Some(cap)=work { assert!(r.report.usage.work<=cap); }
                if let ParseOutcome::Stopped {progress:Some(ref p),..}=r.outcome {
                    for arena in &p.arenas {
                        for mapping in &arena.source_maps {
                            for span in [&mapping.source,&mapping.target] {
                                assert!(arena.sources.iter().chain(r.sources.iter()).any(|s|s.identity()==span.snapshot_ref()));
                            }
                        }
                    }
                }
                if part==96 { assert_eq!(r.outcome,complete.outcome); assert_eq!(r.sources,complete.sources); assert_eq!(r.source_maps,complete.source_maps); assert_eq!(r.report.diagnostics,complete.report.diagnostics); }
            }
        }
    }
    assert!(REVIEW_OK.load(Relaxed)>0);
    assert!(REVIEW_STOP.load(Relaxed)>0,"must actually exercise stopped internal validation");
    // Fault injection changes only ephemeral tree.profile_digest, which is
    // not a progress field. Real immutable validator returns Selection.
    // This is restore/error-path evidence, not an externally reachable attack.
    REVIEW_MODE.store(2,Relaxed);
    let bad=run_scenario("let \"x\\n\" y",true,Scenario {text:true,provider:true,native:Some(host::Action::Serve),..Scenario::default()});
    REVIEW_MODE.store(1,Relaxed);
    assert!(bad.as_ref().is_err_and(|e|e.contains("Selection")),"{bad:?}");
    assert!(REVIEW_ERROR.load(Relaxed)>0);
    println!("REVIEW_RESTORED ok={} stopped={} error={}",REVIEW_OK.load(Relaxed),REVIEW_STOP.load(Relaxed),REVIEW_ERROR.load(Relaxed));
    Ok(())
}
