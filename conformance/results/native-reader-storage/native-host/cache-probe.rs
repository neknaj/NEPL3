
#[cfg(test)] mod review_cache {
 use super::*;
 fn budget()->Budget{Budget::new(nepl3_core::budget::Limits{work:1_000_000,source_bytes:1_000_000,allocation_units:1_000_000,..Default::default()})}
 fn src(id:&str,uri:&str,text:&str)->SourceSnapshot{SourceSnapshot::new(nepl3_core::source::SourceId(id.into()),0,uri.into(),text.as_bytes().to_vec(),&mut budget()).expect("source")}
 #[test] fn review_cache_all_reorders_scope_replacement_and_failure_boundaries(){
  let a=src("a","memory:a","a");let b=src("b","memory:b","b");let c=src("c","memory:c","c");let input=[a,b,c];
  for order in [[0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0]] {let mut s=SourceStore::default();refresh_sources(&mut s,&input,&mut budget()).expect("initial");let request=order.map(|i|input[i].clone());refresh_sources(&mut s,&request,&mut budget()).expect("reorder");assert_eq!(s.snapshots(),request);refresh_sources(&mut s,&request[1..2],&mut budget()).expect("shrink");assert_eq!(s.snapshots(),&request[1..2]);}
  for request in [vec![input[0].clone(),input[1].clone(),input[2].clone()],vec![input[2].clone(),input[1].clone()],vec![src("a","memory:new","replacement")]] {
   let setup=||{let mut s=SourceStore::default();refresh_sources(&mut s,&input[..1],&mut budget()).expect("seed");s};let mut full=budget();refresh_sources(&mut setup(),&request,&mut full).expect("full");
   for resource in [Resource::Work,Resource::AllocationUnits] {let max=if resource==Resource::Work{full.usage().work}else{full.usage().allocation_units};for cap in 0..=max {let mut s=setup();let mut l=budget().limits();if resource==Resource::Work{l.work=cap}else{l.allocation_units=cap};let mut q=Budget::new(l);let result=refresh_sources(&mut s,&request,&mut q);assert_eq!(q.usage().source_bytes,0);if result.is_ok(){assert_eq!(s.snapshots(),request)}else{assert!(matches!(result,Err(ParseError::Stopped(_))|Err(ParseError::Source(nepl3_core::source::SourceError::Stopped(_)))));};refresh_sources(&mut s,&[],&mut budget()).expect("empty subsequent scope");assert!(s.snapshots().is_empty());}}
  }
  let mut s=SourceStore::default();refresh_sources(&mut s,&input,&mut budget()).expect("seed");let independent=input.iter().map(|x|src(&x.identity().source.0,x.uri(),x.text())).collect::<Vec<_>>();let mut limited=Budget::new(nepl3_core::budget::Limits{work:3,..Default::default()});assert!(refresh_sources(&mut s,&independent,&mut limited).is_err());assert_eq!(s.snapshots(),input);
 }
}
