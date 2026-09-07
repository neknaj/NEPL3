use super::*;
use nepl3_reader::tokenizer::*;
struct ReviewHost<'a> { mode:u8,calls:usize,registry:&'a SchemaRegistry }
impl TokenizationHost for ReviewHost<'_> {
 fn provider(&mut self,call:&ProviderCall,b:&mut Budget,admission:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError>{
  self.calls+=1;
  let (request,depth)=match call { ProviderCall::Read{request,depth_base,..}=>{assert_eq!(self.calls,1);(request,*depth_base)}, ProviderCall::Dependent{request,depth_base,..}=>{assert_eq!(request.first,NdfValue::Text("a".into()));assert_eq!(self.calls,2);(&request.request,*depth_base)}, _=>return Err(ReaderError::ProviderContract)};
  assert_eq!(b.current_depth(),depth);
  if self.calls==2 {match self.mode{1=>return Ok(None),2=>return Err(ReaderError::Context),4=>return Err(ReaderError::Stopped(StopReason::Cancelled)),5=>{b.cancel();return Ok(None);},_=>{}}}
  if self.mode==6 { review_wire_request(request,self.registry,b,admission); }
  let mut reply=terminal(if self.calls==1{"a"}else{"b"},request.start+1,b)?;
  if self.calls==2 && self.mode==3 {if let ProviderReply::Read(v)=&mut reply {if let ReadReply::Matched{value,..}=v.as_mut(){*value=NdfValue::Unit;}}}
  Ok(Some(reply))
 }
 fn reservation(&mut self,_:&ReservationRequest,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{Err(ReaderError::Context)}
}
#[test] fn review_read_then_dependent_native_retry_scope_and_stops()->Result<(),ReaderError>{
 let (r,schema)=registry()?;let read=signature(&schema,ProviderKind::Read);let dep=signature(&schema,ProviderKind::Dependent);let mut p=plan(&schema,vec![ReaderExpr::Call(read.operation.clone()),ReaderExpr::Then{first:ReaderId(0),provider:dep.operation.clone()}],1,TypeDescriptor::Text);p.providers=vec![read,dep];let checked=p.check(&r,&mut budget())?;
 let input=source("ab")?;let mut store=SourceStore::default();store.insert(input.clone())?;let raw=context(&schema,&r)?;let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![TakeRule{reader:TokenReader::Rule("entry".into()),kind:KindRef{schema:schema.clone(),local_kind:0}}]}];let mut results=vec![];
 for mode in 0..7 {let mut b=budget();let mut a=SourceAdmission::default();let proof=check_context(&raw,&store,&r,&mut b,&mut a)?;let mut session=TokenizationSession::new("dependent-native".into(),&modes,&checked,&r,&mut b)?;let scope=TokenizationScope{operation_id:"dependent-scope".into(),profile_digest:Digest([7;32]),snapshot:input.reference()};let accepted=AcceptedTokenizationReport::empty(scope.clone(),&mut b)?;let mut host=ReviewHost{mode,calls:0,registry:&r};let result=session.read_accepted_with_host(ScopedTokenizationRequest{scope:&scope,target:TokenTarget::Mode,input:TokenizationRequest{snapshot:&input,start:0,limit:2,final_input:true,context:&proof,state:&NdfValue::Unit}},&store,&mut b,&mut a,accepted,&mut host)?;assert_eq!(host.calls,2);assert_eq!(b.current_depth(),0);assert_eq!(result.host_error.is_some(),matches!(mode,2|3|4));let mut result=result.reply.into_raw();
  if matches!(mode,4|5) {assert!(matches!(result.outcome,TokenizationOutcome::Stopped{reason:StopReason::Cancelled}));assert_eq!(b.poll(),Err(StopReason::Cancelled));continue;}
  if matches!(mode,1|2|3) {let TokenizationOutcome::Await{continuation,..}=&result.outcome else{return Err(ReaderError::NoPending)};let mut forged=continuation.as_ref().clone();forged.scope.operation_id="other".into();assert_eq!(session.resume(&forged,terminal("b",2,&mut b)?,&store,&mut b,&mut a),Err(ReaderError::Continuation));let mut forged=continuation.as_ref().clone();forged.request.limit=1;assert_eq!(session.resume(&forged,terminal("b",2,&mut b)?,&store,&mut b,&mut a),Err(ReaderError::Continuation));result=session.resume(continuation,terminal("b",2,&mut b)?,&store,&mut b,&mut a)?;}
  assert_eq!(result.cursor,2);assert!(matches!(&result.outcome,TokenizationOutcome::Token(t) if t.payload==NdfValue::Text("b".into())));results.push((result.outcome,result.sources,result.source_maps));
 }
 for x in &results[1..]{assert_eq!(x,&results[0]);}Ok(())
}

fn review_wire_request(request:&OwnedReadRequest,registry:&SchemaRegistry,b:&mut Budget,a:&mut SourceAdmission){
 use nepl3_reader::portable::{request_to_value,request_sources,request_from_value};use nepl3_wire::foundation::FoundationCodec;
 let schema=registry.selected("nepl3.reader",1).expect("reader schema");let mut store=SourceStore::default();for x in &request.sources{store.insert_ref_with_budget(x,b).expect("request source");}
 let value=request_to_value(request,schema,&mut FoundationCodec::new(registry,&store,a).expect("codec"),&store,registry,b).expect("encode request value");
 let bytes=nepl3_wire::encode_checked(&value,&reader_type("ReadRequest"),registry,b).expect("real CBOR encode");let value=nepl3_wire::decode_checked(&bytes,&reader_type("ReadRequest"),registry,b).expect("real CBOR decode");let empty=SourceStore::default();let mut first=SourceStore::default();
 for x in request_sources(value.value(),schema,&mut FoundationCodec::new(registry,&empty,a).expect("fresh codec"),registry,b).expect("fresh sources"){first.insert_with_budget(x,b).expect("declare");}
 let restored=request_from_value(value.value(),schema,&mut FoundationCodec::new(registry,&first,a).expect("closed codec"),&first,registry,b).expect("restore request");assert_eq!(restored,*request);let source=first.resolve(&restored.snapshot).expect("source");assert_eq!(source.text(),"ab");
}
