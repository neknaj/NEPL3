use super::*;
use nepl3_doc_core::{check::Category, lower, model::*, pages::*};
use nepl3_doc_html::{ParallelMode, RenderOptions, pages::*};

fn request(source: &str) -> Result<(Compiled, PagesHtmlRequest), String> {
    let mut compiled = compiled()?;
    for descriptor in [
        nepl3_markup::schema::descriptor(&mut budget()),
        nepl3_doc_html::schema::descriptor(&mut budget()),
    ] {
        let descriptor = descriptor.map_err(err)?;
        compiled
            .doc
            .registry
            .register(
                descriptor.reference(&mut budget()).map_err(err)?,
                descriptor,
                &mut budget(),
            )
            .map_err(err)?;
    }
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    let document = with_input_route(true, &compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)
    })?;
    Ok((
        compiled,
        PagesHtmlRequest {
            set: PageSet {
                pages: vec![PageDocument {
                    registration: PageRegistration {
                        id: "external".into(),
                        source: "external.nepld".into(),
                        route: "docs/links/index.html".into(),
                    },
                    document,
                }],
            },
            options: RenderOptions {
                parallel: ParallelMode::Rows,
            },
        },
    ))
}


use nepl3_markup::html::*;
use nepl3_doc_html::portable::pages as wire;
fn rewrite(v:&mut nepl3_core::value::NdfValue,from:&str,to:&str)->usize{
 use nepl3_core::value::NdfValue::*;
 match v{Text(s) if s==from=>{*s=to.into();1},List(xs)=>xs.iter_mut().map(|x|rewrite(x,from,to)).sum(),Record(x)=>x.fields.iter_mut().map(|x|rewrite(x,from,to)).sum(),Variant(x)=>x.fields.iter_mut().map(|x|rewrite(x,from,to)).sum(),Some(x)=>rewrite(x,from,to),_=>0}
}

fn rewrite_link_target(v:&mut nepl3_core::value::NdfValue,from:&str,to:&str)->usize{
 use nepl3_core::value::NdfValue::*;
 match v{
 Variant(x) if x.type_name=="LinkTarget" && x.variant=="External"=>x.fields.iter_mut().map(|x|rewrite(x,from,to)).sum(),
 List(xs)=>xs.iter_mut().map(|x|rewrite_link_target(x,from,to)).sum(),Record(x)=>x.fields.iter_mut().map(|x|rewrite_link_target(x,from,to)).sum(),Variant(x)=>x.fields.iter_mut().map(|x|rewrite_link_target(x,from,to)).sum(),Some(x)=>rewrite_link_target(x,from,to),_=>0}
}
#[test]
fn independent_external_profile_and_sticky_stop(){
 let good=["http://example.org/","https://EXAMPLE.org:65535/a?b=one&c=two#x","https://xn--wgv71a119e.jp/%E6%97%A5","mailto:a@example.org?subject=x%20y","https://example.org/?x='&y=%22"];
 let bad=["","javascript:alert(1)","JaVaScRiPt:x","data:text/html,x","https://example.org:65536/","https://example.org:+1/","https://example.org:-1/","https://example.org:/","https://example.org:1.0/","https://user@example.org/","https://%65xample.org/","https://[::1]/","https://.example.org/","https://example..org/"," https://example.org/","https://example.org/\n","https://example.org/\t","https://example.org/\\x","https://example.org/%x0","https://example.org/%","https://example.org/\"x","https://example.org/<x>","//example.org/","/local","mailto:@example.org","mailto:a@","mailto:a/b@example.org"];
 for s in good { assert_eq!(external_uri(s,&mut budget()),Ok(true),"{s}"); }
 for s in bad { assert_eq!(external_uri(s,&mut budget()),Ok(false),"{s}"); }
 let s="https://example.org/";let n=4*s.len() as u64;
 for cap in [0,n-1,n] {let mut l=budget().limits();l.work=cap;let mut b=Budget::new(l);let v=external_uri(s,&mut b);if cap<n{assert_eq!(v,Err(StopReason::WorkLimit));assert_eq!(external_uri("",&mut b),Err(StopReason::WorkLimit));}else{assert_eq!(v,Ok(true));assert_eq!(b.usage().work,n);}assert_eq!(b.usage().allocation_units,0);}
 let mut b=budget();b.cancel();assert_eq!(external_uri("",&mut b),Err(StopReason::Cancelled));
}
#[test]
fn independent_external_fresh_wire_and_stale_response()->Result<(),String>{
 let original="https://example.org/?x='&y=%22#part";
 let input=format!("article en \"Links\" body cons paragraph cons sentence cons link external \"{original}\" text \"Go <&>\" cons text \" / \" cons link page \"external\" none text \"Self\" nil nil nil");
 let(c,request)=request(&input)?;let r=&c.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
 let result=render_pages(&request,r,&mut codec,&mut budget()).map_err(err)?;
 let m=&result.fragments[0].markup;let html=serialize(&validate(&m.fragment,m.slot,&m.policy,&mut budget()).map_err(err)?,&mut budget()).map_err(err)?;println!("INDEPENDENT_HTML={html}");assert!(html.contains("Go &lt;&amp;&gt;"));
 let raw=wire::request_to_value(&request,r,&mut codec,&mut budget()).map_err(err)?;let bytes=nepl3_wire::encode(&raw,&mut budget()).map_err(err)?;
 let mut fresh_a=SourceAdmission::default();let mut fresh=FoundationCodec::new(r,&empty,&mut fresh_a).map_err(err)?;
 let decoded=wire::request_from_value(&nepl3_wire::decode(&bytes,&mut budget()).map_err(err)?,r,&mut fresh,&mut budget()).map_err(err)?;assert_eq!(render_pages(&decoded,r,&mut fresh,&mut budget()).map_err(err)?,result);
 for changed in ["https://other.example.org/","javascript:alert(1)"] {
  let mut raw=wire::rendered_to_value(&result,&request,r,&mut codec,&mut budget()).map_err(err)?;assert_eq!(rewrite(&mut raw,original,changed),1);
  let bytes=nepl3_wire::encode(&raw,&mut budget()).map_err(err)?;let value=nepl3_wire::decode(&bytes,&mut budget()).map_err(err)?;
  assert!(wire::rendered_from_value(&value,&decoded,r,&mut fresh,&mut budget()).is_err());
 }
 let mut stale=request.clone();for n in &mut stale.set.pages[0].document.value.nodes {if let DocKind::Link{target:LinkTarget::External{uri},..}=&mut n.kind{*uri="https://other.example.org/".into();}}
 let raw=wire::rendered_to_value(&result,&request,r,&mut codec,&mut budget()).map_err(err)?;let bytes=nepl3_wire::encode(&raw,&mut budget()).map_err(err)?;
 assert!(wire::rendered_from_value(&nepl3_wire::decode(&bytes,&mut budget()).map_err(err)?,&stale,r,&mut fresh,&mut budget()).is_err());
 Ok(())
}
#[test]
fn independent_external_hidden_receiver_and_render_stops()->Result<(),String>{
 let(c,mut req)=request(r#"article en "T" body cons paragraph cons parallel cons variant en "Visible" cons variant ja sentence cons link external "https://example.org/" text "Hidden" nil nil nil nil"#)?;
 req.options.parallel=ParallelMode::Single{language:"en".into(),fallbacks:vec![]};let r=&c.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
 let mut full=budget();let good=render_pages(&req,r,&mut codec,&mut full).map_err(err)?;assert!(!format!("{good:?}").contains("https://example.org/"));
 for cap in [0,1,100,1000,full.usage().work-1]{let mut l=budget().limits();l.work=cap;let mut b=Budget::new(l);let mut ca=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut ca).map_err(err)?;println!("STOP_CAP={cap} baseline={}",full.usage().work);assert!(matches!(render_pages(&req,r,&mut codec,&mut b),Err(PagesRenderError::Stopped(StopReason::WorkLimit))));assert!(matches!(render_pages(&req,r,&mut codec,&mut b),Err(PagesRenderError::Stopped(StopReason::WorkLimit))));}
 for bad in ["https://example.org:+1/","https://example.org/\" onmouseover=\"x","javascript:alert(1)"]{
  let mut raw=wire::request_to_value(&req,r,&mut codec,&mut budget()).map_err(err)?;assert_eq!(rewrite_link_target(&mut raw,"https://example.org/",bad),1);
  let bytes=nepl3_wire::encode(&raw,&mut budget()).map_err(err)?;let mut fa=SourceAdmission::default();let mut fresh=FoundationCodec::new(r,&empty,&mut fa).map_err(err)?;
  let received=wire::request_from_value(&nepl3_wire::decode(&bytes,&mut budget()).map_err(err)?,r,&mut fresh,&mut budget()).map_err(err)?;
  assert!(matches!(render_pages(&received,r,&mut fresh,&mut budget()),Err(PagesRenderError::InvalidExternalUri{page:0,..})));
 }
 Ok(())
}
