use super::*;
use nepl3_core::origin::{Mapping, MappingKind};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
struct Count;
static ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Count {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) { ALLOCATED.fetch_add(layout.size(), Ordering::Relaxed); }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) { unsafe { System.dealloc(p, layout) } }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if ACTIVE.load(Ordering::Relaxed) { ALLOCATED.fetch_add(size, Ordering::Relaxed); }
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOC: Count = Count;

fn generated(id: &str, text: &str) -> Result<SourceSnapshot, ReaderError> {
    Ok(SourceSnapshot::new(SourceId(id.into()), 0, format!("memory:{id}"), text.as_bytes().to_vec(), &mut budget())?)
}
fn mapped_reply(end: u64, maps: Vec<Mapping>, added: Vec<SourceSnapshot>, capture: Option<Span>, b: &mut Budget) -> Result<ProviderReply, ReaderError> {
    let mut reply=terminal("b",end,b)?;
    if let ProviderReply::Read(v)=&mut reply && let ReadReply::Matched{sources,source_maps,facts,..}=v.as_mut() {
        *sources=added;*source_maps=maps;
        if let Some(span)=capture {facts.push(ReaderFact::Capture{name:"derived".into(),span});}
    }
    Ok(reply)
}
#[test]
fn independent_union_provider_cross_part_failures_retry_and_containment() -> Result<(),ReaderError> {
    let (registry,schema)=registry()?;
    for mode in 0..5 {
        let mut p=provider_plan(&schema);
        p.expressions.push(ReaderExpr::Seq(vec![ReaderId(0),ReaderId(0)]));
        p.rules[0].root=ReaderId(1);p.rules[0].output=TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
        let checked=p.check(&registry,&mut budget())?;
        let mut b=budget();let mut admission=SourceAdmission::default();
        let mut session=ReaderSession::new(format!("union-{mode}"),&checked,&registry,&mut b)?;
        let input=source("ab")?;let aux=generated(&("long".repeat(2500)+"aux"),"b")?;
        let aux2=generated(&("long".repeat(2500)+"next"),"b")?;
        let badtext=generated("bad-text","q")?;
        let other=generated("other","b")?;
        let mut store=SourceStore::default();store.insert(input.clone())?;
        let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut b,&mut admission)?;
        let first=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut admission)?;
        let ReadReply::Await{continuation,..}=first else{return Err(ReaderError::NoPending)};
        // The first provider declares provenance for the next byte, without claiming a capture.
        let firstmap=Mapping{source:input.span(1,2)?,target:aux.span(0,1)?,kind:MappingKind::Exact};
        let first=mapped_reply(1,vec![firstmap.clone()],vec![aux.clone()],None,&mut b)?;
        let next=session.resume(&continuation,first,&store,&mut b,&mut admission)?;
        let ReadReply::Await{continuation,report:prefix,..}=next else{return Err(ReaderError::NoPending)};
        let saved=continuation.clone();
        let secondmap=Mapping{source:aux.span(0,1)?,target:aux2.span(0,1)?,kind:MappingKind::Exact};
        if mode<4 {
            let (maps,declared)=match mode {
                0 => (vec![Mapping{source:aux.span(0,1)?,target:input.span(1,2)?,kind:MappingKind::Exact}],vec![]),
                1 => (vec![Mapping{source:aux.span(0,1)?,target:badtext.span(0,1)?,kind:MappingKind::Exact}],vec![badtext.clone()]),
                2 => (vec![secondmap.clone()],vec![]),
                _ => (vec![secondmap.clone(),Mapping{source:other.span(0,1)?,target:aux2.span(0,1)?,kind:MappingKind::Exact}],vec![aux2.clone(),other.clone()]),
            };
            let capture=if mode==3{Some(aux2.span(0,1)?)}else{None};
            let bad=mapped_reply(2,maps,declared,capture,&mut b)?;
            let rejected=session.resume(&continuation,bad,&store,&mut b,&mut admission);
            println!("mode={mode} rejected={rejected:?}");
            assert!(rejected.is_err());assert_eq!(b.poll(),Ok(()));assert_eq!(continuation,saved);
        }
        let valid=mapped_reply(2,vec![secondmap.clone()],vec![aux2.clone()],Some(aux2.span(0,1)?),&mut b)?;
        let before=b.usage();ALLOCATED.store(0,Ordering::Relaxed);ACTIVE.store(true,Ordering::Relaxed);
        let result=session.resume(&continuation,valid,&store,&mut b,&mut admission);
        ACTIVE.store(false,Ordering::Relaxed);
        let measured=ALLOCATED.load(Ordering::Relaxed);
        let ReadReply::Matched{value,end,source_maps,sources,facts,report,..}=result? else{return Err(ReaderError::ProviderContract)};
        assert_eq!(end,2);assert_eq!(value,NdfValue::List(vec![NdfValue::Text("b".into()),NdfValue::Text("b".into())]));
        assert_eq!(source_maps,vec![firstmap,secondmap]);assert_eq!(sources,vec![aux,aux2]);assert_eq!(facts.len(),1);
        assert_eq!(report.diagnostics,prefix.diagnostics);assert_eq!(report.events,prefix.events);
        println!("mode={mode} complete allocation_actual={measured} allocation_units={} work={}",b.usage().allocation_units-before.allocation_units,b.usage().work-before.work);
    }
    Ok(())
}

#[test]
fn independent_union_sticky_stops_preserve_pending_prefix() -> Result<(),ReaderError> {
    use nepl3_core::budget::{Resource,StopReason};
    let (registry,schema)=registry()?;
    for mode in 0..4 {
        let mut p=provider_plan(&schema);
        p.expressions.push(ReaderExpr::Seq(vec![ReaderId(0),ReaderId(0)]));
        p.rules[0].root=ReaderId(1);p.rules[0].output=TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
        let checked=p.check(&registry,&mut budget())?;
        let mut b=budget();let mut admission=SourceAdmission::default();
        let mut session=ReaderSession::new(format!("stops-{mode}"),&checked,&registry,&mut b)?;
        let input=source("ab")?;let aux=generated("aux","b")?;let aux2=generated("next","b")?;
        let mut store=SourceStore::default();store.insert(input.clone())?;
        let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut b,&mut admission)?;
        let first=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut admission)?;
        let ReadReply::Await{continuation,..}=first else{return Err(ReaderError::NoPending)};
        let map=Mapping{source:input.span(1,2)?,target:aux.span(0,1)?,kind:MappingKind::Exact};
        let reply=mapped_reply(1,vec![map.clone()],vec![aux.clone()],None,&mut b)?;
        let ReadReply::Await{continuation,..}=session.resume(&continuation,reply,&store,&mut b,&mut admission)? else{return Err(ReaderError::NoPending)};
        let saved=continuation.clone();
        let reply=mapped_reply(2,vec![Mapping{source:aux.span(0,1)?,target:aux2.span(0,1)?,kind:MappingKind::Exact}],vec![aux2.clone()],Some(aux2.span(0,1)?),&mut b)?;
        let limits=b.limits();let usage=b.usage();
        let expected=match mode {
            0=>{b.charge(Resource::Work,limits.work-usage.work)?;StopReason::WorkLimit},
            1=>{b.charge(Resource::AllocationUnits,limits.allocation_units-usage.allocation_units)?;StopReason::AllocationLimit},
            2=>{b.charge(Resource::SourceBytes,limits.source_bytes-usage.source_bytes)?;StopReason::SourceLimit},
            _=>{b.cancel();StopReason::Cancelled},
        };
        let result=session.resume(&continuation,reply,&store,&mut b,&mut admission);
        match result {
            Err(ref error)=>assert_eq!(error.stop_reason(),Some(expected)),
            Ok(ReadReply::Stopped{reason,source_maps,sources,..})=>{
                assert_eq!(reason,expected);assert_eq!(source_maps,vec![map]);assert_eq!(sources,vec![aux]);
            },
            other=>panic!("unexpected stop: {other:?}"),
        }
        assert_eq!(b.poll(),Err(expected));assert_eq!(continuation,saved);
        println!("sticky mode={mode} reason={expected:?} original continuation unchanged");
    }
    Ok(())
}

#[test]
fn independent_union_choice_rollback_removes_sources_and_maps() -> Result<(),ReaderError> {
    let (registry,schema)=registry()?;let mut p=provider_plan(&schema);
    p.expressions.extend([ReaderExpr::Literal("!".into()),ReaderExpr::Seq(vec![ReaderId(0),ReaderId(1)]),ReaderExpr::Seq(vec![ReaderId(0)]),ReaderExpr::Choice(vec![ReaderId(2),ReaderId(3)])]);
    p.rules[0].root=ReaderId(4);p.rules[0].output=TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue));
    let checked=p.check(&registry,&mut budget())?;
    let mut b=budget();let mut admission=SourceAdmission::default();
    let mut session=ReaderSession::new("rollback".into(),&checked,&registry,&mut b)?;
    let input=source("ab")?;let aux=generated("aux","a")?;let next=generated("next","a")?;
    let mut store=SourceStore::default();store.insert(input.clone())?;
    let raw=context(&schema,&registry)?;let ctx=check_context(&raw,&store,&registry,&mut b,&mut admission)?;
    let first=session.read("entry",ReadRequest{snapshot:&input,start:0,limit:2,final_input:true,context:&ctx,state:&NdfValue::Unit},&store,&mut b,&mut admission)?;
    let ReadReply::Await{continuation,..}=first else{return Err(ReaderError::NoPending)};
    let reply=mapped_reply(1,vec![Mapping{source:input.span(0,1)?,target:aux.span(0,1)?,kind:MappingKind::Exact}],vec![aux.clone()],None,&mut b)?;
    let ReadReply::Await{continuation,..}=session.resume(&continuation,reply,&store,&mut b,&mut admission)? else{return Err(ReaderError::NoPending)};
    let bad=mapped_reply(1,vec![Mapping{source:aux.span(0,1)?,target:next.span(0,1)?,kind:MappingKind::Exact}],vec![next],None,&mut b)?;
    let error=session.resume(&continuation,bad,&store,&mut b,&mut admission).expect_err("rolled-back aux must be absent");
    assert_eq!(error,ReaderError::Origin(nepl3_core::origin::OriginError::Source(nepl3_core::source::SourceError::MissingSnapshot)));
    let valid=terminal("a",1,&mut b)?;
    let ReadReply::Matched{end,sources,source_maps,..}=session.resume(&continuation,valid,&store,&mut b,&mut admission)? else{return Err(ReaderError::ProviderContract)};
    assert_eq!(end,1);assert!(sources.is_empty());assert!(source_maps.is_empty());
    println!("choice rollback missing old aux, retry complete without leaked artifacts");
    Ok(())
}
