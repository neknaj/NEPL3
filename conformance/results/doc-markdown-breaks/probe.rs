
use super::*;
use nepl3_tools::doc::projection::{self,Error};
use pulldown_cmark::{Event,Parser,Tag};
#[test]
fn independent_breaks_keep_content_and_list_ownership()->Result<(),String>{
 let c=compiled()?;
 for (lk,l,rk,right) in [("text","x\\","text","# h"),("code","``","code","`"),("code"," ","text","- item"),("text","<a>&","code"," both "),("text","\u{65e5}","text","\u{672c}")] {
  let leftq=serde_json::to_string(l).map_err(err)?;let rightq=serde_json::to_string(right).map_err(err)?;
  let sentence=format!("sentence cons {lk} {leftq} cons break cons {rk} {rightq} nil");
  for list in [false,true]{
   let body=if list{format!("cons list unordered cons item none body cons paragraph cons {sentence} nil nil cons item none body cons paragraph cons \"tail\" nil nil nil nil")}else{format!("cons paragraph cons {sentence} nil cons paragraph cons \"tail\" nil nil")};
   let source=format!("article en \"T\" body {body}");let output=projection::from_source(&c,&source)?;
   let mut text=String::new();let mut codes=vec![];let(mut breaks,mut lists,mut items,mut heads,mut paragraphs)=(0,0,0,0,0);
   for e in Parser::new(&output){match e{Event::Text(t)=>text.push_str(&t),Event::Code(t)=>codes.push(t.into_string()),Event::HardBreak=>breaks+=1,Event::SoftBreak=>return Err(format!("unexpected soft break: {output}")),Event::Start(Tag::List(_))=>lists+=1,Event::Start(Tag::Item)=>items+=1,Event::Start(Tag::Heading{..})=>heads+=1,Event::Start(Tag::Paragraph)=>paragraphs+=1,_=>{}}}
   let expected=format!("T{}{}tail",if lk=="text"{l}else{""},if rk=="text"{right}else{""});let mut expected_codes=vec![];if lk=="code"{expected_codes.push(l);}if rk=="code"{expected_codes.push(right);}
   assert_eq!(text,expected,"{output}");assert_eq!(codes,expected_codes,"{output}");assert_eq!((breaks,lists,items,heads,paragraphs),if list{(1,1,2,1,0)}else{(1,0,0,1,2)},"{output}");
   println!("BREAK_CASE list={list} left={l:?} right={right:?} md={output:?}");
  }
 }
 let output=projection::from_source(&c,"article en \"T\" body cons paragraph cons sentence cons text \"a\" cons break cons text \"b\" cons break cons text \"c\" nil nil nil")?;
 assert_eq!(Parser::new(&output).filter(|e|matches!(e,Event::HardBreak)).count(),2);
 Ok(())
}
#[test]
fn independent_break_refusals_are_projection_failures()->Result<(),String>{
 let c=compiled()?;
 for content in ["cons break cons text \"a\"","cons text \"a\" cons break","cons text \"a\" cons break cons break cons text \"b\"","cons text \"a \" cons break cons text \"b\"","cons text \"a\" cons break cons text \" b\"","cons text \"a\" cons break cons text \"\u{a0}b\"","cons text \"a\u{3000}\" cons break cons text \"b\"","cons text \"a\\nb\""]{
  let source=format!("article en \"T\" body cons paragraph cons sentence {content} nil nil nil");
  let result=projection::from_source(&c,&source);assert!(result.is_err_and(|s|s.starts_with("Unsupported")||s.starts_with("Text")),"{source}");
 }
 for source in ["article en sentence cons text \"a\" cons break cons text \"b\" nil body nil","article en \"T\" body cons section s sentence cons text \"a\" cons break cons text \"b\" nil body nil nil"]{
  assert!(projection::from_source(&c,source).is_err_and(|s|s.starts_with("Unsupported")),"{source}");
 }
 Ok(())
}
#[test]
fn independent_break_output_budget_includes_escape_and_newline()->Result<(),String>{
 use nepl3_doc_core::{model::*,prepare};let c=compiled()?;let r=&c.doc.registry;
 let doc=DocumentSyntax{value:DocValue{root:DocRoot::Article(ArticleRef(0)),nodes:vec![DocKind::Article{language:"en".into(),title:SentenceRef(1),body:BodyRef(3)},DocKind::Sentence{inlines:vec![InlineRef(2)]},DocKind::Text{text:"T".into()},DocKind::Body{blocks:vec![BlockRef(4)]},DocKind::Paragraph{items:vec![FlowRef(5)]},DocKind::Sentence{inlines:vec![InlineRef(6),InlineRef(7),InlineRef(8)]},DocKind::Text{text:"a".into()},DocKind::Break,DocKind::Text{text:"b".into()}].into_iter().map(|kind|DocNode{kind,locations:vec![],span:None,origin:None}).collect(),embeds:vec![]},sources:vec![],origins:vec![],views:vec![],source_maps:vec![]};let original=doc.clone();
 let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;let mut full=budget();let output=projection::markdown(&doc,r,&mut codec,&mut full).map_err(err)?;assert_eq!(output,"# T\n\na\\\nb\n\n");let mut prep=budget();prepare::inspect(&doc,r,&mut codec,&mut prep).map_err(err)?;assert_eq!(full.usage().output_bytes,prep.usage().output_bytes+output.len() as u64);
 for resource in [Resource::Work,Resource::AllocationUnits,Resource::OutputBytes]{let mut l=budget().limits();match resource{Resource::Work=>l.work=full.usage().work-1,Resource::AllocationUnits=>l.allocation_units=full.usage().allocation_units-1,Resource::OutputBytes=>l.output_bytes=full.usage().output_bytes-1,_=>{}}let mut b=Budget::new(l);assert!(matches!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(_))));assert!(matches!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(_))));}
 let mut l=budget().limits();l.depth=0;let mut b=Budget::new(l);assert!(matches!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(StopReason::DepthLimit))));assert_eq!(doc,original);Ok(())
}
