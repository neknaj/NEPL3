struct OwnHost{mode:u8}
struct PrefixHost{calls:u8}
impl crate::tokenizer::TokenizationHost for PrefixHost {
 fn provider(&mut self,call:&ProviderCall,b:&mut Budget,_:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError>{
  self.calls+=1;
  if self.calls==2{return Err(ReaderError::Context)}
  let ProviderCall::Read{operation,request,..}=call else{return Err(ReaderError::Context)};
  let snapshot=request.sources.iter().find(|s|s.reference()==request.snapshot).ok_or(ReaderError::Context)?;
  Ok(Some(annotated_terminal(&operation.schema,snapshot.span(request.start,request.start+1)?,request.start+1,b)?))
 }
 fn reservation(&mut self,_:&crate::tokenizer::ReservationRequest,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{Ok(None)}
}
impl crate::tokenizer::TokenizationHost for OwnHost {
 fn provider(&mut self,call:&ProviderCall,b:&mut Budget,_:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError>{
  match self.mode{1=>return Ok(None),2=>return Err(ReaderError::Context),4=>{b.cancel();return Ok(None)},5=>{b.charge(Resource::Work,u64::MAX)?;},6=>return Err(ReaderError::Stopped(StopReason::Cancelled)),_=>()}
  let ProviderCall::Read{request,..}=call else{return Err(ReaderError::Context)};
  let mut reply=terminal("a",request.start+1,b)?;
  if self.mode==3{if let ProviderReply::Read(v)=&mut reply{if let ReadReply::Matched{value,..}=v.as_mut(){*value=NdfValue::Unit;}}}
  Ok(Some(reply))
 }
 fn reservation(&mut self,_:&crate::tokenizer::ReservationRequest,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{Ok(None)}
}
fn seeded(schema:&SchemaRef,s:&SourceSnapshot,b:&mut Budget)->Result<crate::runtime::AcceptedReport,ReaderError>{
 let terminal=annotated_terminal(schema,s.span(0,1)?,1,b)?;
 let ProviderReply::Read(reply)=terminal else{return Err(ReaderError::Context)};
 let ReadReply::Matched{report,..}=*reply else{return Err(ReaderError::Context)};
 Ok(crate::runtime::AcceptedReport{report,sources:vec![s.clone()],source_maps:vec![nepl3_core::origin::Mapping{source:s.span(0,1)?,target:s.span(1,2)?,kind:nepl3_core::origin::MappingKind::Exact}]})
}
#[test]
fn independent_actual_preflight_owns_seed()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;
 let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;
 for mode in 0..5 {
  let mut setup=budget();let mut session=ReaderSession::new("ownership".into(),&checked,&registry,&mut setup)?;let mut admission=SourceAdmission::default();
  if mode==0{session.close();}
  if mode==1{let reply=session.read("entry",ReadRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut setup,&mut admission)?;assert!(matches!(reply,ReadReply::Await{..}));}
  let seed=seeded(&schema,&s,&mut setup)?;let ptrs=(seed.report.diagnostics.as_ptr(),seed.report.events.as_ptr(),seed.sources.as_ptr(),seed.source_maps.as_ptr());let expected=seed.report.clone();let expected_sources=seed.sources.clone();let expected_maps=seed.source_maps.clone();
  let bad_state=NdfValue::Text("invalid state".into());let rule=if mode==2{"missing"}else{"entry"};let state=if mode==3{&bad_state}else{&NdfValue::Unit};
  let request=ReadRequest{snapshot:&s,start:if mode==4{3}else{0},limit:2,final_input:true,context:&ctx,state};
  let result=session.read_with_report_host_recover(rule,request,&store,&mut setup,&mut admission,seed,None);
  let Err(failure)=result else{return Err(ReaderError::Context)};
  assert_eq!(failure.accepted.report,expected);assert_eq!(failure.accepted.sources,expected_sources);assert_eq!(failure.accepted.source_maps,expected_maps);
  assert_eq!((failure.accepted.report.diagnostics.as_ptr(),failure.accepted.report.events.as_ptr(),failure.accepted.sources.as_ptr(),failure.accepted.source_maps.as_ptr()),ptrs);
  println!("preflight mode={mode} error={:?} seed moved intact",failure.error);
 }
 Ok(())
}
#[test]
fn independent_actual_stops_and_native_host_preserve_seed()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;
 for mode in [0,1,2,3,4,5,6] {
  let mut setup=budget();let mut session=ReaderSession::new("ownership".into(),&checked,&registry,&mut setup)?;let mut admission=SourceAdmission::default();let seed=seeded(&schema,&s,&mut setup)?;let expected=(seed.report.diagnostics.clone(),seed.report.events.clone(),seed.sources.clone(),seed.source_maps.clone());
  let mut host=OwnHost{mode};let mut error=None;let mut native=crate::runtime::NativeDispatch{host:&mut host,error:&mut error,deferred:false};
  let reply=session.read_with_report_host_recover("entry",ReadRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut setup,&mut admission,seed,Some(&mut native)).map_err(|f|f.error)?;
  let (report,sources,maps)=match &reply {ReadReply::Await{report,continuation,..}=>(report,&continuation.current.sources,&continuation.current.source_maps),ReadReply::Matched{report,sources,source_maps,..}|ReadReply::Stopped{report,sources,source_maps,..}=>(report,sources,source_maps),_=>return Err(ReaderError::Context)};
  assert_eq!((&report.diagnostics,&report.events,sources,maps),(&expected.0,&expected.1,&expected.2,&expected.3));
  if mode>=4{assert!(matches!(reply,ReadReply::Stopped{..}));}else if mode!=0{assert!(matches!(reply,ReadReply::Await{..}));}
  println!("native host mode={mode}: retained seed diagnostics/events/source/map");
 }
 for allocation in [false,true] {let mut stopped=0;for cap in (0..1000).step_by(17) {
  let mut setup=budget();let mut session=ReaderSession::new("sweep".into(),&checked,&registry,&mut setup)?;let seed=seeded(&schema,&s,&mut setup)?;let expected=(seed.report.diagnostics.clone(),seed.report.events.clone(),seed.sources.clone(),seed.source_maps.clone());let observed=setup.usage();let mut limits=budget().limits();if allocation{limits.allocation_units=observed.allocation_units+cap}else{limits.work=observed.work+cap}let mut limited=Budget::new(limits);limited.record_observed_usage(observed)?;
  let result=session.read_with_report_host_recover("entry",ReadRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut limited,&mut SourceAdmission::default(),seed,None).map_err(|f|f.error)?;
  if let ReadReply::Stopped{reason,report,sources,source_maps}=result {assert_eq!((&report.diagnostics,&report.events,&sources,&source_maps),(&expected.0,&expected.1,&expected.2,&expected.3));assert_eq!(limited.poll(),Err(reason));stopped+=1;}
 }assert!(stopped>0);println!("actual request stop sweep allocation={allocation} preserved={stopped}");}
 Ok(())
}
#[test]
fn independent_actual_accepted_prefix_survives_second_host_failure()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let mut p=provider_plan(&schema);p.expressions.push(ReaderExpr::Seq(vec![ReaderId(0),ReaderId(0)]));p.rules[0].root=ReaderId(1);p.rules[0].output=TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));let checked=p.check(&registry,&mut budget())?;
 let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;
 let mut b=budget();let mut session=ReaderSession::new("two-calls".into(),&checked,&registry,&mut b)?;let seed=seeded(&schema,&s,&mut b)?;let expected=(seed.sources.clone(),seed.source_maps.clone());let mut admission=SourceAdmission::default();let mut error=None;let mut host=PrefixHost{calls:0};let mut native=crate::runtime::NativeDispatch{host:&mut host,error:&mut error,deferred:false};
 let reply=session.read_with_report_host_recover("entry",ReadRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut admission,seed,Some(&mut native)).map_err(|f|f.error)?;
 let ReadReply::Await{continuation,report,..}=reply else{return Err(ReaderError::Context)};
 assert_eq!(error,Some(ReaderError::Context));assert_eq!(host.calls,2);assert_eq!(report.diagnostics.len(),2);assert_eq!(report.events.len(),2);assert_eq!(continuation.current.sources,expected.0);assert_eq!(continuation.current.source_maps,expected.1);
 let reply=session.resume(&continuation,terminal("a",2,&mut b)?,&store,&mut b,&mut admission)?;
 let ReadReply::Matched{report,sources,source_maps,..}=reply else{return Err(ReaderError::Context)};assert_eq!(report.diagnostics.len(),2);assert_eq!(report.events.len(),2);assert_eq!(sources,expected.0);assert_eq!(source_maps,expected.1);
 println!("actual accepted first callback + second hard host error -> Await -> public resume: original and accepted new reports preserved");Ok(())
}
