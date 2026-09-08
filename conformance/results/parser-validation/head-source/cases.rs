    for count in [1,2,8,32] {
        let original=make(16)?;
        let mut all=vec![original.bundle.sources[0].clone()];
        for revision in 0..count-1 {all.push(SourceSnapshot::new(original.bundle.sources[0].identity().source.clone(),revision,"memory:unrelated".into(),b"not a form".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?);}
        for position in [0,count/2,count-1] {
            let mut tree=original.clone();tree.bundle.sources=all.clone();tree.bundle.sources.swap(0,position as usize);
            let saved=tree.clone();let mut b=budget();validate(&tree,&mut b).map_err(|e|format!("count{count} pos{position}: {e:?}"))?;assert_eq!(tree,saved);
            println!("COST count={count} position={position} {:?}",b.usage());
            for cap in [0,1,100,b.usage().work-1,b.usage().work,b.usage().work+1] {
                let mut limits=budget().limits();limits.work=cap;let mut limited=Budget::new(limits);let result=validate(&tree,&mut limited);
                if cap<b.usage().work {assert_eq!(result,Err(TreeError::Stopped(StopReason::WorkLimit)));assert_eq!(limited.poll(),Err(StopReason::WorkLimit));}else{assert!(result.is_ok());}
            }
        }
    }
    let original=make(1)?;
    let mut fallback=original.clone();fallback.bundle.sources.insert(0,SourceSnapshot::new(original.bundle.sources[0].identity().source.clone(),0,"memory:wrong-revision".into(),b"not a form".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?);
    let mut full=budget();validate(&fallback,&mut full).map_err(|e|format!("{e:?}"))?;
    for cap in (0..=full.usage().work+1).step_by(17) {
        let mut limits=budget().limits();limits.work=cap;let mut b=Budget::new(limits);let result=validate(&fallback,&mut b);
        assert!(result.is_ok() || result==Err(TreeError::Stopped(StopReason::WorkLimit)));
        println!("STOP cap={cap} result={result:?} work={}",b.usage().work);
    }
    for mode in 0..6 {
        let mut tree=original.clone();let source=&original.bundle.sources[0];
        match mode {
            0=>tree.bundle.sources.clear(),
            1=>tree.bundle.sources[0]=SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:tree".into(),b"bad x y".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?,
            2=>tree.bundle.sources[0]=SourceSnapshot::new(source.identity().source.clone(),0,"memory:tree".into(),source.text().as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?,
            3=>tree.bundle.sources.insert(0,SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:other-uri".into(),source.text().as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?),
            4=>tree.bundle.sources.push(source.clone()),
            _=>tree.bundle.sources.insert(0,SourceSnapshot::new(source.identity().source.clone(),source.identity().revision,"memory:tree".into(),b"bad x y".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?),
        }
        let result=validate(&tree,&mut budget());assert!(result.is_err());println!("ERROR mode={mode}: {result:?}");
    }
    let mut b=budget();b.charge(Resource::Work,9).map_err(|e|format!("{e:?}"))?;let usage=b.usage();b.cancel();assert_eq!(validate(&original,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(usage,b.usage());
    for cap in [0,1,100] {let mut limits=budget().limits();limits.allocation_units=cap;let mut b=Budget::new(limits);assert_eq!(validate(&original,&mut b),Err(TreeError::Stopped(StopReason::AllocationLimit)));}
    Ok(())
}
