use super::*;
use nepl3_core::origin::{Mapping,MappingKind};
fn error(e:impl core::fmt::Debug)->String{format!("{e:?}")}
#[test]
fn empty_transform_preserves_maps_and_checks_future_cross_cycle_and_roots()->Result<(),String>{
 for (portable,cancel) in [(false,false),(true,false),(false,true),(true,true)]{
  let (registry,schema)=registry().map_err(error)?;let read=signature(&schema,ProviderKind::Read);let transform=signature(&schema,ProviderKind::Transform);
  let mut plan=plan(&schema,vec![ReaderExpr::Call(read.operation.clone()),ReaderExpr::Scalar(CharClass::Any),ReaderExpr::Map{provider:transform.operation.clone(),body:ReaderId(1)},ReaderExpr::Seq(vec![ReaderId(0),ReaderId(2),ReaderId(0)])],3,TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)));plan.providers=vec![read,transform];
  let checked=plan.check(&registry,&mut budget()).map_err(error)?;let mut b=budget();let mut admission=SourceAdmission::default();let input=source("abc").map_err(error)?;let mut store=SourceStore::default();store.insert(input.clone()).map_err(error)?;let raw=context(&schema,&registry).map_err(error)?;let proof=check_context(&raw,&store,&registry,&mut b,&mut admission).map_err(error)?;let mut session=ReaderSession::new("empty-artifacts".into(),&checked,&registry,&mut b).map_err(error)?;
  let wait=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:3,final_input:true,context:&proof,state:&NdfValue::Unit},&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Await{continuation,..}=wait else{return Err("first wait".into())};
  let generated=admission.create(SourceId("generated".into()),0,"memory:generated".into(),b"a".to_vec(),&mut b).map_err(error)?;let map=Mapping{source:input.span(0,1).map_err(error)?,target:generated.span(0,1).map_err(error)?,kind:MappingKind::Exact};let mut reply=terminal("a",1,&mut b).map_err(error)?;
  let ProviderReply::Read(inner)=&mut reply else{return Err("read".into())};let ReadReply::Matched{sources,source_maps,..}=inner.as_mut() else{return Err("matched".into())};sources.push(generated.clone());source_maps.push(map.clone());
  let wait=session.resume(&continuation,reply,&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Await{continuation,..}=wait else{return Err("transform wait".into())};assert_eq!(continuation.current.source_maps,vec![map.clone()]);
  let reply=TransformReply{outcome:TransformOutcome::Complete{value:NdfValue::Text("B".into()),view:ViewBundle{elements:vec![],roots:vec![]},facts:vec![]},sources:vec![],source_maps:vec![],report:Report{usage:b.usage(),..Report::default()}};
  // Roots alone are not an empty artifact set. A malformed root must not short circuit.
  let mut invalid=reply.clone();let TransformOutcome::Complete{view,..}=&mut invalid.outcome else{return Err("complete".into())};view.roots.push(ViewRef(0));assert!(matches!(session.resume(&continuation,ProviderReply::Transform(Box::new(invalid)),&store,&mut b,&mut admission),Err(ReaderError::View(ViewError::Reference))));assert_eq!(b.poll(),Ok(()));
  // Additional sources still undergo conflict validation even when artifacts are empty.
  let conflict=SourceSnapshot::new(SourceId("generated".into()),0,"memory:conflict".into(),b"a".to_vec(),&mut b).map_err(error)?;let mut invalid=reply.clone();invalid.sources.push(conflict);assert!(matches!(session.resume(&continuation,ProviderReply::Transform(Box::new(invalid)),&store,&mut b,&mut admission),Err(ReaderError::Source(SourceError::IdentityConflict))));
  let reply=if portable{let dispatch=session.pending_transform().map_err(error)?;let empty=SourceStore::default();let mut codec=nepl3_wire::foundation::FoundationCodec::new(&registry,&empty,&mut admission).map_err(error)?;let value=nepl3_reader::portable::transform::reply_to_value(&reply,&dispatch,&mut codec,&mut b).map_err(error)?;let mut bad=value.clone();let NdfValue::Record(row)=&mut bad else{return Err("wire Record".into())};let NdfValue::Variant(outcome)=&mut row.fields[0] else{return Err("wire outcome".into())};outcome.fields[0]=NdfValue::Unit;
registry.validate(&reader_type("TransformReply"),&bad,&mut b).map_err(error)?;
let bytes=nepl3_wire::encode(&bad,&mut b).map_err(error)?;let bad=nepl3_wire::decode(&bytes,&mut b).map_err(error)?;
assert!(matches!(nepl3_reader::portable::transform::reply_from_value(&bad,&dispatch,&mut codec,&mut b),Err(nepl3_reader::portable::PortableError::Reader(ReaderError::Schema(_)))));
let bytes=nepl3_wire::encode(&value,&mut b).map_err(error)?;let value=nepl3_wire::decode(&bytes,&mut b).map_err(error)?;nepl3_reader::portable::transform::reply_from_value(&value,&dispatch,&mut codec,&mut b).map_err(error)?}else{reply};
  let wait=session.resume(&continuation,ProviderReply::Transform(Box::new(reply)),&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Await{continuation,..}=wait else{return Err("third wait".into())};assert_eq!(continuation.current.source_maps,vec![map.clone()]);assert_eq!(continuation.current.sources,vec![generated.clone()]);
  if cancel{let reply=terminal("c",3,&mut b).map_err(error)?;b.cancel();let stopped=session.resume(&continuation,reply,&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Stopped{reason,source_maps,sources,..}=stopped else{return Err("stopped".into())};assert_eq!(reason,StopReason::Cancelled);assert_eq!(source_maps,vec![map]);assert_eq!(sources,vec![generated]);assert_eq!(b.poll(),Err(StopReason::Cancelled));println!("portable={portable} cancelled after empty reply retains accepted map/source");continue;}
  let mut invalid=terminal("c",3,&mut b).map_err(error)?;let ProviderReply::Read(inner)=&mut invalid else{return Err("read".into())};let ReadReply::Matched{source_maps,..}=inner.as_mut() else{return Err("matched".into())};source_maps.push(Mapping{source:map.target.clone(),target:map.source.clone(),kind:MappingKind::Exact});let rejected=session.resume(&continuation,invalid,&store,&mut b,&mut admission);println!("portable={portable} cross-cycle rejected: {rejected:?}");assert!(matches!(rejected,Err(ReaderError::Origin(nepl3_core::origin::OriginError::Cycle))));assert_eq!(b.poll(),Ok(()));
  let final_reply=terminal("c",3,&mut b).map_err(error)?;let done=session.resume(&continuation,final_reply,&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Matched{value,source_maps,sources,..}=done else{return Err("final matched".into())};assert_eq!(value,NdfValue::List(vec![NdfValue::Text("a".into()),NdfValue::Text("B".into()),NdfValue::Text("c".into())]));assert_eq!(source_maps,vec![map]);assert_eq!(sources,vec![generated]);assert_eq!(b.current_depth(),0);
 }
 Ok(())
}
#[test]
fn empty_read_rejects_payload_state_range_usage_echo_and_source_conflicts_then_retries()->Result<(),String>{
 let (registry,schema)=registry().map_err(error)?;let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget()).map_err(error)?;let mut b=budget();let mut admission=SourceAdmission::default();let input=source("éa").map_err(error)?;let mut store=SourceStore::default();store.insert(input.clone()).map_err(error)?;let raw=context(&schema,&registry).map_err(error)?;let proof=check_context(&raw,&store,&registry,&mut b,&mut admission).map_err(error)?;let mut session=ReaderSession::new("empty-checks".into(),&checked,&registry,&mut b).map_err(error)?;
 let wait=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:3,final_input:true,context:&proof,state:&NdfValue::Unit},&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Await{continuation,..}=wait else{return Err("wait".into())};
 for case in 0..7{
  let mut reply=terminal("é",2,&mut b).map_err(error)?;
  let ProviderReply::Read(inner)=&mut reply else{return Err("read".into())};let ReadReply::Matched{value,new_state,end,report,..}=inner.as_mut() else{return Err("matched".into())};
  match case{0=>*value=NdfValue::Unit,1=>*new_state=NdfValue::Text("bad".into()),2=>*end=1,3=>*end=4,4=>report.usage.work=0,5=>report.usage.work=u64::MAX,6=>report.trace_overflow=Some(TraceOverflow{dropped:1}),_=>return Err("case".into())};
  let e=session.resume(&continuation,reply,&store,&mut b,&mut admission).expect_err("invalid reply");println!("empty Read negative {case}: {e:?}");match case{0|1=>assert!(matches!(e,ReaderError::Schema(_))),2=>assert!(matches!(e,ReaderError::Source(SourceError::ScalarBoundary))),_=>assert!(matches!(e,ReaderError::ProviderContract))};assert_eq!(b.poll(),Ok(()));
 }
 let mut echo=continuation.clone();echo.current.sources.push(input.clone());let reply=terminal("é",2,&mut b).map_err(error)?;assert!(matches!(session.resume(&echo,reply,&store,&mut b,&mut admission),Err(ReaderError::Continuation)));
 let empty=SourceStore::default();let reply=terminal("é",2,&mut b).map_err(error)?;assert!(matches!(session.resume(&continuation,reply,&empty,&mut b,&mut admission),Err(ReaderError::Source(SourceError::MissingSnapshot))));
 let conflict=SourceSnapshot::new(input.identity().source.clone(),0,"memory:changed-uri".into(),input.text().as_bytes().to_vec(),&mut b).map_err(error)?;let mut wrong=SourceStore::default();wrong.insert(conflict).map_err(error)?;let reply=terminal("é",2,&mut b).map_err(error)?;assert!(matches!(session.resume(&continuation,reply,&wrong,&mut b,&mut admission),Err(ReaderError::Source(SourceError::IdentityConflict))));
 let reply=terminal("é",2,&mut b).map_err(error)?;assert!(matches!(session.resume(&continuation,reply,&store,&mut b,&mut admission).map_err(error)?,ReadReply::Matched{end:2,..}));assert_eq!(b.current_depth(),0);Ok(())
}

#[test]
fn no_match_need_more_empty_contract_and_original_retry()->Result<(),String>{
 let (registry,schema)=registry().map_err(error)?;
 for need_more in [false,true]{
  let p=provider_plan(&schema);let checked=p.check(&registry,&mut budget()).map_err(error)?;let mut b=budget();let mut admission=SourceAdmission::default();let input=source("é").map_err(error)?;let mut store=SourceStore::default();store.insert(input.clone()).map_err(error)?;let raw=context(&schema,&registry).map_err(error)?;let proof=check_context(&raw,&store,&registry,&mut b,&mut admission).map_err(error)?;let mut session=ReaderSession::new("empty-tags".into(),&checked,&registry,&mut b).map_err(error)?;
  let wait=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:2,final_input:false,context:&proof,state:&NdfValue::Unit},&store,&mut b,&mut admission).map_err(error)?;let ReadReply::Await{continuation,..}=wait else{return Err("wait".into())};
  for case in 0..4{
   let mut report=Report{usage:b.usage(),..Report::default()};if case==0{report.usage.work=0;}if case==1{report.trace_overflow=Some(TraceOverflow{dropped:1});}
   let expected=if case==2{vec![Expectation::ScalarClass(CharClass::Range{lo:'z',hi:'a'})]}else{vec![]};let sources=if case==3{vec![input.clone()]}else{vec![]};
   let reply=if need_more{ReadReply::NeedMore{expected,sources,source_maps:vec![],report}}else{ReadReply::NoMatch{expected,furthest:0,sources,source_maps:vec![],report}};
   assert!(matches!(session.resume(&continuation,ProviderReply::Read(Box::new(reply)),&store,&mut b,&mut admission),Err(ReaderError::ProviderContract)));assert_eq!(b.poll(),Ok(()));
  }
  let report=Report{usage:b.usage(),..Report::default()};let reply=if need_more{ReadReply::NeedMore{expected:vec![],sources:vec![],source_maps:vec![],report}}else{ReadReply::NoMatch{expected:vec![],furthest:0,sources:vec![],source_maps:vec![],report}};
  let out=session.resume(&continuation,ProviderReply::Read(Box::new(reply)),&store,&mut b,&mut admission).map_err(error)?;assert!(if need_more{matches!(out,ReadReply::NeedMore{..})}else{matches!(out,ReadReply::NoMatch{..})});assert_eq!(b.current_depth(),0);
 }Ok(())
}
