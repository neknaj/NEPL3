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
