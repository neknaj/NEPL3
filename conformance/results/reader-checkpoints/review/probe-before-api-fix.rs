include!("review_helpers.rs");
use nepl3_reader::tokenizer::*;
use nepl3_core::origin::{Mapping, MappingKind};

struct ReviewHost { schema: SchemaRef, input: SourceSnapshot, calls: usize, transport_only: bool }
impl ReviewHost {
    fn reply(&mut self, call: &ProviderCall, b: &mut Budget, a: &mut SourceAdmission) -> Result<ProviderReply, ReaderError> {
        self.calls += 1;
        match call {
            ProviderCall::Read { request, .. } => {
                if request.start == request.limit {
                    return Ok(ProviderReply::Read(Box::new(ReadReply::NoMatch {
                        expected: vec![], furthest: request.start, sources: vec![], source_maps: vec![],
                        report: Report { usage: b.usage(), ..Report::default() },
                    })));
                }
                let NdfValue::U64(state) = request.state else { return Err(ReaderError::Context) };
                let start=request.start; let end=start+1;
                let added=a.create(SourceId(format!("generated-{}",self.calls)),0,format!("memory:generated-{}",self.calls),b"g".to_vec(),b)?;
                let generated=added.span(0,1)?;
                let span=self.input.span(start,end)?;
                let ProviderReply::Read(mut r)=annotated_terminal(&self.schema,generated.clone(),end,b)? else { return Err(ReaderError::Context) };
                let ReadReply::Matched { new_state, view, facts, sources, source_maps, .. }=r.as_mut() else { return Err(ReaderError::Context) };
                *new_state=NdfValue::U64(state+1);
                view.elements.push(ViewElement { kind: KindRef { schema:self.schema.clone(),local_kind:0 }, span:span.clone(), fields:vec![] });
                view.roots.push(ViewRef(0));
                facts.push(ReaderFact::Capture { name:"captured".into(),span:generated.clone() });
                sources.push(added);
                source_maps.push(Mapping { source:span,target:generated,kind:MappingKind::Transformed });
                Ok(ProviderReply::Read(r))
            }
            ProviderCall::Transform { .. } => Ok(ProviderReply::Transform(Box::new(TransformReply {
                outcome: TransformOutcome::Complete { value:NdfValue::Text("mapped".into()),view:ViewBundle { elements:vec![],roots:vec![] },facts:vec![] },
                sources:vec![],source_maps:vec![],report:Report { usage:b.usage(),..Report::default() },
            }))),
            _=>Err(ReaderError::Context),
        }
    }
}
impl TokenizationHost for ReviewHost {
    fn provider(&mut self, call:&ProviderCall,b:&mut Budget,a:&mut SourceAdmission)->Result<Option<ProviderReply>,ReaderError> {
        if self.transport_only { return Ok(None) };
        self.reply(call,b,a).map(Some)
    }
    fn reservation(&mut self,_:&ReservationRequest,_:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError> { Ok(None) }
}

fn transaction_plan(schema:&SchemaRef,mode:usize)->ReaderPlan {
    let mut read=signature(schema,ProviderKind::Read); read.state_type=TypeDescriptor::U64;
    let mut map=signature(schema,ProviderKind::Transform); map.state_type=TypeDescriptor::U64;
    let k=KindRef { schema:schema.clone(), local_kind:0 };
    let mut expressions=vec![ReaderExpr::Call(read.operation.clone()),ReaderExpr::Literal("z".into()),ReaderExpr::Literal("b".into()),ReaderExpr::Node { kind:k.clone(),body:ReaderId(0) },ReaderExpr::Seq(vec![ReaderId(3),ReaderId(1)]),ReaderExpr::Discard(ReaderId(4))];
    let root=match mode {
        0=>ReaderExpr::Choice(vec![ReaderId(5),ReaderId(2)]),
        1=>ReaderExpr::Look(ReaderId(3)),
        2=>ReaderExpr::Not(ReaderId(4)),
        3=>ReaderExpr::Repeat { body:ReaderId(3),min:2,max:3 },
        4=>ReaderExpr::Map { provider:map.operation.clone(),body:ReaderId(3) },
        5=>ReaderExpr::Decode { provider:map.operation.clone(),body:ReaderId(3) },
        _=>ReaderExpr::Node { kind:k,body:ReaderId(3) },
    };
    expressions.push(root);
    let wrapper=if mode==3 { expressions.push(ReaderExpr::Optional(ReaderId(6)));7 }
        else if mode>=4 { expressions.push(ReaderExpr::Look(ReaderId(6)));7 }
        else {6};
    let mut seq=vec![ReaderId(0),ReaderId(wrapper)];
    if mode!=0 {seq.push(ReaderId(2));}
    let end=expressions.len() as u64;expressions.push(ReaderExpr::Seq(seq));
    let mut p=plan(schema,expressions,end,TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)));
    p.state_type=TypeDescriptor::U64;p.providers=vec![read,map];p
}

#[test]
fn seven_collectors_state_and_nested_root_rollback_match_owned_transport()->Result<(),ReaderError> {
    let (r,schema)=registry()?;
    for mode in 0..7 {
        let p=transaction_plan(&schema,mode);let checked=p.check(&r,&mut budget())?;
        let input=source("ab")?;let mut store=SourceStore::default();store.insert(input.clone())?;
        let raw=context(&schema,&r)?;
        let modes=vec![ReaderMode {name:"test".into(),skip:vec![],take:vec![TakeRule {reader:TokenReader::Rule("entry".into()),kind:KindRef {schema:schema.clone(),local_kind:0}}]}];
        let mut previous=None;
        for owned in [false,true] {
            let mut b=budget();let mut a=SourceAdmission::default();let ctx=check_context(&raw,&store,&r,&mut b,&mut a)?;
            let mut session=TokenizationSession::new("review".into(),&modes,&checked,&r,&mut b)?;
            let scope=TokenizationScope {operation_id:"operation".into(),profile_digest:Digest([3;32]),snapshot:input.reference()};
            let accepted=AcceptedTokenizationReport::empty(scope.clone(),&mut b)?;
            let mut host=ReviewHost { schema:schema.clone(),input:input.clone(),calls:0,transport_only:owned };
            let result=session.read_accepted_with_host(ScopedTokenizationRequest {scope:&scope,target:TokenTarget::Mode,input:TokenizationRequest {snapshot:&input,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::U64(0)}},&store,&mut b,&mut a,accepted,&mut host)?;
            assert!(result.host_error.is_none());let mut result=result.reply.into_raw();
            loop {
                let TokenizationOutcome::Await {call,continuation}= &result.outcome else {break};
                let reply=host.reply(call,&mut b,&mut a)?;
                result=session.resume(continuation,reply,&store,&mut b,&mut a)?;
            }
            let TokenizationOutcome::Token(token)=&result.outcome else {return Err(ReaderError::Context)};
            assert_eq!(result.cursor,2,"mode={mode} owned={owned}");
            assert_eq!(result.state,NdfValue::U64(1));
            assert_eq!(token.views.elements.len(),1);assert_eq!(token.views.roots,vec![ViewRef(0)]);
            assert_eq!((token.views.elements[0].span.start(),token.views.elements[0].span.end()),(0,1));
            assert_eq!(result.sources.len(),1);assert_eq!(result.sources[0].id().0,"generated-1");assert_eq!(result.source_maps.len(),1);
            assert_eq!(result.report.diagnostics.len(),1);assert_eq!(result.report.events.len(),1);
            assert!(b.usage().diagnostics>1);assert!(b.usage().events>1);
            assert_eq!(result.fact_batches.len(),1);assert_eq!(result.fact_batches[0].facts.len(),1);
            // Transport materialization may cost more; semantic data must remain exact.
            result.report.usage=Usage::default();
            if let Some(prior)=&previous {assert_eq!(prior,&result,"mode={mode}")} else {previous=Some(result)};
            println!("mode={mode} owned={owned} calls={} work={} allocation={}",host.calls,b.usage().work,b.usage().allocation_units);
        }
    }
    Ok(())
}
