use nepl3_core::{
    budget::{Budget, Limits, Resource},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_doc_core::{model::*, pages::*, portable};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 1000,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_doc_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn doc(target: LinkTarget) -> DocumentSyntax {
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(3),
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(2)],
        },
        DocKind::Text {
            text: "文書".into(),
        },
        DocKind::Body {
            blocks: vec![BlockRef(4)],
        },
        DocKind::Paragraph {
            items: vec![FlowRef(5)],
        },
        DocKind::Sentence {
            inlines: vec![InlineRef(6), InlineRef(7)],
        },
        DocKind::Anchor {
            id: "導入".into(),
            label: InlineRef(2),
        },
        DocKind::Link {
            target,
            label: InlineRef(2),
        },
    ];
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(0)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
fn set() -> PageSet {
    PageSet {
        pages: vec![
            PageDocument {
                registration: PageRegistration {
                    id: "first".into(),
                    source: "doc/first.nepld".into(),
                    route: "docs/first/index.html".into(),
                },
                document: doc(LinkTarget::Page {
                    page: "second".into(),
                    fragment: Some("導入".into()),
                }),
            },
            PageDocument {
                registration: PageRegistration {
                    id: "second".into(),
                    source: "doc/next/second.nepld".into(),
                    route: "docs/second/index.html".into(),
                },
                document: doc(LinkTarget::Relative {
                    path: ".././first.nepld".into(),
                    fragment: Some("導入".into()),
                }),
            },
        ],
    }
}
fn run(set: &PageSet) -> Result<PageLinkPlan, String> {
    let r = registry()?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    Ok(resolve(set, &r, &mut c, &mut b())
        .map_err(err)?
        .plan()
        .clone())
}

fn main()->Result<(),String>{use nepl3_core::budget::StopReason;let original=set();
for (path,expected)in [("next/second.nepld",true),("./next/second.nepld",true),("next/../next/second.nepld",true),("../doc/next/second.nepld",true),("next/second",false),("NEXT/second.nepld",false),("../../doc/next/second.nepld",false),("next//second.nepld",false),("next/second.nepld/",false),(".",false),("..",false),("next/..",false),("/doc/next/second.nepld",false),("next/%73econd.nepld",false)]{let mut s=original.clone();if let DocKind::Link{target,..}=&mut s.pages[0].document.value.nodes[7].kind{*target=LinkTarget::Relative{path:path.into(),fragment:None};}let result=run(&s);assert_eq!(result.is_ok(),expected,"{path}: {result:?}");if let Ok(p)=result{assert_eq!(p.links[0].target,1);}}
for field in [PageField::Id,PageField::Source,PageField::Route]{for reverse in [false,true]{let mut s=original.clone();let a=s.pages[0].registration.clone();let b=&mut s.pages[1].registration;match field{PageField::Id=>b.id=a.id,PageField::Source=>b.source=a.source,PageField::Route=>b.route=a.route};if reverse{s.pages.reverse();}assert!(run(&s).unwrap_err().contains("Collision"));}}
for field in [PageField::Source,PageField::Route]{for longer in [false,true]{let mut s=original.clone();let prefix=match field{PageField::Source=>s.pages[0].registration.source.clone(),_=>s.pages[0].registration.route.clone()};let value=if longer{prefix+"/child"}else{prefix.split('/').next().unwrap().into()};match field{PageField::Source=>s.pages[1].registration.source=value,_=>s.pages[1].registration.route=value};assert!(run(&s).unwrap_err().contains("Collision"));}}
let mut unicode=original.clone();unicode.pages[1].registration.source="doc/?? ??.nepld".into();if let DocKind::Link{target,..}=&mut unicode.pages[0].document.value.nodes[7].kind{*target=LinkTarget::Relative{path:"?? ??.nepld".into(),fragment:Some("??".into())};}if let DocKind::Link{target,..}=&mut unicode.pages[1].document.value.nodes[7].kind{*target=LinkTarget::Page{page:"first".into(),fragment:None};}assert_eq!(run(&unicode)?.links[0].target,1);
let mut wrong=original.clone();wrong.pages[1].document.value.nodes[6].kind=DocKind::Concat{inlines:vec![InlineRef(2)]};assert!(run(&wrong).unwrap_err().contains("MissingFragment"));
let r=registry()?;let store=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&store,&mut a).map_err(err)?;let mut normal=b();let expected=resolve(&original,&r,&mut c,&mut normal).map_err(err)?.plan().clone();let mut nested=b();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&store,&mut a).map_err(err)?;let p=nested.with_depth_at_least(7,|bb|resolve(&original,&r,&mut c,bb)).map_err(err)?;assert_eq!(p.plan(),&expected);assert_eq!(nested.current_depth(),0);assert_eq!(nested.usage().depth,normal.usage().depth+7);
let raw=portable::pages::plan_to_value(&expected,&original,&r,&mut c,&mut b()).map_err(err)?;let mut stops=0;let mut successes=0;
for phase in 0..2{let execute=|bb:&mut Budget|{let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&store,&mut a).unwrap();if phase==0{resolve(&original,&r,&mut c,bb).map(|p|p.plan().clone())}else{portable::pages::plan_from_value(&raw,&original,&r,&mut c,bb)}};let mut measured=b();assert_eq!(execute(&mut measured).map_err(err)?,expected);let u=measured.usage();
for (k,n,reason)in [(0,u.work,StopReason::WorkLimit),(1,u.allocation_units,StopReason::AllocationLimit),(2,u.nodes,StopReason::NodeLimit),(3,u.depth,StopReason::DepthLimit),(4,u.output_bytes,StopReason::OutputLimit)]{let mut caps=(0..=24).map(|i|n*i/24).collect::<Vec<_>>();caps.extend([n.saturating_sub(1),n,n+1]);caps.sort();caps.dedup();for cap in caps{let mut l=b().limits();match k{0=>l.work=cap,1=>l.allocation_units=cap,2=>l.nodes=cap,3=>l.depth=cap,_=>l.output_bytes=cap};let mut bb=Budget::new(l);match execute(&mut bb){Ok(p)=>{successes+=1;assert_eq!(p,expected)},Err(e)=>{stops+=1;assert!(matches!(e,PageError::Stopped(s)if s==reason),"phase={phase} resource={k} cap={cap} error={e:?}");assert_eq!(bb.poll(),Err(reason));}}}}
let mut cancelled=b();cancelled.cancel();assert!(matches!(execute(&mut cancelled),Err(PageError::Stopped(StopReason::Cancelled))));}
assert_eq!(original,set());println!("Page model: 14 relative cases; collision/prefix/Unicode/page-local fragment; caller7; resolve/replay caps {stops} stops/{successes} successes plus cancel");Ok(())}
