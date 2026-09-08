use alloc::{vec,vec::Vec,format,string::String,boxed::Box};
use std::println;
use nepl3_core::{budget::*,diagnostic::*,schema::*,source::*,syntax::*,value::*,view::*};
use crate::{model::*,plan::*,runtime::*};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10_000_000,
        work: 1_000_000_000,
        depth: 512,
        nodes: 1_000_000,
        allocation_units: 1_000_000_000,
        output_bytes: 10_000_000,
        diagnostics: 1000,
        events: 1000,
    })
}
fn reader_type(name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: "nepl3.reader".into(),
        revision: 1,
        name: name.into(),
    })
}
fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let mut b = budget();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut b)?;
    let reference = foundation.reference(&mut b)?;
    registry.register(reference, foundation, &mut b)?;
    let reader = nepl3_reader::schema::descriptor(&mut b)?;
    let reference = reader.reference(&mut b)?;
    registry.register(reference, reader, &mut b)?;
    let descriptor = SchemaDescriptor {
        package: "test".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            constraints: vec![],
            shape: TypeShape::Record { fields: vec![] },
        }],
        operations: vec![
            OperationDescriptor {
                name: "read".into(),
                input: reader_type("ReadRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "transform".into(),
                input: reader_type("TransformRequest"),
                output: reader_type("TransformReply"),
                pure: true,
            },
            OperationDescriptor {
                name: "dependent".into(),
                input: reader_type("DependentRequest"),
                output: reader_type("ReadReply"),
                pure: true,
            },
        ],
    };
    let schema = descriptor.reference(&mut b)?;
    registry.register(schema.clone(), descriptor, &mut b)?;
    registry.finalize(&mut b)?;
    Ok((registry, schema))
}
fn plan(
    schema: &SchemaRef,
    expressions: Vec<ReaderExpr>,
    root: u64,
    output: TypeDescriptor,
) -> ReaderPlan {
    ReaderPlan {
        schema: schema.clone(),
        state_type: TypeDescriptor::Unit,
        expressions,
        rules: vec![ReaderRule {
            name: "entry".into(),
            root: ReaderId(root),
            output,
        }],
        providers: vec![],
    }
}
fn context(schema: &SchemaRef, registry: &SchemaRegistry) -> Result<ReaderContext, ReaderError> {
    let mut context = ReaderContext {
        schema: schema.clone(),
        category: "Token".into(),
        mode: "test".into(),
        environment: EnvironmentEntry {
            id: 0,
            digest: Digest([0; 32]),
            value: Environment {
                bindings: vec![],
                resources: vec![],
            },
        },
        origins: vec![],
    };
    context.environment.digest = nepl3_wire::environment::environment_digest(
        &context.environment.value,
        registry
            .selected("nepl3.foundation", 1)
            .ok_or(SchemaError::UnknownSchema)?,
        registry,
        &mut budget(),
    )
    .map_err(|_| ReaderError::Context)?;
    Ok(context)
}
fn check_context<'a>(
    raw: &'a ReaderContext,
    store: &'a SourceStore,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<nepl3_reader::context::CheckedReaderContext<'a>, ReaderError> {
    let mut codec = nepl3_wire::foundation::FoundationCodec::new(registry, store, admission)
        .map_err(|_| ReaderError::Context)?;
    raw.check(&mut codec, store, registry, budget)
        .map_err(|_| ReaderError::Context)
}
fn source(text: &str) -> Result<SourceSnapshot, SourceError> {
    SourceSnapshot::new(
        SourceId("test".into()),
        0,
        "memory:test".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
}
fn signature(schema: &SchemaRef, kind: ProviderKind) -> ProviderSignature {
    ProviderSignature {
        operation: OperationRef {
            schema: schema.clone(),
            name: match kind {
                ProviderKind::Read => "read",
                ProviderKind::Transform => "transform",
                ProviderKind::Dependent => "dependent",
            }
            .into(),
        },
        kind,
        value_input: if kind == ProviderKind::Read {
            TypeDescriptor::Unit
        } else {
            TypeDescriptor::Text
        },
        value_output: TypeDescriptor::Text,
        pure: true,
        state_type: TypeDescriptor::Unit,
        continuation_type: reader_type("ReaderContinuation"),
    }
}
fn provider_plan(schema: &SchemaRef) -> ReaderPlan {
    let signature = signature(schema, ProviderKind::Read);
    let mut p = plan(
        schema,
        vec![ReaderExpr::Call(signature.operation.clone())],
        0,
        TypeDescriptor::Text,
    );
    p.providers.push(signature);
    p
}
fn terminal(value: &str, end: u64, budget: &mut Budget) -> Result<ProviderReply, StopReason> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::AllocationUnits, value.len() as u64)?;
    Ok(ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text(value.into()),
        end,
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            usage: budget.usage(),
            ..Report::default()
        },
    })))
}
fn annotated_terminal(
    schema: &SchemaRef,
    span: Span,
    end: u64,
    budget: &mut Budget,
) -> Result<ProviderReply, StopReason> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::Diagnostics, 1)?;
    budget.charge(Resource::Events, 1)?;
    let payload = TypedValue::Record(Record {
        schema: schema.clone(),
        kind: "Node".into(),
        fields: vec![],
    });
    let diagnostic = Diagnostic {
        schema: schema.clone(),
        code: "Note".into(),
        severity: Severity::Information,
        stage: "provider".into(),
        arguments: payload.clone(),
        primary: Some(span.clone()),
        related: vec![],
        fixes: vec![],
    };
    let event = Event {
        schema: schema.clone(),
        kind: "Read".into(),
        operation_path: vec![],
        span: Some(span),
        payload,
    };
    Ok(ProviderReply::Read(Box::new(ReadReply::Matched {
        value: NdfValue::Text("a".into()),
        end,
        new_state: NdfValue::Unit,
        view: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        facts: vec![],
        sources: vec![],
        source_maps: vec![],
        report: Report {
            diagnostics: vec![diagnostic],
            events: vec![event],
            usage: budget.usage(),
            trace_overflow: None,
        },
    })))
}
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
