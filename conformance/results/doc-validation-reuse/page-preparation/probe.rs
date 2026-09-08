mod review_reuse {
    use super::{err,registry};
    use nepl3_core::{budget::{Budget,Limits,StopReason},source::{Digest,SourceAdmission,SourceId,SourceSnapshot,SourceStore},origin::{Origin,OriginId},value::NdfValue};
    use nepl3_doc_core::{model::*,pages::{self,PageSet,PageDocument,PageRegistration},portable as dp};
    use nepl3_doc_html::{pages::{render_pages,PagesHtmlRequest},portable as hp,ParallelMode,RenderOptions};
    use nepl3_wire::foundation::FoundationCodec;
    fn b()->Budget { Budget::new(Limits{work:100_000_000,allocation_units:500_000_000,nodes:10_000_000,depth:1000,source_bytes:1_000_000,output_bytes:10_000_000,diagnostics:100,events:100}) }
    fn document(index:usize,sourced:bool)->Result<DocumentSyntax,String> {
        let title=if index==0 {"一<&🙂"}else{"二\r\n"};
        let kinds=vec![DocKind::Article{language:"ja".into(),title:SentenceRef(1),body:BodyRef(3)},DocKind::Sentence{inlines:vec![InlineRef(2)]},DocKind::Text{text:title.into()},DocKind::Body{blocks:vec![BlockRef(4)]},DocKind::Paragraph{items:vec![FlowRef(5)]},DocKind::Sentence{inlines:vec![InlineRef(6),InlineRef(7)]},DocKind::Anchor{id:"導入".into(),label:InlineRef(2)},DocKind::Link{target:LinkTarget::Page{page:if index==0 {"p1"}else{"p0"}.into(),fragment:Some("導入".into())},label:InlineRef(2)}];
        let mut doc=DocumentSyntax{value:DocValue{root:DocRoot::Article(ArticleRef(0)),nodes:kinds.into_iter().map(|kind|DocNode{kind,locations:vec![],span:None,origin:None}).collect(),embeds:vec![]},sources:vec![],origins:vec![],views:vec![],source_maps:vec![]};
        if sourced {
            let source=SourceSnapshot::new(SourceId(format!("page-{index}")),3,format!("memory:page-{index}"),title.as_bytes().to_vec(),&mut b()).map_err(err)?;
            let span=source.span(0,source.text().len() as u64).map_err(err)?;
            for node in &mut doc.value.nodes {node.span=Some(span.clone());node.origin=Some(OriginId(0));}
            doc.origins.push(Origin::Direct(span));doc.sources.push(source);
        }
        Ok(doc)
    }
    fn input(sourced:bool)->Result<PagesHtmlRequest,String> {
        Ok(PagesHtmlRequest{set:PageSet{pages:(0..2).map(|i|Ok(PageDocument{registration:PageRegistration{id:format!("p{i}"),source:format!("doc/p{i}.nepld"),route:format!("docs/p{i}/index.html")},document:document(i,sourced)?})).collect::<Result<Vec<_>,String>>()?},options:RenderOptions{parallel:ParallelMode::Rows}})
    }
    #[test]
    fn review_identity_origins_first_receiver_and_stale_output()->Result<(),String> {
        let r=registry()?;let empty=SourceStore::default();
        for sourced in [false,true] {
            let req=input(sourced)?;let original=req.clone();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
            let setvalue=dp::pages::set_to_value(&req.set,&r,&mut c,&mut b()).map_err(err)?;
            let bytes=nepl3_wire::encode(&setvalue,&mut b()).map_err(err)?;
            let mut expected=pages::SET_DOMAIN.to_vec();expected.extend(&bytes);
            let mut budget=b();let rendered=render_pages(&req,&r,&mut c,&mut budget).map_err(err)?;
            assert_eq!(rendered.identity,Digest::of(&expected));
            for (i,f) in rendered.fragments.iter().enumerate() {
                let value=dp::to_value(&req.set.pages[i].document,&r,&mut c,&mut b()).map_err(err)?;
                let mut expected=nepl3_doc_core::prepare::DOCUMENT_DOMAIN.to_vec();expected.extend(nepl3_wire::encode(&value,&mut b()).map_err(err)?);
                assert_eq!(f.document_digest,Digest::of(&expected));
                let m=&f.markup;let checked=nepl3_markup::html::validate(&m.fragment,m.slot,&m.policy,&mut b()).map_err(err)?;
                let html=nepl3_markup::html::serialize(&checked,&mut b()).map_err(err)?;
                assert!(html.contains(&format!("../p{}/index.html#n-e5b08ee585a5",1-i)));
                assert_eq!(f.origins.len(),f.markup.fragment.nodes.len());
                assert!(f.origins.iter().enumerate().all(|(n,o)|o.element==n as u64&&o.node<8));
                println!("REVIEW_EQUAL html {sourced} {i} {html:?} {:?}",f.origins);
            }
            let encoded=hp::pages::request_to_value(&req,&r,&mut c,&mut b()).map_err(err)?;
            let packet=nepl3_wire::encode(&encoded,&mut b()).map_err(err)?;
            let reply=hp::pages::rendered_to_value(&rendered,&req,&r,&mut c,&mut b()).map_err(err)?;
            let replybytes=nepl3_wire::encode(&reply,&mut b()).map_err(err)?;
            println!("REVIEW_EQUAL packet {sourced} {:?} {:?}",Digest::of(&packet),Digest::of(&replybytes));
            println!("REVIEW_USAGE {sourced} {:?}",budget.usage());
            let mut a=SourceAdmission::default();let mut fresh=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
            let received=hp::pages::request_from_value(&nepl3_wire::decode(&packet,&mut b()).map_err(err)?,&r,&mut fresh,&mut b()).map_err(err)?;
            let raw=nepl3_wire::decode(&replybytes,&mut b()).map_err(err)?;
            assert_eq!(rendered,hp::pages::rendered_from_value(&raw,&received,&r,&mut fresh,&mut b()).map_err(err)?);
            for mutation in 0..3 {
                let mut changed=received.clone();
                match mutation {0=>changed.options.parallel=ParallelMode::Columns,1=>changed.set.pages[0].registration.route="moved/index.html".into(),_=>changed.set.pages[0].document.value.nodes[2].kind=DocKind::Text{text:"modified".into()}}
                assert!(hp::pages::rendered_from_value(&raw,&changed,&r,&mut fresh,&mut b()).is_err());
            }
            assert_eq!(req,original);
        }
        Ok(())
    }
    #[test]
    fn review_failure_order_source_closure_and_plan_are_not_proofs()->Result<(),String> {
        let r=registry()?;let empty=SourceStore::default();
        for case in 0..7 {
            let mut req=input(true)?;
            if case<=1 {req.set.pages[0].document.value.nodes[5].kind=DocKind::Sentence{inlines:vec![InlineRef(6),InlineRef(6),InlineRef(7)]};}
            match case {0=>req.set.pages[1].document.value.root=DocRoot::Article(ArticleRef(u64::MAX)),1=>{},2=>req.set.pages[0].document.sources.clear(),3=>{let s=req.set.pages[0].document.sources[0].clone();req.set.pages[1].document.sources.push(s);},4=>req.set.pages[1].document.sources.push(SourceSnapshot::new(SourceId("page-0".into()),3,"memory:page-0".into(),b"different".to_vec(),&mut b()).map_err(err)?),5=>req.set.pages[1].registration.id="p0".into(),_=>req.set.pages[1].document.value.nodes[7].kind=DocKind::Link{target:LinkTarget::Page{page:"missing".into(),fragment:None},label:InlineRef(2)}}
            let original=req.clone();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;let mut budget=b();
            let result=pages::resolve(&req.set,&r,&mut c,&mut budget).map(|p|p.into_plan());
            if case==3 {assert!(result.is_ok());}else{assert!(result.is_err());}
            let out=format!("{result:?}");if case==0 {assert!(out.starts_with("Err(Boundary("));}if case==1 {assert!(out.starts_with("Err(Input { page: 0,"));}
            assert!(budget.poll().is_ok());assert_eq!(req,original);println!("REVIEW_EQUAL failure {case} {out}");
        }
        let req=input(true)?;let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
        let plan=pages::resolve(&req.set,&r,&mut c,&mut b()).map_err(err)?.into_plan();
        let mut raw=dp::pages::plan_to_value(&plan,&req.set,&r,&mut c,&mut b()).map_err(err)?;
        let NdfValue::Record(ref mut record)=raw else{return Err("plan".into())};record.fields[1]=NdfValue::List(vec![]);
        let packet=nepl3_wire::encode(&raw,&mut b()).map_err(err)?;let raw=nepl3_wire::decode(&packet,&mut b()).map_err(err)?;
        let mut a=SourceAdmission::default();let mut fresh=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
        assert!(dp::pages::plan_from_value(&raw,&req.set,&r,&mut fresh,&mut b()).is_err());
        Ok(())
    }
    #[test]
    fn review_foreign_requirements_remain_closed_and_unevaluated()->Result<(),String> {
        use nepl3_core::{syntax::{Environment,EnvironmentEntry,EnvironmentRef,ForeignClosure,ForeignSyntax,NodeRef,SyntaxBundle,SyntaxNode},value_codec::FoundationValueCodec};
        let r=registry()?;let mut req=input(true)?;let empty=SourceStore::default();
        let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
        let source=SourceSnapshot::new(SourceId("independent-guest".into()),7,"memory:guest".into(),b"not an Article meaning".to_vec(),&mut b()).map_err(err)?;
        let span=source.span(0,source.text().len() as u64).map_err(err)?;
        let env=Environment{bindings:vec![],resources:vec![]};let digest=c.environment_digest(&env,&mut b()).map_err(err)?;
        let schema=r.selected("nepl3.doc",1).ok_or("schema")?.clone();
        let closure=ForeignClosure{syntax:ForeignSyntax{schema:schema.clone(),category:"Article".into(),root:NodeRef(0),environment:EnvironmentRef{id:71,digest},bundle:SyntaxBundle{sources:vec![source],nodes:vec![SyntaxNode{schema,kind:"View:TextRun".into(),fields:vec![],head:Some(span.clone()),cover:Some(span.clone()),origin:OriginId(0),token:None}],origins:vec![Origin::Direct(span)],root:NodeRef(0),environments:vec![],tokens:vec![],source_maps:vec![]}},owner_environment:EnvironmentEntry{id:71,digest,value:env},owner_origins:vec![],owner_sources:vec![],owner_source_maps:vec![]};
        req.set.pages[0].document.value.embeds.push(DocEmbed{kind:EmbedKind::Code,closure});
        req.set.pages[0].document.value.nodes.push(DocNode{kind:DocKind::Code{syntax:EmbedRef(0)},span:None,origin:None,locations:vec![]});
        req.set.pages[0].document.value.nodes[3].kind=DocKind::Body{blocks:vec![BlockRef(4),BlockRef(8)]};
        let expected=nepl3_doc_core::prepare::inspect(&req.set.pages[0].document,&r,&mut c,&mut b()).map_err(err)?;
        let plan=pages::resolve(&req.set,&r,&mut c,&mut b()).map_err(err)?.into_plan();
        assert_eq!(plan.remaining.len(),1);assert_eq!(plan.remaining[0].page,0);
        assert_eq!(Some(&plan.remaining[0].requirement),expected.requirements.last());
        assert!(matches!(render_pages(&req,&r,&mut c,&mut b()),Err(nepl3_doc_html::pages::PagesRenderError::NeedsResolution(p)) if p==plan));
        let packet=nepl3_wire::encode(&hp::pages::request_to_value(&req,&r,&mut c,&mut b()).map_err(err)?,&mut b()).map_err(err)?;
        let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
        let got=hp::pages::request_from_value(&nepl3_wire::decode(&packet,&mut b()).map_err(err)?,&r,&mut c,&mut b()).map_err(err)?;
        assert_eq!(pages::resolve(&got.set,&r,&mut c,&mut b()).map_err(err)?.plan(),&plan);
        println!("REVIEW_EQUAL foreign {:?} {:?}",plan,Digest::of(&packet));
        for bad in 0..2 {
            let mut changed=got.clone();let closure=&mut changed.set.pages[0].document.value.embeds[0].closure;
            if bad==0 {closure.syntax.bundle.sources.clear();}else{closure.owner_environment.id=72;}
            let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
            let mut budget=b();assert!(pages::resolve(&changed.set,&r,&mut c,&mut budget).is_err());assert!(budget.poll().is_ok());
        }
        Ok(())
    }
    #[test]
    fn review_stops_are_sticky_and_input_is_immutable()->Result<(),String> {
        let r=registry()?;let req=input(true)?;let original=req.clone();let empty=SourceStore::default();
        let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;let mut budget=b();
        render_pages(&req,&r,&mut c,&mut budget).map_err(err)?;let needed=budget.usage();
        let values=[needed.source_bytes,needed.work,needed.allocation_units,needed.nodes,needed.depth,needed.output_bytes];
        for resource in 0..6 {
            for fraction in 0..9 {
                let cap=values[resource]*fraction/8;let mut limits=b().limits();match resource {0=>limits.source_bytes=cap,1=>limits.work=cap,2=>limits.allocation_units=cap,3=>limits.nodes=cap,4=>limits.depth=cap,_=>limits.output_bytes=cap}
                let mut budget=Budget::new(limits);let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
                let result=render_pages(&req,&r,&mut c,&mut budget);let stop=budget.poll();
                if fraction==8 {assert!(result.is_ok(),"{resource} {cap} {result:?}");}else{
                    assert!(result.is_err()&&stop.is_err(),"{resource} {cap}");
                    assert!(render_pages(&req,&r,&mut c,&mut budget).is_err());assert_eq!(budget.poll(),stop);
                }
                assert_eq!(req,original);
            }
        }
        let mut budget=b();budget.stop(StopReason::Cancelled);let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
        assert!(render_pages(&req,&r,&mut c,&mut budget).is_err());assert_eq!(budget.poll(),Err(StopReason::Cancelled));assert_eq!(req,original);
        Ok(())
    }
}
