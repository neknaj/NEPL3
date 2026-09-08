use nepl3_tools::doc::source::{self,budget};
use nepl3_core::{source::{SourceStore,SourceAdmission},budget::Budget};
use nepl3_doc_core::{lower,check::Category,pages::*};
use nepl3_doc_html::{pages::{render_pages,PagesHtmlRequest,PagesRenderError},RenderOptions,ParallelMode};
use nepl3_wire::foundation::FoundationCodec;
fn err(e:impl std::fmt::Debug)->String{format!("{e:?}")}
#[test]
fn independent_self_hidden_anchor_and_sticky_stop()->Result<(),String>{
 let compiled=source::compiled()?;
 let input=r#"article en "Self" body cons paragraph cons sentence cons link relative "" some "hidden" text "go" nil cons parallel cons variant ja sentence cons anchor hidden text "Target" nil cons variant en "Translation" nil nil nil"#;
 let document=source::with_named_input(true,&compiled,input,"self","Article",|tree,profile,_,_|{
 let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(profile.registry(),&empty,&mut a).map_err(err)?;
 lower::document(tree.syntax(),&compiled.doc.package.schema,Category::Article,profile.registry(),&mut budget(),&mut c).map_err(err)})?;
 let mut request=PagesHtmlRequest{set:PageSet{pages:vec![PageDocument{registration:PageRegistration{id:"self".into(),source:"nested/doc.md".into(),route:"nested/index.html".into()},document}],files:vec![]},options:RenderOptions{parallel:ParallelMode::Single{language:"en".into(),fallbacks:vec![]}}};
 let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&compiled.doc.registry,&empty,&mut a).map_err(err)?;
 assert!(matches!(render_pages(&request,&compiled.doc.registry,&mut c,&mut budget()),Err(PagesRenderError::MissingOutputAnchor{page:0,target:0,..})));
 request.options.parallel=ParallelMode::Rows;let mut full=budget();render_pages(&request,&compiled.doc.registry,&mut c,&mut full).map_err(err)?;
 let mut limits=budget().limits();limits.work=full.usage().work-1;let mut stopped=Budget::new(limits);
 assert!(render_pages(&request,&compiled.doc.registry,&mut c,&mut stopped).is_err());assert!(stopped.poll().is_err());assert!(render_pages(&request,&compiled.doc.registry,&mut c,&mut stopped).is_err());
 let mut cancelled=budget();cancelled.cancel();assert!(render_pages(&request,&compiled.doc.registry,&mut c,&mut cancelled).is_err());
 Ok(())
}
