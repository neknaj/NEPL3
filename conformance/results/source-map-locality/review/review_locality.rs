use nepl3_core::{budget::*,origin::*,source::*};
fn err(e:impl core::fmt::Debug)->String{format!("{e:?}")}
fn limits()->Limits{Limits{source_bytes:1_000_000,work:20_000_000,allocation_units:20_000_000,nodes:1_000_000,depth:1000,output_bytes:0,diagnostics:0,events:0}}
fn budget()->Budget{Budget::new(limits())}
fn source(name:&str,revision:u64,data:&str)->Result<SourceSnapshot,String>{SourceSnapshot::new(SourceId(name.into()),revision,format!("memory:{name}:{revision}"),data.as_bytes().to_vec(),&mut budget()).map_err(err)}
fn map(a:&SourceSnapshot,b:&SourceSnapshot)->Result<Mapping,String>{Ok(Mapping{source:a.span(0,1).map_err(err)?,target:b.span(0,1).map_err(err)?,kind:MappingKind::Exact})}
fn validate(m:&[Mapping],s:&SourceStore,b:&mut Budget)->Result<(),OriginError>{SourceMap::validate_mappings(m,s,b).map(|_|())}
#[test]
fn full_identity_endpoint_roles_and_fresh_graphs()->Result<(),String>{
 let prefix="同じ接頭辞".repeat(24);
 let s=vec![source(&prefix,0,"aaaa")?,source(&prefix,1,"aaab")?,source(&prefix,u64::MAX,"aaac")?,source(&(prefix.clone()+"a"),0,"aaad")?,source(&(prefix.clone()+"b"),0,"aaae")?,source("z",0,"aaaf")?];
 let mut store=SourceStore::default();for v in &s{store.insert(v.clone()).map_err(err)?;}
 // All six logical vertices differ by complete identity, despite identical
 // first-byte content and common names. A->B->C with joins has no cycle.
 let edges=[(0,1),(1,2),(0,2),(2,3),(4,3),(3,5)];
 let mut count=0;let mut total=0;
 for order in 0..6 {for reverse in [false,true]{for duplicate in [false,true]{
  let mut pairs=edges.to_vec();pairs.rotate_left(order);if reverse{pairs.reverse();}if duplicate{pairs.extend_from_within(..);}
  let maps=pairs.iter().map(|&(a,b)|map(&s[a],&s[b])).collect::<Result<Vec<_>,_>>()?;let before=maps.clone();let mut b=budget();validate(&maps,&store,&mut b).map_err(err)?;assert_eq!(b.usage().nodes,6);assert_eq!(b.usage().depth,5);assert_eq!(maps,before);count+=1;total+=b.usage().work;
  let mut cyclic=maps;cyclic.push(map(&s[5],&s[0])?);assert_eq!(validate(&cyclic,&store,&mut budget()),Err(OriginError::Cycle));
 }}}
 // Matching source/revision but a different full-content digest must fail even
 // when every mapped byte equals 'a'. It cannot hit a previously resolved ID.
 let forged=source(&prefix,0,"aaaz")?;let valid=map(&s[0],&s[5])?;
 for bad in [map(&forged,&s[5])?,map(&s[5],&forged)?]{
  let maps=vec![valid.clone(),valid.clone(),bad];let mut b=budget();assert_eq!(validate(&maps,&store,&mut b),Err(OriginError::Source(SourceError::MissingSnapshot)));assert_eq!(b.poll(),Ok(()));
 }
 assert_eq!(store.insert(forged.clone()),Err(SourceError::IdentityConflict));
 let mut other=SourceStore::default();other.insert(forged.clone()).map_err(err)?;other.insert(s[5].clone()).map_err(err)?;
 validate(&[map(&forged,&s[5])?],&other,&mut budget()).map_err(err)?;
 assert_eq!(validate(&[valid.clone()],&other,&mut budget()),Err(OriginError::Source(SourceError::MissingSnapshot)));
 validate(&[valid],&store,&mut budget()).map_err(err)?;
 println!("identity orders={count} successful_work_sum={total} altered_digest=2 fresh_graphs=3");Ok(())
}
#[test]
fn repeated_endpoints_stop_before_comparison_and_preserve_usage()->Result<(),String>{
 let a=source("a",0,"aaaa")?;let b=source("b",0,"aaaa")?;let mut store=SourceStore::default();store.insert(a.clone()).map_err(err)?;store.insert(b.clone()).map_err(err)?;
 let m=map(&a,&b)?;let mut usages=Vec::new();
 for n in [1,2,3,8]{let maps=vec![m.clone();n];let mut b=budget();validate(&maps,&store,&mut b).map_err(err)?;usages.push(b.usage());println!("repeat n={n} usage={:?}",b.usage());}
 let maps=vec![m;8];let before=maps.clone();let full=usages[3];let mut stops=0;
 for cap in 0..=full.work{
  let mut l=limits();l.work=cap;let mut b=Budget::new(l);let got=validate(&maps,&store,&mut b);assert!(b.usage().work<=cap);
  if cap<full.work{assert_eq!(got,Err(OriginError::Stopped(StopReason::WorkLimit)));let saved=b.usage();assert_eq!(validate(&[],&store,&mut b),Err(OriginError::Stopped(StopReason::WorkLimit)));assert_eq!(b.usage(),saved);stops+=1;}else{assert_eq!(got,Ok(()));assert_eq!(b.usage(),full);}
  assert_eq!(maps,before);
 }
 let mut l=limits();l.work=0;let mut zero=Budget::new(l);assert_eq!(validate(&vec![maps[0].clone();100000],&store,&mut zero),Err(OriginError::Stopped(StopReason::WorkLimit)));assert_eq!(zero.usage(),Usage::default());
 let mut cancelled=budget();cancelled.cancel();assert_eq!(validate(&maps,&store,&mut cancelled),Err(OriginError::Stopped(StopReason::Cancelled)));assert_eq!(cancelled.usage(),Usage::default());
 println!("exhaustive_work_stops={stops} endpoint_comparison_utf8_bytes=1 fixed_identity_bytes=41");Ok(())
}
#[test]
fn star_same_budget_before_after_and_disjoint_misses()->Result<(),String>{
 for width in [1024,2048]{
  let root=source("00000",0,"aaaa")?;let mut store=SourceStore::default();store.insert(root.clone()).map_err(err)?;let mut maps=Vec::new();
  for i in 1..=width{let leaf=source(&format!("{i:05}"),0,"aaaa")?;maps.push(map(&root,&leaf)?);store.insert(leaf).map_err(err)?;}
  let mut b=budget();validate(&maps,&store,&mut b).map_err(err)?;let mut l=limits();l.work=1_500_000;let mut capped=Budget::new(l);let got=validate(&maps,&store,&mut capped);
  if b.usage().work>l.work{assert_eq!(got,Err(OriginError::Stopped(StopReason::WorkLimit)));}else{assert_eq!(got,Ok(()));}
  println!("star width={width} usage={:?} cap1500000={got:?}",b.usage());
 }
 // No repeated endpoint: the locality check adds real comparison work on misses.
 let mut store=SourceStore::default();let mut maps=Vec::new();
 for i in 0..16{let a=source(&format!("{i:02}a"),0,"aaaa")?;let b=source(&format!("{i:02}b"),0,"aaaa")?;maps.push(map(&a,&b)?);store.insert(a).map_err(err)?;store.insert(b).map_err(err)?;}
 for reverse in [false,true]{if reverse{maps.reverse();}let mut b=budget();validate(&maps,&store,&mut b).map_err(err)?;println!("disjoint reverse={reverse} usage={:?}",b.usage());}
 Ok(())
}
