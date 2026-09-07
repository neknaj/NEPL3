use super::*;
use nepl3_reader::tokenizer::*;
struct BoundaryHost { stop: bool, calls: usize }
impl TokenizationHost for BoundaryHost {
 fn provider(&mut self,_:&ProviderCall,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError>{self.calls+=1;Err(ReaderError::Stopped(StopReason::Cancelled))}
 fn reservation(&mut self,_:&ReservationRequest,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{self.calls+=1;if self.stop{return Err(ReaderError::Stopped(StopReason::Cancelled))}Ok(Some(SourceReservation{source_id:SourceId("new".into()),revision:0,uri:String::new()}))}
}
fn scenario(builtin:bool,stop:bool)->Result<(),ReaderError>{
 let (r,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&r,&mut budget())?;let input=source(if builtin{"\"a\""}else{"a"})?;let mut store=SourceStore::default();store.insert(input.clone())?;let raw=context(&schema,&r)?;
 let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![TakeRule{reader:TokenReader::Rule("entry".into()),kind:KindRef{schema:schema.clone(),local_kind:0}}]}];
 let mut b=budget();let mut a=SourceAdmission::default();let proof=check_context(&raw,&store,&r,&mut b,&mut a)?;let mut session=TokenizationSession::new("independent".into(),&modes,&checked,&r,&mut b)?;
 let scope=TokenizationScope{operation_id:"original-operation".into(),profile_digest:Digest([9;32]),snapshot:input.reference()};let accepted=AcceptedTokenizationReport::empty(scope.clone(),&mut b)?;
 let mut host=BoundaryHost{stop,calls:0};let target=if builtin{TokenTarget::Builtin{reader:nepl3_reader::builtin::BuiltinReader::Text,token_kind:KindRef{schema:schema.clone(),local_kind:0}}}else{TokenTarget::Mode};
 let result=session.read_accepted_with_host(ScopedTokenizationRequest{scope:&scope,target,input:TokenizationRequest{snapshot:&input,start:0,limit:input.text().len()as u64,final_input:true,context:&proof,state:&NdfValue::Unit}},&store,&mut b,&mut a,accepted,&mut host);
 assert_eq!(host.calls,1);
 match result{
  Err(error)=>{println!("builtin={builtin} stop={stop} returned Err({error:?}) budget={:?}",b.poll());Err(error)},
  Ok(result)=>{let host_error=result.host_error;let reply=result.reply.into_raw();println!("builtin={builtin} stop={stop} host_error={host_error:?} budget={:?} outcome={:?}",b.poll(),reply.outcome);
   if stop{assert!(matches!(reply.outcome,TokenizationOutcome::Stopped{reason:StopReason::Cancelled}));assert_eq!(b.poll(),Err(StopReason::Cancelled));}
   else{assert!(host_error.is_some());let TokenizationOutcome::Reserve{continuation,..}=reply.outcome else{return Err(ReaderError::NoPending)};let good=SourceReservation{source_id:SourceId("new".into()),revision:0,uri:"memory:new".into()};let retry=session.reserve(&continuation,&good,&store,&mut b,&mut a)?;assert!(matches!(retry.outcome,TokenizationOutcome::Token(_)));}
   Ok(())}
 }
}
#[test]fn invalid_reservation_keeps_owned_retry()->Result<(),ReaderError>{scenario(true,false)}
#[test]fn explicit_provider_stop_is_not_an_await()->Result<(),ReaderError>{scenario(false,true)}
#[test]fn explicit_reservation_stop_is_not_a_reserve()->Result<(),ReaderError>{scenario(true,true)}

struct ReservationMode(bool);
impl TokenizationHost for ReservationMode {
 fn provider(&mut self,_:&ProviderCall,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError>{Ok(None)}
 fn reservation(&mut self,_:&ReservationRequest,b:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{assert!(b.current_depth()>=8);Ok(self.0.then(||SourceReservation{source_id:SourceId("new".into()),revision:0,uri:String::new()}))}
}
#[test]
fn invalid_supplied_reservation_matches_owned_error_contract()->Result<(),ReaderError>{
 let (r,schema)=registry()?;let plan=provider_plan(&schema);let checked=plan.check(&r,&mut budget())?;let input=source("\"a\"")?;let mut store=SourceStore::default();store.insert(input.clone())?;let raw=context(&schema,&r)?;
 let modes=vec![ReaderMode{name:"test".into(),skip:vec![],take:vec![]}];
 for native in [false,true]{let mut b=budget();let mut a=SourceAdmission::default();let proof=check_context(&raw,&store,&r,&mut b,&mut a)?;let mut session=TokenizationSession::new("reservation-contract".into(),&modes,&checked,&r,&mut b)?;let scope=TokenizationScope{operation_id:"reservation-operation".into(),profile_digest:Digest([6;32]),snapshot:input.reference()};let accepted=AcceptedTokenizationReport::empty(scope.clone(),&mut b)?;let mut host=ReservationMode(native);
 let result=b.with_depth_at_least(7,|b|session.read_accepted_with_host(ScopedTokenizationRequest{scope:&scope,target:TokenTarget::Builtin{reader:nepl3_reader::builtin::BuiltinReader::Text,token_kind:KindRef{schema:schema.clone(),local_kind:0}},input:TokenizationRequest{snapshot:&input,start:0,limit:3,final_input:true,context:&proof,state:&NdfValue::Unit}},&store,b,&mut a,accepted,&mut host));
 assert_eq!(b.current_depth(),0);
 if native{assert!(matches!(result,Err(ReaderError::Source(SourceError::Locator))));}
 else{let result=result?;assert!(result.host_error.is_none());let TokenizationOutcome::Reserve{continuation,..}=result.reply.into_raw().outcome else{return Err(ReaderError::NoPending)};let bad=SourceReservation{source_id:SourceId("new".into()),revision:0,uri:String::new()};assert_eq!(session.reserve(&continuation,&bad,&store,&mut b,&mut a),Err(ReaderError::Source(SourceError::Locator)));let good=SourceReservation{uri:"memory:new".into(),..bad};assert_eq!(session.reserve(&continuation,&good,&store,&mut b,&mut a),Err(ReaderError::NoPending));}
 println!("invalid reservation native={native} returns Source Locator; owned retry NoPending; caller depth restored");
 }
 Ok(())
}
