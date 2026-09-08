from pathlib import Path
r=Path(__file__).parent;w=r/'workspace'
p=w/'crates/foundation/engine/tests/parse.rs';s=p.read_text(encoding='utf-8');needle='    if unknown {';insert='''    if forms.is_some_and(|f|f.first()==Some(&"beta")) {
        let mut category=package.categories[0].clone();category.name="Other".into();package.categories.push(category);
        let mut other=package.forms[0].clone();other.category="Other".into();package.forms.insert(0,other);
    }
''';assert s.count(needle)==1;s=s.replace(needle,insert+needle)
s+='''
#[test]
fn reviewer_category_unicode_prefix_and_original_index() -> TestResult {
    const FORMS:&[&str]=&["beta","日本語","a","aa","café","α"];
    for (index,word) in FORMS.iter().enumerate() {
        let result=run_scenario(&format!("{word} x y"),true,Scenario{forms:Some(FORMS),..Scenario::default()})?;
        let ParseOutcome::Complete{tree,..}=result.outcome else { return Err("complete expected".into()); };
        assert_eq!(tree.contexts[0].nodes[0].shape,nepl3_engine::selection::ShapeSelection::Form{index:(index+1) as u64});
        eprintln!("REVIEW_FORM {word} index={} usage={:?}",index+1,result.report.usage);
    }
    for word in ["b","bet","betaa","日本","cafe","αα"] {
        let result=run_scenario(word,true,Scenario{forms:Some(FORMS),..Scenario::default()})?;
        let ParseOutcome::Complete{tree,..}=result.outcome else { return Err("leaf expected".into()); };
        assert_eq!(tree.contexts[0].nodes[0].shape,nepl3_engine::selection::ShapeSelection::Leaf{index:0});
    }
    Ok(())
}
''';p.write_text(s,encoding='utf-8',newline='\n')
p=w/'crates/foundation/engine/tests/package.rs';s=p.read_text(encoding='utf-8')+'''
#[test]
fn reviewer_public_check_budget_and_subject_preservation() -> TestResult {
    use nepl3_core::budget::StopReason;
    let (mut package,registry)=fixture()?;
    let template=package.forms[0].clone();
    package.forms=(0..12).rev().map(|i|{let mut f=template.clone();f.spelling=format!("form{i:02}");f}).collect();
    let original=package.clone();
    let mut full=budget();
    let identity=package.check(&registry,&mut full).map_err(|e|format!("{e:?}"))?.semantic_identity(&mut budget()).map_err(|e|format!("{e:?}"))?;
    let needed=full.usage();
    for resource in 0..2 {
        for cap in [0,1,if resource==0 {needed.work-1}else{needed.allocation_units-1}] {
            let mut limits=budget().limits();if resource==0 {limits.work=cap}else{limits.allocation_units=cap};
            let mut b=Budget::new(limits);assert!(package.check(&registry,&mut b).is_err());
            assert_eq!(b.poll(),Err(if resource==0 {StopReason::WorkLimit}else{StopReason::AllocationLimit}));
            assert_eq!(package,original);
        }
    }
    let mut stopped=budget();stopped.cancel();assert!(package.check(&registry,&mut stopped).is_err());assert_eq!(stopped.poll(),Err(StopReason::Cancelled));
    assert_eq!(identity,package.check(&registry,&mut budget()).map_err(|e|format!("{e:?}"))?.semantic_identity(&mut budget()).map_err(|e|format!("{e:?}"))?);
    assert_eq!(package,original);eprintln!("REVIEW_CHECK {needed:?} identity={identity:?}");
    Ok(())
}
''';p.write_text(s,encoding='utf-8',newline='\n')
