
#[test]
fn reviewer_source_conflicts_and_cost() -> Result<(),String> {
 let r=registry()?;
 for variant in 0..3 {
  for front in [false,true] {
   let mut document=literal(&r)?;
   let first=&document.sources[0];
   let copy=SourceSnapshot::new(SourceId("doc".into()),3,if variant==1 {"memory:conflict"} else {"memory:doc"}.into(),if variant==2 {b"changed".to_vec()}else{first.text().as_bytes().to_vec()},&mut b()).map_err(err)?;
   if front{document.sources.insert(0,copy);}else{document.sources.push(copy);}
   let original=document.clone();let mut budget=b();
   let result=document.validate_structure(&r,&mut budget,&mut SourceAdmission::default());
   assert!(result.is_err());eprintln!("REVIEW_CONFLICT {variant} front={front} error={:?}",result.err());assert_eq!(document,original);
  }
 }
 for count in [0,16,64,256] {
  let mut document=literal(&r)?;
  for i in 0..count {document.sources.push(SourceSnapshot::new(SourceId(format!("fixture/{i:04}")),0,"memory:fixture".into(),Vec::new(),&mut b()).map_err(err)?);}
  let original=document.clone();let mut budget=b();document.validate_structure(&r,&mut budget,&mut SourceAdmission::default()).map_err(err)?;assert_eq!(document,original);
  eprintln!("REVIEW_COST {count} {:?}",budget.usage());
  let mut stopped=b();stopped.cancel();assert!(document.validate_structure(&r,&mut stopped,&mut SourceAdmission::default()).is_err());assert_eq!(stopped.poll(),Err(StopReason::Cancelled));
  let mut limited=Budget::new(Limits{work:1,..b().limits()});assert!(document.validate_structure(&r,&mut limited,&mut SourceAdmission::default()).is_err());assert_eq!(limited.poll(),Err(StopReason::WorkLimit));
 }
 Ok(())
}
