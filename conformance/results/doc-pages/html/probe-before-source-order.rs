use super::*;
use nepl3_doc_core::{check::Category, lower, model::*, pages::*};
use nepl3_doc_html::{pages::*, portable::pages as wire, ParallelMode, RenderOptions};
use nepl3_markup::html::{self, HtmlAttribute, HtmlHref, HtmlNode};

fn make(inputs: &[(&str, &str, &str, &str)]) -> Result<(Compiled, PagesHtmlRequest), String> {
    let mut compiled = compiled()?;
    for d in [nepl3_markup::schema::descriptor(&mut budget()), nepl3_doc_html::schema::descriptor(&mut budget())] {
        let d=d.map_err(err)?;
        compiled.doc.registry.register(d.reference(&mut budget()).map_err(err)?,d,&mut budget()).map_err(err)?;
    }
    compiled.doc.registry.finalize(&mut budget()).map_err(err)?;
    let mut pages=Vec::new();
    for (id,source,route,input) in inputs {
        let document=nepl3_tools::doc::source::with_named_input(true,&compiled,input,id,"Article",|tree,profile,b,a| {
            let checked=tree.tree().bundle.validate_with_sources(profile.registry(),b,a).map_err(err)?;
            let empty=SourceStore::default();let mut admission=SourceAdmission::default();
            let mut c=FoundationCodec::new(profile.registry(),&empty,&mut admission).map_err(err)?;
            lower::document(&checked,&compiled.doc.package.schema,Category::Article,profile.registry(),&mut budget(),&mut c).map_err(err)
        })?;
        pages.push(PageDocument{registration:PageRegistration{id:(*id).into(),source:(*source).into(),route:(*route).into()},document});
    }
    Ok((compiled,PagesHtmlRequest{set:PageSet{pages},options:RenderOptions{parallel:ParallelMode::Rows}}))
}
fn pair() -> Result<(Compiled,PagesHtmlRequest),String> {
    let a="article en \"Start\" body cons paragraph cons sentence cons link relative \"../refs/target.nepld\" some \"\u{898b}\u{51fa}\u{3057}\" text \"go<&\" cons link page \"a\" none text \"self\" nil nil nil";
    let b="article en \"Target\" body cons section \u{898b}\u{51fa}\u{3057} \"Heading\" body nil nil";
    make(&[("a","manual/start.nepld","docs/start/index.html",a),("b","refs/target.nepld","reference/v1/page.html",b)])
}
fn round(v:&NdfValue)->Result<NdfValue,String> {
    nepl3_wire::decode(&nepl3_wire::encode(v,&mut budget()).map_err(err)?,&mut budget()).map_err(err)
}
fn alter_href(v:&mut NdfValue, field:usize)->bool {
    match v {
        NdfValue::Variant(x) if x.variant=="BetweenArtifacts" => {
            x.fields[field]=if field==2 {NdfValue::Some(Box::new(NdfValue::Text("n-66616b65".into())))}else {NdfValue::Text("forged/index.html".into())};true
        },
        NdfValue::Record(x)=>x.fields.iter_mut().any(|v|alter_href(v,field)),
        NdfValue::Variant(x)=>x.fields.iter_mut().any(|v|alter_href(v,field)),
        NdfValue::List(x)=>x.iter_mut().any(|v|alter_href(v,field)),
        NdfValue::Some(x)=>alter_href(x,field),_=>false
    }
}
#[test]
fn independent_pages_first_receiver_rejects_forged_source_target_fragment_and_stale_requests()->Result<(),String>{
    let (compiled,request)=pair()?;let r=&compiled.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
    let actual=render_pages(&request,r,&mut c,&mut budget()).map_err(err)?;
    let mut strings=Vec::new();
    for (i,f) in actual.fragments.iter().enumerate(){
        let checked=html::validate(&f.markup.fragment,f.markup.slot,&f.markup.policy,&mut budget()).map_err(err)?;
        strings.push(html::serialize(&checked,&mut budget()).map_err(err)?);
        assert_eq!(f.origins.len(),f.markup.fragment.nodes.len());
        for (n,cause) in f.origins.iter().enumerate(){assert_eq!(cause.element,n as u64);assert!((cause.node as usize)<request.set.pages[i].document.value.nodes.len());}
    }
    assert!(strings[0].contains("href=\"../../reference/v1/page.html#n-e8a68be587bae38197\""));
    assert!(strings[0].contains("href=\"index.html\""));
    assert!(strings[0].contains("go&lt;&amp;"));
    assert!(strings[1].contains("id=\"n-e8a68be587bae38197\""));
    let request_value=round(&wire::request_to_value(&request,r,&mut c,&mut budget()).map_err(err)?)?;
    let result_value=round(&wire::rendered_to_value(&actual,&request,r,&mut c,&mut budget()).map_err(err)?)?;
    let mut admission=SourceAdmission::default();let mut receiver=FoundationCodec::new(r,&empty,&mut admission).map_err(err)?;let mut b=budget();
    let got=wire::request_from_value(&request_value,r,&mut receiver,&mut b).map_err(err)?;assert_eq!(got,request);
    let admitted=b.usage().source_bytes;assert!(admitted>0);
    assert_eq!(wire::rendered_from_value(&result_value,&got,r,&mut receiver,&mut b).map_err(err)?,actual);assert_eq!(b.usage().source_bytes,admitted);
    for field in 0..3 {let mut forged=result_value.clone();assert!(alter_href(&mut forged,field));assert!(wire::rendered_from_value(&round(&forged)?,&got,r,&mut receiver,&mut budget()).is_err());}
    for mode in 0..6 {
        let mut changed=got.clone();
        match mode {
            0=>changed.set.pages[0].registration.route="moved/index.html".into(),
            1=>changed.options.parallel=ParallelMode::Columns,
            2=>changed.set.pages.swap(0,1),
            3=>{changed.set.pages.pop();},
            4=>changed.set.pages[1].registration.source="refs/new.nepld".into(),
            _=>{for n in &mut changed.set.pages[1].document.value.nodes {if let DocKind::Text{text}= &mut n.kind {text.push('!');break;}}},
        }
        let changed=wire::request_from_value(&round(&wire::request_to_value(&changed,r,&mut receiver,&mut budget()).map_err(err)?)?,r,&mut receiver,&mut budget()).map_err(err)?;
        assert!(wire::rendered_from_value(&result_value,&changed,r,&mut receiver,&mut budget()).is_err(),"stale mode {mode}");
    }
    for mode in 0..3 {let mut bad=result_value.clone();let NdfValue::Record(root)=&mut bad else{return Err("shape".into())};if mode==0 {root.fields[0]=NdfValue::Bytes(vec![0;32]);}else {let NdfValue::List(fs)=&mut root.fields[1] else{return Err("shape".into())};if mode==1{fs.swap(0,1);}else{fs.pop();}}assert!(wire::rendered_from_value(&round(&bad)?,&got,r,&mut receiver,&mut budget()).is_err());}
    Ok(())
}
#[test]
fn independent_pages_only_emitted_crosslinks_require_visible_anchor()->Result<(),String>{
    let source="article en \"From\" body cons paragraph cons parallel cons variant ja sentence cons link page \"b\" some \"hidden\" text \"go\" nil cons variant en \"No link\" nil nil nil";
    let target="article en \"To\" body cons paragraph cons parallel cons variant ja sentence cons anchor hidden text \"anchor\" nil cons variant en \"No anchor\" nil nil nil";
    let (compiled,mut request)=make(&[("a","a.nepld","a/index.html",source),("b","b.nepld","b/index.html",target)])?;
    let r=&compiled.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
    for mode in [ParallelMode::Rows,ParallelMode::Columns,ParallelMode::Single{language:"en".into(),fallbacks:vec![]},ParallelMode::Single{language:"ja".into(),fallbacks:vec![]}] {request.options.parallel=mode;render_pages(&request,r,&mut c,&mut budget()).map_err(err)?;}
    // Now source link survives in en while target anchor remains ja-only.
    for n in &mut request.set.pages[0].document.value.nodes {if let DocKind::Variant{language,..}=&mut n.kind {if language=="ja"{*language="en".into()}else{*language="ja".into()}}}
    request.options.parallel=ParallelMode::Single{language:"en".into(),fallbacks:vec![]};
    let failure=render_pages(&request,r,&mut c,&mut budget());
    let Err(PagesRenderError::MissingOutputAnchor{page,node,target})=failure else{return Err(format!("unexpected {failure:?}"))};assert_eq!((page,target),(0,1));assert!(matches!(request.set.pages[0].document.value.nodes[node as usize].kind,DocKind::Link{..}));
    Ok(())
}
#[test]
fn independent_pages_resources_local_guard_and_unresolved_external_remain_errors()->Result<(),String>{
    let (compiled,request)=pair()?;let r=&compiled.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
    assert!(matches!(nepl3_doc_html::prepare_local(&request.set.pages[0].document,&request.options,r,&mut c,&mut budget()),Err(nepl3_doc_html::LocalPreparationError::NeedsResolution(_))));
    let mut full=budget();let value=render_pages(&request,r,&mut c,&mut full).map_err(err)?;let usage=full.usage();
    for (resource,max,reason) in [(Resource::Work,usage.work,StopReason::WorkLimit),(Resource::AllocationUnits,usage.allocation_units,StopReason::AllocationLimit)] {
        for cap in [0,1,max/4,max/2,max.saturating_sub(1)] {let mut limits=budget().limits();match resource {Resource::Work=>limits.work=cap,_=>limits.allocation_units=cap};let mut b=Budget::new(limits);let mut admission=SourceAdmission::default();let mut receiver=FoundationCodec::new(r,&empty,&mut admission).map_err(err)?;let result=render_pages(&request,r,&mut receiver,&mut b);assert!(matches!(result,Err(PagesRenderError::Stopped(s)) if s==reason),"cap {cap}: {result:?}");assert_eq!(b.current_depth(),0);}
    }
    let mut external=request.clone();for node in &mut external.set.pages[0].document.value.nodes {if let DocKind::Link{target,..}=&mut node.kind {*target=LinkTarget::External{uri:"https://example.com/".into()};break;}}
    assert!(matches!(render_pages(&external,r,&mut c,&mut budget()),Err(PagesRenderError::NeedsResolution(plan)) if !plan.remaining.is_empty()));
    let mut cancelled=budget();cancelled.cancel();assert!(matches!(render_pages(&request,r,&mut c,&mut cancelled),Err(PagesRenderError::Stopped(StopReason::Cancelled))));
    let encoded=wire::rendered_to_value(&value,&request,r,&mut c,&mut budget()).map_err(err)?;
    assert!(matches!(wire::rendered_from_value(&encoded,&request,r,&mut c,&mut cancelled),Err(wire::PagesPortableError::Boundary(nepl3_doc_html::portable::PortableError::Stopped(StopReason::Cancelled)))));
    Ok(())
}
