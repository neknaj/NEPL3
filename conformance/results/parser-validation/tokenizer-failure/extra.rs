#[test]
fn independent_read_seed_rejected_budget_must_not_erase_recorded_usage()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;
 let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![]}];let mut session=TokenizationSession::new("usage".into(),&modes,&checked,&registry,&mut budget())?;
 let mut original=budget();original.charge(Resource::Work,123)?;
 let accepted=AcceptedTokenizationReport::empty(TokenizationScope{operation_id:"operation".into(),profile_digest:Digest([3;32]),snapshot:s.reference()},&mut original)?;
 let before=accepted.report.usage;let mut regressed=budget();let mut error=None;
 let result=session.read_seed(TokenTarget::Mode,TokenizationRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut regressed,&mut SourceAdmission::default(),accepted,None,&mut error);
 let Err(failure)=result else{return Err(ReaderError::Context)};assert_eq!(failure.error,ReaderError::Continuation);
 println!("actual read_seed old work={} allocation={}, rejected current={} {}, returned={} {}",before.work,before.allocation_units,regressed.usage().work,regressed.usage().allocation_units,failure.accepted.report.usage.work,failure.accepted.report.usage.allocation_units);
 assert!(failure.accepted.report.usage.work>=before.work,"rejected caller cannot refund accepted prefix work");Ok(())
}
fn seed(schema:&SchemaRef,s:&SourceSnapshot,store:&SourceStore,registry:&SchemaRegistry,b:&mut Budget)->Result<AcceptedTokenizationReport,ReaderError>{
 let mut accepted=AcceptedTokenizationReport::empty(TokenizationScope{operation_id:"op".into(),profile_digest:Digest([3;32]),snapshot:s.reference()},b)?;let saved=b.usage();let ProviderReply::Read(reply)=annotated_terminal(schema,s.span(0,1)?,1,b)? else{return Err(ReaderError::Context)};let ReadReply::Matched{report,..}=*reply else{return Err(ReaderError::Context)};
 accepted.append_report(report,saved,store,registry,b,&mut SourceAdmission::default())?;
 accepted.source_maps.push(nepl3_core::origin::Mapping{source:s.span(0,1)?,target:s.span(1,2)?,kind:nepl3_core::origin::MappingKind::Exact});Ok(accepted)
}
#[test]
fn independent_read_seed_actual_errors_and_scope_gate()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;
 let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![TakeRule{reader:TokenReader::Rule("entry".into()),kind:KindRef{schema:schema.clone(),local_kind:0}}]}];
 for mode in 0..6{
  let mut raw=context(&schema,&registry)?;if mode==2{raw.mode="absent".into();}let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;
  let mut b=budget();let mut session=TokenizationSession::new("actual".into(),&modes,&checked,&registry,&mut b)?;let mut admission=SourceAdmission::default();let mut error=None;
  if mode==0{session.close();}
  if mode==1{let accepted=AcceptedTokenizationReport::empty(TokenizationScope{operation_id:"op".into(),profile_digest:Digest([3;32]),snapshot:s.reference()},&mut b)?;let first=session.read_seed(TokenTarget::Mode,TokenizationRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut admission,accepted,None,&mut error).map_err(|f|f.error)?;assert!(matches!(first.outcome,TokenizationOutcome::Await{..}));}
  let accepted=seed(&schema,&s,&store,&registry,&mut b)?;let expected=(accepted.report.diagnostics.clone(),accepted.report.events.clone(),accepted.sources.clone(),accepted.source_maps.clone());let ptr=(accepted.sources.as_ptr(),accepted.source_maps.as_ptr());let bad=NdfValue::Text("bad".into());let unit=NdfValue::Unit;
  let target=if mode==5{TokenTarget::Builtin{reader:BuiltinReader::Name,token_kind:KindRef{schema:schema.clone(),local_kind:u64::MAX}}}else{TokenTarget::Mode};
  let result=session.read_seed(target,TokenizationRequest{snapshot:&s,start:if mode==4{3}else{0},limit:2,final_input:true,context:&ctx,state:if mode==3{&bad}else{&unit}},&store,&mut b,&mut admission,accepted,None,&mut error);
  let Err(failure)=result else{return Err(ReaderError::Context)};assert_eq!((&failure.accepted.report.diagnostics,&failure.accepted.report.events,&failure.accepted.sources,&failure.accepted.source_maps),(&expected.0,&expected.1,&expected.2,&expected.3));assert_eq!((failure.accepted.sources.as_ptr(),failure.accepted.source_maps.as_ptr()),ptr);println!("actual seed mode={mode} error={:?}",failure.error);
 }
 let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;let mut b=budget();let mut session=TokenizationSession::new("scope".into(),&modes,&checked,&registry,&mut b)?;let accepted=seed(&schema,&s,&store,&registry,&mut b)?;let mut forged=accepted.scope().clone();forged.operation_id="other".into();assert!(matches!(session.read_with_accepted(ScopedTokenizationRequest{scope:&forged,target:TokenTarget::Mode,input:TokenizationRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit}},&store,&mut b,&mut SourceAdmission::default(),accepted),Err(ReaderError::Continuation)));Ok(())
}
#[test]
fn independent_finish_recover_actual_entry_failure_branches()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![]}];
 for mode in 0..6{
  let mut b=budget();let mut session=TokenizationSession::new("finish".into(),&modes,&checked,&registry,&mut b)?;let accepted=seed(&schema,&s,&store,&registry,&mut b)?;let expected=(accepted.report.diagnostics.clone(),accepted.report.events.clone(),accepted.sources.clone(),accepted.source_maps.clone());
  if mode!=3{session.scope=Some(Rc::clone(&accepted.scope));}
  let current=ReaderCheckpoint{cursor:0,state:NdfValue::Unit,view:ViewBundle{elements:vec![],roots:vec![]},facts:vec![],diagnostics:accepted.report.diagnostics,events:accepted.report.events,trace_overflow:accepted.report.trace_overflow,sources:accepted.sources,source_maps:accepted.source_maps};
  let machine=Machine{request:Input{snapshot:&s,start:0,initial_state:&NdfValue::Unit,limit:2,final_input:true,context:&ctx},mode:&modes[0],target:TokenTarget::Mode,phase:TokenizationPhase::Skip{next:0},current,trivia:vec![],waiting:mode>=2,expected:vec![],furthest:0,limits:b.limits(),depth_base:1};
  let result=match mode{0=>Err(ReaderError::Context),1=>{b.stop(StopReason::WorkLimit);Err(ReaderError::Stopped(StopReason::WorkLimit))},2=>Ok(Outcome::End),_=>Ok(Outcome::Reserve{request:ReservationRequest{session_id:"finish".into(),request_id:0,snapshot:s.reference(),start:0,limit:2}})};
  let mut machine=machine;if mode==5{machine.waiting=false;}if mode==4{b.stop(StopReason::AllocationLimit);}
  match session.finish_recover(machine,result,&mut b){Err(f)=>{assert!([0,2,3,5].contains(&mode));assert_eq!((&f.accepted.report.diagnostics,&f.accepted.report.events,&f.accepted.sources,&f.accepted.source_maps),(&expected.0,&expected.1,&expected.2,&expected.3));},Ok(reply)=>{assert!([1,4].contains(&mode));assert!(matches!(reply.outcome,TokenizationOutcome::Stopped{..}));assert_eq!((&reply.report.diagnostics,&reply.report.events,&reply.sources,&reply.source_maps),(&expected.0,&expected.1,&expected.2,&expected.3));}}
  assert!(session.pending.is_none());println!("finish mode={mode} retained collector; no pending installed");
 }
 Ok(())
}
#[test]
fn independent_read_seed_foreign_limits_do_not_replace_usage()->Result<(),ReaderError>{
 let (registry,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget())?;let s=source("aa")?;let mut store=SourceStore::default();store.insert(s.clone())?;let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut budget(),&mut SourceAdmission::default())?;let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![]}];let mut session=TokenizationSession::new("foreign-budget".into(),&modes,&checked,&registry,&mut budget())?;
 let mut b=budget();let accepted=seed(&schema,&s,&store,&registry,&mut b)?;let original=accepted.report.clone();let mut limits=b.limits();limits.work+=1;let mut foreign=Budget::new(limits);foreign.record_observed_usage(b.usage())?;foreign.charge(Resource::Work,1000)?;let mut error=None;
 let result=session.read_seed(TokenTarget::Mode,TokenizationRequest{snapshot:&s,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut foreign,&mut SourceAdmission::default(),accepted,None,&mut error);
 let Err(failure)=result else{return Err(ReaderError::Context)};assert_eq!(failure.error,ReaderError::Continuation);assert_eq!(failure.accepted.report,original);println!("different Limits and larger Usage rejected without copying unrelated observation into original Report");Ok(())
}
