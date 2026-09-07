use nepl3_core::{budget::{Budget,Limits,StopReason},schema::SchemaRegistry,source::{SourceStore,SourceAdmission},value::NdfValue};
use nepl3_doc_core::model::*;
use nepl3_doc_html::*;
use nepl3_wire::foundation::FoundationCodec;
fn err(e:impl std::fmt::Debug)->String{format!("{e:?}")}
fn b()->Budget{Budget::new(Limits{work:1_000_000_000,allocation_units:1_000_000_000,output_bytes:100_000_000,source_bytes:1_000_000,nodes:1_000_000,depth:100_000,diagnostics:100,events:100})}
fn registry()->Result<SchemaRegistry,String>{let mut r=SchemaRegistry::default();for d in [nepl3_core::schema::foundation::descriptor(&mut b()),nepl3_doc_core::schema::descriptor(&mut b()),nepl3_markup::schema::descriptor(&mut b()),schema::descriptor(&mut b())]{let d=d.map_err(err)?;r.register(d.reference(&mut b()).map_err(err)?,d,&mut b()).map_err(err)?;}r.finalize(&mut b()).map_err(err)?;Ok(r)}
fn node(kind:DocKind)->DocNode{DocNode{kind,locations:vec![],span:None,origin:None}}
fn request()->LocalHtmlRequest{LocalHtmlRequest{document:DocumentSyntax{value:DocValue{root:DocRoot::Article(ArticleRef(0)),nodes:vec![node(DocKind::Article{language:"ja".into(),title:SentenceRef(1),body:BodyRef(3)}),node(DocKind::Sentence{inlines:vec![InlineRef(2)]}),node(DocKind::Text{text:"本文🙂\r\n\t<&\"".into()}),node(DocKind::Body{blocks:vec![]})],embeds:vec![]},sources:vec![],origins:vec![],views:vec![],source_maps:vec![]},options:RenderOptions{parallel:ParallelMode::Rows}}}
fn serialize(f:&RenderedFragment)->Result<String,String>{let m=&f.markup;nepl3_markup::html::serialize(&nepl3_markup::html::validate(&m.fragment,m.slot,&m.policy,&mut b()).map_err(err)?,&mut b()).map_err(err)}
fn main()->Result<(),String>{
 let r=registry()?;let q=request();let unchanged=q.clone();let store=SourceStore::default();let mut admission=SourceAdmission::default();let mut codec=FoundationCodec::new(&r,&store,&mut admission).map_err(err)?;
 let prepared=prepare_local(&q.document,&q.options,&r,&mut codec,&mut b()).map_err(err)?;let fragment=render(&prepared,&mut b()).map_err(err)?;
 let req=portable::request_to_value(&q,&r,&mut codec,&mut b()).map_err(err)?;let reply=portable::rendered_to_value(&fragment,&prepared,&r,&mut codec,&mut b()).map_err(err)?;
 let mut total_stop=0;let mut total_ok=0;
 for phase in 0..2 {
  let execute=|bb:&mut Budget|->Result<(),portable::PortableError<nepl3_wire::WireError>>{let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&store,&mut a).unwrap();if phase==0 {portable::request_from_value(&req,&r,&mut c,bb).map(|x|assert_eq!(x,q))}else{portable::rendered_from_value(&reply,&prepared,&r,&mut c,bb).map(|x|assert_eq!(x,fragment))}};
  let mut measured=b();execute(&mut measured).map_err(err)?;let used=measured.usage();
  for resource in 0..5 {let (needed,reason)=match resource{0=>(used.work,StopReason::WorkLimit),1=>(used.allocation_units,StopReason::AllocationLimit),2=>(used.nodes,StopReason::NodeLimit),3=>(used.depth,StopReason::DepthLimit),_=>(used.output_bytes,StopReason::OutputLimit)};
   let mut caps=(0..=32).map(|i|needed*i/32).collect::<Vec<_>>();caps.extend([needed.saturating_sub(1),needed,needed+1]);caps.sort();caps.dedup();
   for cap in caps {let mut l=b().limits();match resource{0=>l.work=cap,1=>l.allocation_units=cap,2=>l.nodes=cap,3=>l.depth=cap,_=>l.output_bytes=cap};let mut bb=Budget::new(l);match execute(&mut bb){Ok(())=>{total_ok+=1;assert!(bb.poll().is_ok());},Err(portable::PortableError::Stopped(s))=>{total_stop+=1;assert_eq!(s,reason);assert_eq!(bb.poll(),Err(reason));},Err(e)=>return Err(format!("phase={phase} resource={resource} cap={cap} nested or semantic failure {e:?}"))};let actual=match resource{0=>bb.usage().work,1=>bb.usage().allocation_units,2=>bb.usage().nodes,3=>bb.usage().depth,_=>bb.usage().output_bytes};assert!(actual<=cap);assert_eq!(q,unchanged);}
  }
  let mut cancelled=b();cancelled.cancel();assert!(matches!(execute(&mut cancelled),Err(portable::PortableError::Stopped(StopReason::Cancelled))));
 }
 assert!(total_stop>0&&total_ok>0);println!("Independent raw request + replay decoder cap sweep: {total_stop} exact sticky stops, {total_ok} exact successful values; 5 resources, cancellation, input unchanged");Ok(())}
