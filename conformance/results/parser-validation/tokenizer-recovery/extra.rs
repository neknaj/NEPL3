struct ProbeHost {
    schema: SchemaRef,
    input: SourceSnapshot,
    extra: Option<SourceSnapshot>,
    mode: u8,
    calls: usize,
}
impl TokenizationHost for ProbeHost {
    fn provider(&mut self, call: &ProviderCall, b: &mut Budget, a: &mut SourceAdmission) -> Result<Option<ProviderReply>, ReaderError> {
        self.calls += 1;
        let ProviderCall::Read {request,..}=call else {return Err(ReaderError::ProviderContract)};
        if request.start != 5 {
            return Ok(Some(ProviderReply::Read(Box::new(ReadReply::NoMatch {
                expected:vec![],furthest:request.start,sources:vec![],source_maps:vec![],report:Report{usage:b.usage(),..Report::default()}
            }))));
        }
        if self.mode == 4 { return Ok(None); }
        let extra=match &self.extra {
            Some(s)=>s.clone_with_budget(b)?,
            None=>a.create(SourceId("extra".into()),0,"memory:extra".into(),b" ".to_vec(),b)?,
        };
        self.extra=Some(extra.clone());
        let mut reply=annotated_terminal(&self.schema,extra.span(0,1)?,6,b)?;
        let ProviderReply::Read(r)=&mut reply else {return Err(ReaderError::ProviderContract)};
        let ReadReply::Matched {sources,source_maps,report,..}=r.as_mut() else {return Err(ReaderError::ProviderContract)};
        source_maps.push(Mapping{source:extra.span(0,1)?,target:self.input.span(5,6)?,kind:MappingKind::Transformed});
        sources.push(extra);report.usage=b.usage();
        Ok(Some(reply))
    }
    fn reservation(&mut self, request:&ReservationRequest,b:&mut Budget,_:&mut SourceAdmission)->Result<Option<SourceReservation>,ReaderError>{
        if request.start==6 {
            if self.mode==2 {b.cancel();return Ok(None)}
            if self.mode==3 {return Ok(None)}
            if self.mode==6 {b.charge(Resource::Work,u64::MAX)?;}
        }
        Ok(Some(SourceReservation{source_id:SourceId(if self.mode==1 && request.start==6 {"decoded-next"} else {"decoded"}.into()),revision:0,uri:if self.mode==1 && request.start==6 {"memory:decoded-next"} else {"memory:decoded"}.into()}))
    }
}

#[test]
fn actual_prefix_hard_error_then_retry_and_live_stop() -> Result<(),ReaderError> {
    let (r,schema)=registry()?;let p=provider_plan(&schema);let checked=p.check(&r,&mut budget())?;
    let input=source("\"a\\n\" \"b\\n\"")?;let mut store=SourceStore::default();store.insert(input.clone())?;
    let raw=context(&schema,&r)?;
    let modes=vec![ReaderMode{name:"test".into(),skip:vec![SkipRule{reader:TokenReader::Rule("entry".into())}],take:vec![]}];
    for mode in 0..7 {
        let mut b=budget();let mut a=SourceAdmission::default();let context=check_context(&raw,&store,&r,&mut b,&mut a)?;
        let mut session=TokenizationSession::new("recover-probe".into(),&modes,&checked,&r,&mut b)?;
        let scope=TokenizationScope{operation_id:"operation".into(),profile_digest:Digest([3;32]),snapshot:input.reference()};
        let state=NdfValue::Unit;
        let request=|start|ScopedTokenizationRequest{scope:&scope,target:TokenTarget::Builtin{reader:BuiltinReader::Text,token_kind:KindRef{schema:schema.clone(),local_kind:0}},input:TokenizationRequest{snapshot:&input,start,limit:11,final_input:true,context:&context,state:&state}};
        let accepted=AcceptedTokenizationReport::empty(scope.clone(),&mut b)?;
        let mut host=ProbeHost{schema:schema.clone(),input:input.clone(),extra:None,mode:0,calls:0};
        let first=session.read_accepted_with_host_recover(request(0),&store,&mut b,&mut a,accepted,&mut host).map_err(AcceptedTokenizationFailure::into_error)?;
        assert!(first.host_error.is_none());assert_eq!(first.reply.cursor,5);
        let mut original=first.reply.accepted;assert_eq!(original.sources().len(),1);assert_eq!(original.source_maps().len(),2);
        let old_sources=original.sources().to_vec();let old_maps=original.source_maps().to_vec();let old_usage=original.report().usage;
        if mode==0 {
            for foreign in [false,true] {
                let mut limits=b.limits();if foreign {limits.work+=1;}
                let original_budget=std::mem::replace(&mut b,Budget::new(limits));
                if foreign {let mut observed=old_usage;observed.work+=99;b.record_observed_usage(observed)?;}
                let failure=session.read_with_accepted_recover(request(5),&store,&mut b,&mut a,original);
                let Err(AcceptedTokenizationFailure::Recoverable{error,accepted})=failure else {panic!("invalid Budget must recover old prefix")};
                assert_eq!(error,ReaderError::Continuation);assert_eq!(accepted.report().usage,old_usage);
                assert_eq!(accepted.sources(),old_sources);assert_eq!(accepted.source_maps(),old_maps);
                original=accepted;b=original_budget;
            }
        }
        host.mode=mode;
        let second=if mode==5 {
            session.read_with_accepted_recover(request(5),&store,&mut b,&mut a,original).map(|reply|TokenizationHostReply{reply,host_error:None})
        } else {session.read_accepted_with_host_recover(request(5),&store,&mut b,&mut a,original,&mut host)};
        if mode==0 {
            let Err(AcceptedTokenizationFailure::Recoverable{error,accepted})=second else {panic!("expected actual hard error")};
            println!("actual hard error {error:?}; live host source {}",host.extra.is_some());
            assert_eq!(error,ReaderError::Source(SourceError::IdentityConflict));assert!(host.extra.is_some());
            assert_eq!(accepted.sources(),old_sources);assert_eq!(accepted.source_maps(),old_maps);
            assert!(accepted.report().diagnostics.is_empty());assert!(accepted.report().events.is_empty());
            assert_eq!(accepted.scope(),&scope);assert_eq!(accepted.report().usage,b.usage());assert!(b.usage().work>old_usage.work);
            assert!(b.usage().source_bytes>old_usage.source_bytes);
            host.mode=1;
            let final_reply=session.read_accepted_with_host_recover(request(5),&store,&mut b,&mut a,accepted,&mut host).map_err(AcceptedTokenizationFailure::into_error)?;
            assert!(final_reply.host_error.is_none());let raw=final_reply.reply.into_raw();
            assert!(matches!(&raw.outcome,TokenizationOutcome::Token(t) if t.payload==NdfValue::Text("b\n".into())));
            assert_eq!(raw.sources.len(),3);assert_eq!(raw.source_maps.len(),5);assert_eq!(raw.report.diagnostics.len(),1);assert_eq!(raw.report.events.len(),1);
            println!("hard error retry kept all live vectors: {:?}",b.usage());
        } else {
            let mut result=second.map_err(AcceptedTokenizationFailure::into_error)?;
            if mode==6 {assert_eq!(result.host_error,Some(ReaderError::Stopped(StopReason::WorkLimit)));}else{assert!(result.host_error.is_none());}
            if matches!(mode,4|5) {
                assert!(matches!(result.reply.outcome,TokenizationOutcome::Await{..}));
                assert_eq!(result.reply.accepted.sources(),old_sources);assert_eq!(result.reply.accepted.source_maps(),old_maps);
                let attempt=session.read_with_accepted_recover(request(5),&store,&mut b,&mut a,result.reply.accepted);
                let Err(AcceptedTokenizationFailure::Recoverable{error,accepted})=attempt else {panic!("Busy must return original proof")};
                assert_eq!(error,ReaderError::Busy);result.reply.accepted=accepted;
            }
            let mut raw=result.reply.into_raw();
            if matches!(mode,4|5) {
                host.mode=1;
                for _ in 0..4 {
                    match &raw.outcome {
                        TokenizationOutcome::Await{call,continuation}=>{
                            let reply=host.provider(call,&mut b,&mut a)?.ok_or(ReaderError::ProviderContract)?;
                            raw=session.resume(continuation,reply,&store,&mut b,&mut a)?;
                        }
                        TokenizationOutcome::Reserve{request,continuation}=>{
                            let reservation=host.reservation(request,&mut b,&mut a)?.ok_or(ReaderError::ProviderContract)?;
                            raw=session.reserve(continuation,&reservation,&store,&mut b,&mut a)?;
                        }
                        _=>break,
                    }
                }
                assert!(matches!(raw.outcome,TokenizationOutcome::Token(_)));assert_eq!(raw.sources.len(),3);assert_eq!(raw.source_maps.len(),5);
            }
            if mode==1 {assert!(matches!(raw.outcome,TokenizationOutcome::Token(_)));assert_eq!(raw.sources.len(),3);assert_eq!(raw.source_maps.len(),5)}
            if mode==2 {assert!(matches!(raw.outcome,TokenizationOutcome::Stopped{reason:StopReason::Cancelled}));assert_eq!(b.poll(),Err(StopReason::Cancelled));}
            if mode==6 {assert!(matches!(raw.outcome,TokenizationOutcome::Stopped{reason:StopReason::WorkLimit}));assert_eq!(b.poll(),Err(StopReason::WorkLimit));}
            if mode==3 {
                let TokenizationOutcome::Reserve{continuation,..}=&raw.outcome else {panic!("expected owned Reserve")};
                assert_eq!(raw.sources.len(),2);assert_eq!(raw.source_maps.len(),3);
                let reservation=SourceReservation{source_id:SourceId("decoded-next".into()),revision:0,uri:"memory:decoded-next".into()};
                raw=session.reserve(continuation,&reservation,&store,&mut b,&mut a)?;
                assert!(matches!(raw.outcome,TokenizationOutcome::Token(_)));assert_eq!(raw.sources.len(),3);assert_eq!(raw.source_maps.len(),5);
            }
            assert_eq!(raw.report.diagnostics.len(),1);assert_eq!(raw.report.events.len(),1);
            if matches!(mode,2|6) {assert_eq!(raw.sources.len(),2);assert_eq!(raw.source_maps.len(),3);}
            println!("live mode {mode} retained report and maps: {:?}",b.usage());
        }
    }
    Ok(())
}
