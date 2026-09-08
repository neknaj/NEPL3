include!("review_helpers.rs");

#[test]
fn owned_frame_conversion_work_window()->Result<(),ReaderError> {
    let (r,schema)=registry()?;
    for depth in [32u64,128] {
        let read=signature(&schema,ProviderKind::Read);
        let mut xs=vec![ReaderExpr::Call(read.operation.clone())];
        for id in 0..depth {xs.push(ReaderExpr::Discard(ReaderId(id)));}
        let mut p=plan(&schema,xs,depth,TypeDescriptor::Unit);p.providers.push(read);
        let checked=p.check(&r,&mut budget())?;let input=source("a")?;let mut store=SourceStore::default();store.insert(input.clone())?;
        let raw=context(&schema,&r)?;let mut setup=budget();let mut setup_a=SourceAdmission::default();let ctx=check_context(&raw,&store,&r,&mut setup,&mut setup_a)?;
        let execute=|remaining:Option<u64>|->Result<(u64,u64,bool),ReaderError>{
            let mut b=budget();let mut a=SourceAdmission::default();let mut session=ReaderSession::new("conversion".into(),&checked,&r,&mut b)?;
            let reply=session.read("entry",ReadRequest {snapshot:&input,start:0,limit:1,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut a)?;
            let ReadReply::Await {continuation,..}=reply else {return Err(ReaderError::Context)};
            if let Some(remaining)=remaining {b.charge(Resource::Work,b.limits().work-b.usage().work-remaining)?;}
            let before=b.usage();let value=terminal("a",1,&mut b)?;
            let reply=session.resume(&continuation,value,&store,&mut b,&mut a)?;
            assert!(matches!(reply,ReadReply::Matched {..}|ReadReply::Stopped {reason:StopReason::WorkLimit,..}));
            Ok((b.usage().work-before.work,b.usage().allocation_units-before.allocation_units,matches!(reply,ReadReply::Matched {..})))
        };
        let full=execute(None)?;assert!(full.2);
        let mut prior=0;let mut largest=(0,0,0,0,false);
        for remaining in 1..=full.0 {
            let (work,allocation,complete)=execute(Some(remaining))?;
            let jump=allocation.saturating_sub(prior);
            if jump>largest.1 {largest=(remaining,jump,prior,allocation,complete)}
            assert!(work<=remaining);prior=allocation;
        }
        println!("depth={depth} full_resume={full:?} largest_adjacent_work_allocation_jump={largest:?}");
    }
    Ok(())
}
