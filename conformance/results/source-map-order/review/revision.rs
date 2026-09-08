use nepl3_core::{budget::*,source::*,origin::*};
type R=Result<(),Box<dyn std::error::Error>>;
fn b()->Budget {Budget::new(Limits{work:100_000_000,allocation_units:100_000_000,source_bytes:1_000_000,depth:10000,nodes:1_000_000,output_bytes:1_000_000,diagnostics:10,events:10})}
fn src(id:&str,rev:u64,text:&str)->Result<SourceSnapshot,String>{SourceSnapshot::new(SourceId(id.into()),rev,format!("m:{id}:{rev}"),text.as_bytes().to_vec(),&mut b()).map_err(|e|format!("{e:?}"))}
fn edge(a:&SourceSnapshot,i:u64,c:&SourceSnapshot,j:u64)->Result<Mapping,String>{Ok(Mapping{source:a.span(i,i+1).map_err(|e|format!("{e:?}"))?,target:c.span(j,j+1).map_err(|e|format!("{e:?}"))?,kind:MappingKind::Exact})}
fn check(m:&[Mapping],s:&SourceStore,b:&mut Budget)->Result<(),OriginError>{SourceMap::validate_mappings(m,s,b).map(|_|())}
fn store(v:&[SourceSnapshot])->Result<SourceStore,String>{let mut s=SourceStore::default();for x in v{s.insert(x.clone()).map_err(|e|format!("{e:?}"))?;}Ok(s)}
#[test]
fn four_vertex_oracle_mixed_orders_and_parts()->R{
 let v=[src("z",0,"x")?,src("\u{65e5}\u{672c}",u64::MAX,"x")?,src("\u{65e5}\u{672c}",0,"x")?,src("a",3,"x")?];let s=store(&v)?;
 for seed in 0..512u32 {let mask=(seed.wrapping_mul(40503).wrapping_add(217))&65535;let mut reach=[[false;4];4];let mut maps=Vec::new();
 for i in 0..4{for j in 0..4{if mask&(1<<(i*4+j))!=0 {reach[i][j]=true;maps.push(edge(&v[i],0,&v[j],0)?);}}}
 for k in 0..4{for i in 0..4{for j in 0..4{reach[i][j]|=reach[i][k]&&reach[k][j];}}}let expected=if (0..4).any(|i|reach[i][i]){Err(OriginError::Cycle)}else{Ok(())};
 for mode in 0..3 {if mode==1{maps.reverse();}if mode==2&&!maps.is_empty(){let len=maps.len();maps.rotate_left(seed as usize%len);}
 for split in 0..=maps.len(){assert_eq!(SourceMap::validate_mapping_parts(&maps[..split],&maps[split..],&s,&mut b()).map(|_|()),expected,"seed={seed} order={mode} split={split}");}}
 }
 println!("512 four-vertex Boolean reachability oracles x 3 orders x every split; complete Unicode/revision identity");Ok(())
}
#[test]
fn coarse_cycle_fallback_identity_and_faults()->R{
 let v=[src("same",0,"xx")?,src("same",1,"xx")?,src("a",0,"xx")?];let s=store(&v)?;
 let noncycle=vec![edge(&v[0],0,&v[1],0)?,edge(&v[1],1,&v[0],1)?,edge(&v[2],0,&v[0],0)?];assert_eq!(check(&noncycle,&s,&mut b()),Ok(()));
 let mut cycle=noncycle.clone();cycle.push(edge(&v[1],0,&v[0],0)?);assert_eq!(check(&cycle,&s,&mut b()),Err(OriginError::Cycle));
 let shift=vec![edge(&v[0],0,&v[0],1)?];assert_eq!(check(&shift,&s,&mut b()),Ok(()));
 let wrong=src("same",0,"xy")?;let malformed=vec![edge(&wrong,0,&v[1],0)?];assert_eq!(check(&malformed,&s,&mut b()),Err(OriginError::Source(SourceError::MissingSnapshot)));
 let original=s.snapshots().to_vec();let mut stops=0;
 for resource in 0..4 {for cap in 0..180 {let mut limits=b().limits();match resource{0=>limits.work=cap,1=>limits.allocation_units=cap,2=>limits.nodes=cap,_=>limits.depth=cap}let mut budget=Budget::new(limits);
 match check(&noncycle,&s,&mut budget){Ok(())=>(),Err(OriginError::Stopped(reason))=>{stops+=1;assert_eq!(budget.poll(),Err(reason));assert_eq!(check(&cycle,&s,&mut budget),Err(OriginError::Stopped(reason)));},Err(e)=>return Err(format!("unexpected {e:?}").into())};assert_eq!(s.snapshots(),original);
 }}let mut cancelled=b();cancelled.cancel();assert_eq!(check(&noncycle,&s,&mut cancelled),Err(OriginError::Stopped(StopReason::Cancelled)));
 println!("coarse cyclic but pointwise acyclic, actual cycle, self-shift, digest-forged missing closure, 720 budget cases {stops} sticky stops");Ok(())
}
#[test]
fn graph_work_distributions()->R{
 let v=(0..257).map(|i|src(&format!("s{i:07}"),0,"x")).collect::<Result<Vec<_>,_>>()?;let s=store(&v)?;
 for shape in 0..3 {for order in 0..3 {let mut maps=Vec::new();for p in 0..256{let i=match order{0=>p,1=>255-p,_=>(p*197)%256};maps.push(match shape{0=>edge(&v[i],0,&v[i+1],0)?,1=>edge(&v[0],0,&v[i+1],0)?,_=>edge(&v[0],0,&v[1],0)?});}
 let mut budget=b();check(&maps,&s,&mut budget).map_err(|e|format!("{e:?}"))?;println!("shape={shape} order={order} work={} allocation={} nodes={} depth={}",budget.usage().work,budget.usage().allocation_units,budget.usage().nodes,budget.usage().depth);}}
 let long="\u{65e5}\u{672c}\u{8a9e}".repeat(1000);let v=[src(&(long.clone()+"z"),0,"x")?,src(&(long.clone()+"a"),0,"x")?,src(&(long+"m"),0,"x")?];let s=store(&v)?;let maps=vec![edge(&v[0],0,&v[1],0)?,edge(&v[1],0,&v[2],0)?];let mut budget=b();check(&maps,&s,&mut budget).map_err(|e|format!("{e:?}"))?;println!("longunicode work={} allocation={}",budget.usage().work,budget.usage().allocation_units);
 let mut small=Budget::new(Limits{work:100,..b().limits()});assert_eq!(check(&maps,&s,&mut small),Err(OriginError::Stopped(StopReason::WorkLimit)));Ok(())
}
