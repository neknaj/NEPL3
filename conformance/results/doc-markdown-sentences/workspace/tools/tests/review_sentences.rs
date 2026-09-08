use nepl3_tools::doc::{source::{compiled,budget,with_input_route,err},projection::{from_source,markdown,Error}};
use nepl3_core::{budget::{Budget,StopReason},source::{SourceStore,SourceAdmission,Digest}};
use nepl3_doc_core::{lower,check::Category};
use nepl3_wire::foundation::FoundationCodec;
use pulldown_cmark::{Parser,Event,Tag,TagEnd};

fn events(markdown: &str) -> Result<Vec<String>,String> {
    let mut out=Vec::<String>::new();
    for e in Parser::new(markdown) {
        let v=match e {
            Event::Text(t)=> {
                if let Some(last)=out.last_mut().filter(|v|v.starts_with("text:")) {last.push_str(&t);continue;}
                format!("text:{t}")
            }
            Event::Code(t)=>format!("code:{t}"),
            Event::Start(Tag::Heading{..})=>"heading".into(),
            Event::End(TagEnd::Heading(_))=>"/heading".into(),
            Event::Start(Tag::Paragraph)=>"paragraph".into(),
            Event::End(TagEnd::Paragraph)=>"/paragraph".into(),
            Event::Start(Tag::List(None))=>"list".into(),
            Event::End(TagEnd::List(false))=>"/list".into(),
            Event::Start(Tag::Item)=>"item".into(),
            Event::End(TagEnd::Item)=>"/item".into(),
            Event::HardBreak=>"break".into(),
            other=>return Err(format!("unexpected CommonMark event {other:?}")),
        };
        out.push(v);
    }
    Ok(out)
}

#[test]
fn independent_sentence_boundary_event_matrix() -> Result<(),String> {
    let compiled=compiled()?;
    let cases=[
        (r#"cons "第一。" cons "第二。""#,vec!["text:第一。第二。"]),
        (r#"cons "One. " cons "  Two.""#,vec!["text:One.   Two."]),
        ("cons \"A\" cons \" \" cons \"\u{a0}\" cons \"B\"",vec!["text:A \u{a0}B"]),
        (r#"cons sentence cons code "` x `" nil cons " / " cons sentence cons code "  " nil cons "end""#,vec!["code:` x `","text: / ","code:  ","text:end"]),
        (r##"cons "before" cons sentence cons text "[x]" cons break cons text "#not-heading" nil cons "after""##,vec!["text:before[x]","break","text:#not-headingafter"]),
        (r#"cons sentence cons code "left" nil cons " " cons sentence cons code "right" nil"#,vec!["code:left","text: ","code:right"]),
    ];
    for (items,expected) in cases {
        for list in [false,true] {
            let block=if list {format!("list unordered cons item none body cons paragraph {items} nil nil nil")} else {format!("paragraph {items} nil")};
            let source=format!("article en \"Title\" body cons {block} nil");
            let output=from_source(&compiled,&source)?;
            let mut wanted=vec!["heading","text:Title","/heading"];
            wanted.extend(if list {vec!["list","item"]} else {vec!["paragraph"]});
            wanted.extend(expected.clone());
            wanted.extend(if list {vec!["/item","/list"]} else {vec!["/paragraph"]});
            assert_eq!(events(&output)?,wanted,"{source}\n{output}");
            assert_eq!(output,from_source(&compiled,&source)?);
            println!("source={:?} output={:?} events={:?}",Digest::of(source.as_bytes()),output,events(&output)?);
        }
    }
    for items in [
        "",r#"cons "a" cons "" cons "b""#,
        r#"cons " " cons "b""#,r#"cons "a" cons " ""#,
        r#"cons sentence cons code "a" nil cons sentence cons code "b" nil"#,
        r#"cons sentence cons code "a" nil cons sentence cons text "" nil cons sentence cons code "b" nil"#,
        r#"cons "a" cons sentence cons break cons text "b" nil"#,
        r#"cons sentence cons text "a" cons break nil cons "b""#,
        r#"cons "a" cons sentence cons text "b " cons break cons text "c" nil"#,
        r#"cons "a" cons sentence cons text "b" cons break cons text " c" nil"#,
        r#"cons "a" cons paragraph cons "b" nil"#,
        r#"cons "a" cons parallel cons variant en "b" nil"#,
    ] {
        let source=format!("article en \"T\" body cons paragraph {items} nil nil");
        assert!(from_source(&compiled,&source).is_err(),"{source}");
    }
    Ok(())
}

#[test]
fn independent_typed_projection_stops_are_atomic() -> Result<(),String> {
    let compiled=compiled()?;
    let source=r#"article en "T" body cons paragraph cons "First. " cons sentence cons code " x " nil cons " Last." nil nil"#;
    with_input_route(true,&compiled,source,"Article",|tree,profile,b,a|{
        let syntax=tree.tree().bundle.validate_with_sources(profile.registry(),b,a).map_err(err)?;
        let store=SourceStore::default();
        let mut admission=SourceAdmission::default();
        let mut codec=FoundationCodec::new(profile.registry(),&store,&mut admission).map_err(err)?;
        let doc=lower::document(&syntax,&compiled.doc.package.schema,Category::Article,profile.registry(),&mut budget(),&mut codec).map_err(err)?;
        let original=doc.clone();
        for reason in [StopReason::WorkLimit,StopReason::SourceLimit,StopReason::DepthLimit,StopReason::NodeLimit,StopReason::AllocationLimit,StopReason::OutputLimit,StopReason::Cancelled] {
            let mut limits=budget().limits();
            match reason {StopReason::WorkLimit=>limits.work=0,StopReason::SourceLimit=>limits.source_bytes=0,StopReason::DepthLimit=>limits.depth=0,StopReason::NodeLimit=>limits.nodes=0,StopReason::AllocationLimit=>limits.allocation_units=0,StopReason::OutputLimit=>limits.output_bytes=0,_=>()}
            let mut limited=Budget::new(limits);
            if reason==StopReason::Cancelled {limited.cancel();}
            let mut admission=SourceAdmission::default();
            let mut codec=FoundationCodec::new(profile.registry(),&store,&mut admission).map_err(err)?;
            let result=markdown(&doc,profile.registry(),&mut codec,&mut limited);
            assert!(matches!(result,Err(Error::Stopped(s)) if s==reason),"{reason:?}: {result:?}");
            assert_eq!(limited.poll(),Err(reason));
            assert_eq!(doc,original);
        }
        let mut fresh=SourceAdmission::default();
        let mut codec=FoundationCodec::new(profile.registry(),&store,&mut fresh).map_err(err)?;
        let mut full=budget();
        let output=markdown(&doc,profile.registry(),&mut codec,&mut full).map_err(err)?;
        assert_eq!(events(&output)?,["heading","text:T","/heading","paragraph","text:First. ","code: x ","text: Last.","/paragraph"]);
        assert!(full.usage().output_bytes>=output.len() as u64);
        Ok(())
    })
}
