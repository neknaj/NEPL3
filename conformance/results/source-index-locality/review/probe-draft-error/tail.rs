use nepl3_core::{budget::*,source::*};
fn b()->Budget {Budget::new(Limits{work:100_000_000,allocation_units:100_000_000,source_bytes:1_000_000,depth:100,nodes:1_000_000,output_bytes:1_000_000,diagnostics:10,events:10})}
fn src(id:&str,rev:u64,uri:&str,text:&str)->Result<SourceSnapshot,SourceError>{SourceSnapshot::new(SourceId(id.into()),rev,uri.into(),text.as_bytes().to_vec(),&mut b())}
fn insert(s:&mut SourceStore,v:&SourceSnapshot,borrow:bool,b:&mut Budget)->Result<(),SourceError>{if borrow{s.insert_ref_with_budget(v,b)}else{s.insert_with_budget(v.clone(),b)}}
fn verify(s:&SourceStore,expected:&[SourceSnapshot])->Result<(),SourceError>{
 assert_eq!(s.snapshots(),expected);
 for v in expected {assert_eq!(s.get_revision_with_budget(&v.identity().source,v.identity().revision,&mut b())?,Some(v));assert_eq!(s.get_ref(v.identity()),Some(v));assert_eq!(s.resolve(&v.reference()),Some(v));}
 Ok(())
}
#[test]
fn permutations_revisions_content_uri_and_lookup()->Result<(),SourceError>{
 let values=[src("a",0,"m:a0","x")?,src("a",u64::MAX,"m:amax","z")?,src("aa",0,"m:aa","t")?,src("z",7,"m:z","q")?,src("\u{6587}\u{5b57}",0,"m:unicode","\u{3042}")?];
 let mut count=0;
 for a in 0..5{for c in 0..5{for d in 0..5{for e in 0..5{for f in 0..5{
 let order=[a,c,d,e,f];if (0..5).any(|i|order[..i].contains(&order[i])){continue}
 for borrow in [false,true]{
  let mut s=SourceStore::default();let mut expected=Vec::new();
  for i in order {insert(&mut s,&values[i],borrow,&mut b())?;expected.push(values[i].clone());verify(&s,&expected)?;}
  for v in &values {
   let same=src(&v.identity().source.0,v.identity().revision,v.uri(),v.text())?;
   insert(&mut s,&same,borrow,&mut b())?;verify(&s,&expected)?;
   for changed in [src(&v.identity().source.0,v.identity().revision,"different:uri",v.text())?,src(&v.identity().source.0,v.identity().revision,v.uri(),"changed bytes")?] {
    assert_eq!(insert(&mut s,&changed,borrow,&mut b()),Err(SourceError::IdentityConflict));verify(&s,&expected)?;
   }
  }
  assert_eq!(s.latest(&SourceId("a".into())),Some(&values[1]));count+=1;
 }
 }}}}}
 assert_eq!(count,240);println!("all {count} permutation/owned/ref cases; independent equal snapshots and URI/digest conflicts preserved");Ok(())
}
#[test]
fn metered_faults_are_atomic_and_sticky()->Result<(),SourceError>{
 let original=[src("a",0,"m:a","a")?,src("c",0,"m:c","c")?];let insertions=[src("b",0,"m:b","b")?,src("z",0,"m:z","z")?,src("c",0,"m:c","c")?];let mut stops=0;
 for borrow in [false,true]{for v in &insertions{for allocation in [false,true]{for cap in 0..220{
  let mut s=SourceStore::default();for p in &original{s.insert(p.clone())?;}
  let mut limits=b().limits();if allocation{limits.allocation_units=cap}else{limits.work=cap}let mut limited=Budget::new(limits);
  match insert(&mut s,v,borrow,&mut limited){
   Ok(())=>{let mut expect=original.to_vec();if v.identity()!=original[1].identity(){expect.push(v.clone())}verify(&s,&expect)?;},
   Err(SourceError::Stopped(reason))=>{assert_eq!(reason,if allocation{StopReason::AllocationLimit}else{StopReason::WorkLimit});assert_eq!(limited.poll(),Err(reason));verify(&s,&original)?;stops+=1;},
   Err(e)=>return Err(e)
  }
 }}}}
 for borrow in [false,true]{for empty in [false,true]{let mut s=SourceStore::default();if !empty{s.insert(original[0].clone())?;}let prior=s.snapshots().to_vec();let mut cancelled=b();cancelled.cancel();assert_eq!(insert(&mut s,&insertions[0],borrow,&mut cancelled),Err(SourceError::Stopped(StopReason::Cancelled)));verify(&s,&prior)?;}}
 let id="\u{65e5}\u{672c}\u{8a9e}".repeat(1000);let long=src(&id,0,"m:long","x")?;
 for borrow in [false,true]{let mut s=SourceStore::default();s.insert(long.clone())?;let mut limited=Budget::new(Limits{work:17999,..b().limits()});assert_eq!(insert(&mut s,&long,borrow,&mut limited),Err(SourceError::Stopped(StopReason::WorkLimit)));verify(&s,&[long.clone()])?;}
 assert!(stops>0);println!("fault sweep 2640 attempts, {stops} typed sticky atomic stops; cancellation empty/nonempty; 9000-byte Unicode ID comparison precharged");Ok(())
}
#[test]
fn workload_order_costs_and_16000_cap()->Result<(),SourceError>{
 let values=(0..512).map(|i|src(&format!("s{i:07}"),0,"m:source","x")).collect::<Result<Vec<_>,_>>()?;
 for borrow in [false,true]{for mode in 0..3{
  let order:Vec<usize>=match mode{0=>(0..512).collect(),1=>(0..512).rev().collect(),_=>(0..512).map(|i|(i*197)%512).collect()};
  let mut s=SourceStore::default();let mut measured=b();for &i in &order{insert(&mut s,&values[i],borrow,&mut measured)?;}
  let expected=order.iter().map(|&i|values[i].clone()).collect::<Vec<_>>();verify(&s,&expected)?;
  println!("order={mode} borrowed={borrow} work={} allocation={}",measured.usage().work,measured.usage().allocation_units);
 }}
 let mut s=SourceStore::default();let mut limited=Budget::new(Limits{work:16000,..b().limits()});let mut result=Ok(());
 for v in &values{if let Err(e)=insert(&mut s,v,false,&mut limited){result=Err(e);break}}
 println!("owned ascending cap16000 result={result:?} accepted={} work={}",s.snapshots().len(),limited.usage().work);
 match result{Ok(())=>assert_eq!(s.snapshots().len(),512),Err(SourceError::Stopped(StopReason::WorkLimit))=>assert!(s.snapshots().len()<512),Err(e)=>return Err(e)}
 Ok(())
}

#[test]
fn mixed_apis_hint_movement_missing_neighbors_and_edits()->Result<(),SourceError>{
 let names=["m","c","x","a","g","p","z"];
 let values=names.iter().map(|id|src(id,0,&format!("m:{id}"),"abc")).collect::<Result<Vec<_>,_>>()?;
 for seed in 0..7 {
  let mut s=SourceStore::default();let mut expected=Vec::new();
  for (position,v) in values.iter().enumerate(){match (position+seed)%3{0=>s.insert(v.clone())?,1=>s.insert_with_budget(v.clone(),&mut b())?,_=>s.insert_ref_with_budget(v,&mut b())?};expected.push(v.clone());verify(&s,&expected)?;}
  // Each hit moves the hint, then test missing keys on either side and interior gaps.
  for (i,name) in ["0","b","f","h","n","y","zz"].iter().enumerate(){
   let hit=&values[(i*3+seed)%values.len()];s.insert_ref_with_budget(hit,&mut b())?;s.insert(hit.clone())?;
   let v=src(name,0,&format!("m:{name}"),"v")?;
   assert_eq!(s.get_revision_with_budget(&v.identity().source,0,&mut b())?,None);
   insert(&mut s,&v,i%2==0,&mut b())?;expected.push(v);verify(&s,&expected)?;
  }
  let edit=TextEdit{span:values[0].span(1,2)?,expected_digest:values[0].identity().digest,replacement:"Z".into()};
  let ids=s.apply(&[edit],&mut b(),&mut SourceAdmission::default())?;assert_eq!(ids.len(),1);
  let newest=s.latest(&SourceId("m".into())).expect("edited source").clone();assert_eq!(newest.text(),"aZc");assert_eq!(newest.identity().revision,1);expected.push(newest.clone());verify(&s,&expected)?;
  for v in [&values[0],&newest,&values[6]] {insert(&mut s,v,true,&mut b())?;verify(&s,&expected)?;}
 }
 // Different prior hint positions disappear at the atomic edit commit.
 let mut stores=[SourceStore::default(),SourceStore::default()];
 for (i,s) in stores.iter_mut().enumerate(){for v in &values{s.insert(v.clone())?;}s.insert(values[if i==0{3}else{6}].clone())?;
  let edit=TextEdit{span:values[0].span(1,2)?,expected_digest:values[0].identity().digest,replacement:"Z".into()};s.apply(&[edit],&mut b(),&mut SourceAdmission::default())?;
 }
 let mut costs=Vec::new();for s in &mut stores{let mut measured=b();s.insert_ref_with_budget(&values[1],&mut measured)?;costs.push(measured.usage());}
 assert_eq!(costs[0],costs[1]);assert_eq!(stores[0].snapshots(),stores[1].snapshots());
 println!("mixed API sequences, hit movement, seven missing gaps/endpoints, edit revision and old-hint-independent post-edit usage passed");Ok(())
}

#[test]
fn failed_insert_and_edit_do_not_move_hint()->Result<(),SourceError>{
 let values=[src("a",0,"m:a","abc")?,src("m",0,"m:m","abc")?,src("z",0,"m:z","abc")?];
 for mode in 0..5 {
  let mut s=SourceStore::default();let mut control=SourceStore::default();for v in &values{s.insert(v.clone())?;control.insert(v.clone())?;}
  let result=match mode{
   0=>s.insert(src("a",0,"bad:uri","abc")?),
   1=>s.insert_ref_with_budget(&src("a",0,"m:a","bad")?,&mut b()),
   2=>s.insert_with_budget(src("b",0,"m:b","b")?,&mut Budget::new(Limits{allocation_units:0,..b().limits()})),
   3=>s.insert_ref_with_budget(&values[0],&mut Budget::new(Limits{work:0,..b().limits()})),
   _=>{let edit=TextEdit{span:values[0].span(0,1)?,expected_digest:values[0].identity().digest,replacement:"q".into()};s.apply(&[edit],&mut Budget::new(Limits{allocation_units:0,..b().limits()}),&mut SourceAdmission::default()).map(|_|())}
  };assert!(result.is_err());verify(&s,&values)?;
  let mut actual=b();let mut expected=b();s.insert_ref_with_budget(&values[1],&mut actual)?;control.insert_ref_with_budget(&values[1],&mut expected)?;assert_eq!(actual.usage(),expected.usage());
 }
 println!("all five rejected/stopped mutation modes preserve subsequent metered search behavior");Ok(())
}

#[test]
fn bounded_sorted_window_and_alternating_duplicate_costs()->Result<(),SourceError>{
 let values=(0..512).map(|i|src(&format!("s{i:07}"),0,"m:source","x")).collect::<Result<Vec<_>,_>>()?;
 for mode in 0..3 {
  let mut s=SourceStore::default();if mode==0{s.insert(src("z",0,"m:z","z")?)?;}
  for v in &values{if mode!=0{s.insert(v.clone())?;}}
  let mut measured=b();for (i,v) in values.iter().enumerate(){if mode<2{s.insert_with_budget(v.clone(),&mut measured)?;}else{s.insert_with_budget(values[if i%2==0{0}else{511}].clone(),&mut measured)?;}}
  println!("window/hits mode={mode} work={} allocation={}",measured.usage().work,measured.usage().allocation_units);
  for v in &values{assert_eq!(s.get_revision_with_budget(&v.identity().source,0,&mut b())?,Some(v));}
 }
 Ok(())
}
