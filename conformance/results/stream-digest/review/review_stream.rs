use nepl3_core::{budget::*,schema::*,source::*,value::*,value_codec::FoundationValueCodec};
use nepl3_wire::{foundation::FoundationCodec,WireError};
fn err(e: impl core::fmt::Debug)->String {format!("{e:?}")}
fn limits()->Limits { Limits {source_bytes:0,work:20_000_000,allocation_units:20_000_000,output_bytes:2_000_000,depth:1024,nodes:1_000_000,diagnostics:0,events:0} }
fn budget()->Budget {Budget::new(limits())}
fn registry()->Result<SchemaRegistry,String> {
 let mut r=SchemaRegistry::default();let d=nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?;
 r.register(d.reference(&mut budget()).map_err(err)?,d,&mut budget()).map_err(err)?;r.finalize(&mut budget()).map_err(err)?;Ok(r)
}
fn magnitude(n:usize)->Vec<u8> { let mut b=vec![0;n];b[0]=0x80;b }
fn schema()->SchemaRef {SchemaRef {package:"p".into(),revision:2,digest:Digest([7;32])}}
fn unhex(s:&str)->Result<Vec<u8>,String> {(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).map_err(err)).collect()}
fn hex(b:&[u8])->String {b.iter().map(|v|format!("{v:02x}")).collect()}
include!("review_vectors.inc");
#[test]
fn independent_canonical_bytes_hashes_and_accounting()->Result<(),String> {
 let r=registry()?;let store=SourceStore::default();let mut admission=SourceAdmission::default();let mut codec=FoundationCodec::new(&r,&store,&mut admission).map_err(err)?;
 let domains:[&[u8];3]=[b"",b"probe\0",&[0,255,128,65]];
 let all=vectors()?;let count=all.len();
 for (name,value,expected,hashes) in all {
  let mut enc=budget();let bytes=nepl3_wire::encode(&value,&mut enc).map_err(err)?;assert_eq!(bytes,expected,"{name}");
  for (i,domain) in domains.iter().enumerate() {
   let mut l=limits();l.output_bytes=32;let mut b=Budget::new(l);let digest=codec.canonical_value_digest(domain,&value,&mut b).map_err(err)?;
   assert_eq!(hex(&digest.0),hashes[i],"{name}");assert_eq!(b.usage().output_bytes,32);assert_eq!(b.usage().source_bytes,0);
   assert_eq!(b.usage().work,enc.usage().work+bytes.len() as u64+domain.len() as u64,"{name}");
   assert_eq!(b.usage().allocation_units+bytes.len() as u64,enc.usage().allocation_units,"{name}");
   assert_eq!(b.usage().nodes,enc.usage().nodes);assert_eq!(b.usage().depth,enc.usage().depth);
  }
 }
 println!("independent_vectors={count} domains=3");Ok(())
}
fn caller<T>(b:&mut Budget,n:usize,f:&mut impl FnMut(&mut Budget)->Result<T,WireError>)->Result<T,WireError> {if n==0 {f(b)} else {b.with_depth(|b|caller(b,n-1,f))}}
#[test]
fn independent_stops_depth_and_fresh_operation()->Result<(),String> {
 let r=registry()?;let store=SourceStore::default();let mut admission=SourceAdmission::default();let mut codec=FoundationCodec::new(&r,&store,&mut admission).map_err(err)?;
 let mut value=NdfValue::Text("文😀\0".repeat(64));for _ in 0..32 {value=NdfValue::Some(Box::new(value));}
 let mut full=budget();let expected=caller(&mut full,7,&mut |b|codec.canonical_value_digest(b"scope\0",&value,b)).map_err(err)?;
 assert_eq!(full.current_depth(),0);assert_eq!(full.usage().depth,40);
 let usage=full.usage();let mut stops=0;
 for resource in 0..5 {
  let need=match resource {0=>usage.work,1=>usage.allocation_units,2=>usage.output_bytes,3=>usage.nodes,_=>usage.depth};
  for cap in [0,1,need/2,need-1,need,need+1] {
   let mut l=limits();let reason=match resource {0=>{l.work=cap;StopReason::WorkLimit},1=>{l.allocation_units=cap;StopReason::AllocationLimit},2=>{l.output_bytes=cap;StopReason::OutputLimit},3=>{l.nodes=cap;StopReason::NodeLimit},_=>{l.depth=cap;StopReason::DepthLimit}};
   let mut b=Budget::new(l);let result=caller(&mut b,7,&mut |b|codec.canonical_value_digest(b"scope\0",&value,b));assert_eq!(b.current_depth(),0);
   if cap<need {assert_eq!(result,Err(WireError::Stopped(reason)));assert_eq!(b.poll(),Err(reason));assert_eq!(codec.canonical_value_digest(b"",&NdfValue::Unit,&mut b),Err(WireError::Stopped(reason)));stops+=1;} else {assert_eq!(result,Ok(expected));}
  }
 }
 let mut cancelled=budget();cancelled.cancel();assert_eq!(codec.canonical_value_digest(b"",&value,&mut cancelled),Err(WireError::Stopped(StopReason::Cancelled)));assert_eq!(cancelled.usage(),Usage::default());
 let mut fresh=budget();assert_eq!(caller(&mut fresh,7,&mut |b|codec.canonical_value_digest(b"scope\0",&value,b)),Ok(expected));
 println!("boundary_stops={stops} caller_depth=7 usage={usage:?}");Ok(())
}
#[test]
fn observe_original_frontier_and_denominator_caps()->Result<(),String> {
 let r=registry()?;let store=SourceStore::default();let mut admission=SourceAdmission::default();let mut codec=FoundationCodec::new(&r,&store,&mut admission).map_err(err)?;
 for width in [1000,100000] {
  let v=NdfValue::List(vec![NdfValue::Unit;width]);let mut l=limits();l.work=15;let mut b=Budget::new(l);
  assert_eq!(codec.canonical_value_digest(b"",&v,&mut b),Err(WireError::Stopped(StopReason::WorkLimit)));println!("frontier width={width} usage={:?}",b.usage());
 }
 let den=magnitude(1024);let v=NdfValue::Rational(Rational::from_canonical(Integer::from(1_i64),&den).map_err(err)?);let mut l=limits();l.work=128;let mut b=Budget::new(l);
 assert_eq!(codec.canonical_value_digest(b"",&v,&mut b),Err(WireError::Stopped(StopReason::WorkLimit)));println!("denominator bytes=1024 usage={:?}",b.usage());
 Ok(())
}
