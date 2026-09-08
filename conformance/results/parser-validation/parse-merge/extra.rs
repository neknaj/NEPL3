#[test]
fn independent_actual_closure_orders_duplicates_and_retry() -> TestResult {
    for order in 0..3 {
        for mode in [0,1,3] {
            let result = run_scenario("let x y tail", true, Scenario { provider: true, native: Some(host::Action::Closure { order, mode }), ..Scenario::default() });
            if mode==1 {assert_eq!(result.err().as_deref(),Some("Reader(Source(IdentityConflict))"));continue;}
            let reply = result?;
            let ParseOutcome::Complete { tree, cursor, .. } = &reply.outcome else {return Err(format!("not complete: {:?}",reply.outcome).into());};
            assert_eq!(*cursor,7);
            let aux: Vec<_> = tree.bundle.sources.iter().filter(|s|s.identity().source.0.starts_with("aux:")).collect();
            assert_eq!(aux.len(),32);
            for (at,s) in aux.iter().enumerate() {let i=match order{0=>at,1=>31-at,_=>(at*13)%32};assert!(s.identity().source.0.ends_with(&format!(":{:04}",i/2)));assert_eq!(s.identity().revision,if i%2==0{0}else{u64::MAX});assert_eq!(s.uri(),format!("memory:aux:{i}"));assert_eq!(s.text(),"abc");}
            assert_eq!(reply.report.usage.source_bytes,12+32*3+if mode==1{3}else{0});
            println!("public order={order} mode={mode} work={} allocation={} sourcebytes={}",reply.report.usage.work,reply.report.usage.allocation_units,reply.report.usage.source_bytes);
        }
    }
    Ok(())
}
#[test]
fn independent_actual_closure_stops_and_recovery() -> TestResult {
    let stopped=run_scenario("let x y tail",true,Scenario{provider:true,native:Some(host::Action::Closure{order:2,mode:2}),..Scenario::default()})?;
    assert!(matches!(stopped.outcome,ParseOutcome::Stopped{reason:nepl3_core::budget::StopReason::Cancelled,..}));
    for work in [0,100,1000,10000,50000] {
        let reply=run_scenario("let x y tail",true,Scenario{provider:true,work:Some(work),native:Some(host::Action::Closure{order:1,mode:0}),..Scenario::default()})?;
        assert!(matches!(reply.outcome,ParseOutcome::Stopped{reason:nepl3_core::budget::StopReason::WorkLimit,..}));
        assert!(reply.report.usage.work<=work);
    }
    let recovered=run_scenario("let x",true,Scenario{provider:true,native:Some(host::Action::Closure{order:2,mode:0}),..Scenario::default()})?;
    assert!(matches!(recovered.outcome,ParseOutcome::Recovered{..}));
    println!("public cancel, five unchanged Work caps, missing-field recovery");
    Ok(())
}
