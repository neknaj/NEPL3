use nepl3_core::{budget::{Budget,Limits},source::*};
fn b()->Budget{Budget::new(Limits{source_bytes:1000000,work:10000000,allocation_units:10000000,nodes:100000,depth:100,..Limits::default()})}
fn sample(id:&str,rev:u64,text:&str)->Result<SourceSnapshot,SourceError>{SourceSnapshot::new(SourceId(id.into()),rev,"memory:same-uri".into(),text.as_bytes().to_vec(),&mut b())}
fn check(s:&SourceStore){for expected in s.snapshots(){assert_eq!(s.get_ref(expected.identity()),Some(expected));assert_eq!(s.get(expected.identity().clone()),Some(expected));assert_eq!(s.resolve(&expected.reference()),Some(expected));let mut wrong=expected.reference();wrong.digest.0[0]^=1;assert!(s.resolve(&wrong).is_none());}}
#[test]
fn independent_permutations_atomic_edits_and_failed_insertions()->Result<(),SourceError>{
 let samples=[sample("a",0,"alpha")?,sample("a",5,"newer")?,sample("a-prefix",0,"beta")?,sample("\u{6587}",u64::MAX,"unicode")?];
 for i in 0..4{for j in 0..4{for k in 0..4{for l in 0..4{let order=[i,j,k,l];if (0..4).any(|a|(a+1..4).any(|b|order[a]==order[b])){continue;}
 let mut s=SourceStore::default();let mut expected=Vec::new();for (n,index) in order.into_iter().enumerate(){match n%3{0=>s.insert(samples[index].clone())?,1=>s.insert_with_budget(samples[index].clone(),&mut b())?,_=>s.insert_ref_with_budget(&samples[index],&mut b())?};expected.push(samples[index].clone());check(&s);assert_eq!(s.snapshots(),expected);}
 let changed=sample("a",5,"conflict")?;assert!(s.insert(changed).is_err());check(&s);
 let mut lim=b().limits();lim.allocation_units=0;let extra=sample("0",0,"zero")?;assert!(s.insert_ref_with_budget(&extra,&mut Budget::new(lim)).is_err());check(&s);assert_eq!(s.snapshots(),expected);
 }}}}
 let a=sample("z",0,"abc")?;let q=sample("b",0,"xyz")?;
 let edits=[TextEdit{span:a.span(1,2)?,expected_digest:Digest::of(b"b"),replacement:"Q".into()},TextEdit{span:q.span(0,1)?,expected_digest:Digest::of(b"x"),replacement:"R".into()}];
 let mut limits=b().limits();for cap in [None,Some(0),Some(1),Some(100)]{let mut s=SourceStore::default();s.insert(a.clone())?;s.insert(q.clone())?;let before=s.snapshots().to_vec();if let Some(cap)=cap{limits.work=cap;}else{limits=b().limits();}let mut budget=Budget::new(limits);let result=s.apply(&edits,&mut budget,&mut SourceAdmission::default());assert_eq!(result.is_ok(),cap.is_none());match result{Ok(ids)=>{assert_eq!(ids.len(),2);assert_eq!(s.latest(&a.identity().source).unwrap().text(),"aQc");assert_eq!(s.latest(&q.identity().source).unwrap().text(),"Ryz");assert!(ids.iter().all(|id|s.get_ref(id).is_some()));},Err(_)=>assert_eq!(s.snapshots(),before)}check(&s);assert_eq!(s.get_ref(a.identity()),Some(&a));assert_eq!(s.get_ref(q.identity()),Some(&q));}
 Ok(())
}
