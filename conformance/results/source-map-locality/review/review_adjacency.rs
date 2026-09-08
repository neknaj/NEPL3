use nepl3_core::{budget::*,origin::*,source::*};
fn err(e:impl core::fmt::Debug)->String {format!("{e:?}")}
fn limits()->Limits {Limits {source_bytes:1_000_000,work:20_000_000,allocation_units:20_000_000,depth:1000,nodes:1_000_000,output_bytes:0,diagnostics:0,events:0}}
fn budget()->Budget {Budget::new(limits())}
fn source(id:&str,revision:u64)->Result<SourceSnapshot,String>{SourceSnapshot::new(SourceId(id.into()),revision,format!("memory:{id}:{revision}"),b"aaaa".to_vec(),&mut budget()).map_err(err)}
fn mapping(s:&[SourceSnapshot],a:usize,x:u64,y:u64,b:usize,u:u64,v:u64,kind:MappingKind)->Result<Mapping,String>{Ok(Mapping {source:s[a].span(x,y).map_err(err)?,target:s[b].span(u,v).map_err(err)?,kind})}
// Each source has four byte vertices and five distinct insertion-anchor vertices.
// Boolean reachability is an independent cycle oracle, without snapshot Kahn traversal.
fn cycle(specs:&[(usize,u64,u64,usize,u64,u64,MappingKind)])->bool {
 let mut r=[[false;27];27];
 for &(a,x,y,b,u,v,kind) in specs {
  let left:Vec<usize>=if x==y {vec![a*9+4+x as usize]}else{(x..y).map(|i|a*9+i as usize).collect()};
  let right:Vec<usize>=if u==v {vec![b*9+4+u as usize]}else{(u..v).map(|i|b*9+i as usize).collect()};
  for (i,l) in left.iter().enumerate(){for (j,t) in right.iter().enumerate(){if kind==MappingKind::Transformed || i==j {r[*l][*t]=true;}}}
 }
 for k in 0..27 {for i in 0..27 {for j in 0..27 {r[i][j]|=r[i][k]&&r[k][j];}}}
 (0..27).any(|i|r[i][i])
}
#[test]
fn pointwise_oracle_duplicates_anchors_and_orders()->Result<(),String>{
 let s=vec![source("root",0)?,source("同源",1)?,source("同源",2)?];let mut store=SourceStore::default();for x in &s {store.insert(x.clone()).map_err(err)?;}
 let e=MappingKind::Exact;let t=MappingKind::Transformed;
 let templates=[(0,0,1,1,0,1,e),(1,0,1,0,1,2,e),(0,1,2,0,0,1,e),(1,1,3,2,0,2,t),(2,0,1,1,1,2,e),(0,0,0,1,0,0,e),(1,0,0,0,0,1,t),(2,3,4,2,2,3,e)];
 let mut valid=0;let mut invalid=0;
 for mask in 0..256 {
  let specs:Vec<_>=templates.iter().enumerate().filter_map(|(i,e)|if mask&(1<<i)!=0 {Some(*e)}else{None}).collect();let expected=cycle(&specs);
  let original:Vec<_>=specs.iter().map(|&(a,x,y,b,u,v,k)|mapping(&s,a,x,y,b,u,v,k)).collect::<Result<_,_>>()?;
  for order in 0..3 {
   let mut maps=original.clone();if order==1 {maps.reverse();} if order==2 {maps.extend(original.iter().cloned());if !maps.is_empty(){maps.rotate_left(1);}}
   let before=maps.clone();let mut b=budget();let got=SourceMap::validate_mappings(&maps,&store,&mut b).map(|_|());
   assert_eq!(got,if expected {Err(OriginError::Cycle)}else{Ok(())},"mask={mask} order={order}");assert_eq!(maps,before);assert_eq!(b.poll(),Ok(()));
   if expected {invalid+=1;}else{valid+=1;}
  }
 }
 // Exact content validation precedes graph reasoning. A missing declaration is
 // not repaired by matching source ID alone or another revision's presence.
 let maps=vec![mapping(&s,0,0,1,2,0,1,e)?];let mut missing=SourceStore::default();missing.insert(s[0].clone()).map_err(err)?;missing.insert(s[1].clone()).map_err(err)?;
 assert!(matches!(SourceMap::validate_mappings(&maps,&missing,&mut budget()),Err(OriginError::Source(SourceError::MissingSnapshot))));
 println!("pointwise_oracle valid={valid} cycles={invalid} permutations=3 masks=256");Ok(())
}
fn caller<T>(b:&mut Budget,n:usize,f:&mut impl FnMut(&mut Budget)->Result<T,OriginError>)->Result<T,OriginError>{if n==0 {f(b)}else{b.with_depth(|b|caller(b,n-1,f))}}
#[test]
fn dag_longest_depth_atomic_insert_and_stop_caps()->Result<(),String>{
 let s=(0..7).map(|i|source(&format!("v{i}"),0)).collect::<Result<Vec<_>,_>>()?;let mut store=SourceStore::default();for x in &s {store.insert(x.clone()).map_err(err)?;}
 let mut maps=Vec::new();for (a,b) in [(0,1),(0,1),(0,2),(1,3),(2,3),(3,4),(6,4),(4,5),(0,5)] {maps.push(mapping(&s,a,0,1,b,0,1,MappingKind::Exact)?);}
 let before=maps.clone();let mut stop_count=0;
 for order in 0..3 {
  if order==1 {maps.reverse();}if order==2 {maps.rotate_left(3);}
  let mut full=budget();caller(&mut full,5,&mut |b|SourceMap::validate_mappings(&maps,&store,b).map(|_|())).map_err(err)?;assert_eq!(full.usage().depth,10);assert_eq!(full.usage().nodes,7);assert_eq!(full.current_depth(),0);
  for resource in 0..4 {
   let u=full.usage();let needed=match resource {0=>u.work,1=>u.allocation_units,2=>u.nodes,_=>u.depth};
   for cap in [0,1,needed/2,needed-1,needed,needed+1] {
    let mut l=limits();let reason=match resource {0=>{l.work=cap;StopReason::WorkLimit},1=>{l.allocation_units=cap;StopReason::AllocationLimit},2=>{l.nodes=cap;StopReason::NodeLimit},_=>{l.depth=cap;StopReason::DepthLimit}};
    let mut b=Budget::new(l);let got=caller(&mut b,5,&mut |b|SourceMap::validate_mappings(&maps,&store,b).map(|_|()));assert_eq!(b.current_depth(),0);
    if cap<needed {assert_eq!(got,Err(OriginError::Stopped(reason)));assert_eq!(SourceMap::validate_mappings(&[],&store,&mut b).map(|_|()),Err(OriginError::Stopped(reason)));stop_count+=1;}else{assert_eq!(got,Ok(()));}
   }
  }
 }
 let mut accepted=SourceMap::default();accepted.insert(before[0].clone(),&store,&mut budget()).map_err(err)?;
 let mut l=limits();l.work=0;let mut stopped=Budget::new(l);let candidate=mapping(&s,1,0,1,0,0,1,MappingKind::Exact)?;
 assert_eq!(accepted.insert(candidate.clone(),&store,&mut stopped),Err(OriginError::Stopped(StopReason::WorkLimit)));
 assert!(accepted.validated().contains(&s[0].span(0,1).map_err(err)?,&s[1].span(0,1).map_err(err)?,&mut budget()).map_err(err)?);
 assert_eq!(accepted.insert(candidate,&store,&mut budget()),Err(OriginError::Cycle));
 accepted.insert(mapping(&s,1,0,1,2,0,1,MappingKind::Exact)?,&store,&mut budget()).map_err(err)?;
 assert!(accepted.validated().contains(&s[0].span(0,1).map_err(err)?,&s[2].span(0,1).map_err(err)?,&mut budget()).map_err(err)?);
 let mut cancelled=budget();cancelled.cancel();assert_eq!(SourceMap::validate_mappings(&before,&store,&mut cancelled).map(|_|()),Err(OriginError::Stopped(StopReason::Cancelled)));assert_eq!(cancelled.usage(),Usage::default());
 println!("stop_caps={stop_count} orders=3 caller_depth=5 longest_relative=5");Ok(())
}
#[test]
fn observe_star_work_and_additional_storage()->Result<(),String>{
 for width in [1024,2048] {
  let root=source("00000",0)?;let mut store=SourceStore::default();store.insert(root.clone()).map_err(err)?;let mut maps=Vec::new();
  for i in 1..=width {let leaf=source(&format!("{i:05}"),0)?;maps.push(Mapping{source:root.span(0,1).map_err(err)?,target:leaf.span(0,1).map_err(err)?,kind:MappingKind::Exact});store.insert(leaf).map_err(err)?;}
  let mut b=budget();SourceMap::validate_mappings(&maps,&store,&mut b).map_err(err)?;assert_eq!(b.usage().nodes,width as u64+1);assert_eq!(b.usage().depth,2);println!("star width={width} usage={:?} head_size={} edge_delta={}",b.usage(),core::mem::size_of::<Option<usize>>(),core::mem::size_of::<(usize,Option<usize>)>()-core::mem::size_of::<(usize,usize)>());
 }
 Ok(())
}
