
use super::*;
use nepl3_tools::doc::projection::{self,Error};
use nepl3_doc_core::{model::*,prepare};
use pulldown_cmark::{Event,Parser,Tag,TagEnd,CodeBlockKind};
fn document(hint:Option<&str>,text:&str)->DocumentSyntax{
 DocumentSyntax{value:DocValue{root:DocRoot::Article(ArticleRef(0)),nodes:vec![DocKind::Article{language:"en".into(),title:SentenceRef(1),body:BodyRef(3)},DocKind::Sentence{inlines:vec![InlineRef(2)]},DocKind::Text{text:"T".into()},DocKind::Body{blocks:vec![BlockRef(4)]},DocKind::RawCode{language_hint:hint.map(str::to_string),text:text.into()}].into_iter().map(|kind|DocNode{kind,locations:vec![],span:None,origin:None}).collect(),embeds:vec![]},sources:vec![],origins:vec![],views:vec![],source_maps:vec![]}
}
#[test]
fn independent_raw_code_bytes_and_container_boundaries()->Result<(),String>{
 let c=compiled()?;
 for (hint,text) in [(None,""),(Some("Rust"),"\n"),(Some("c++"),"\n\n"),(Some("x.y_z-1"),"\t \n \t\n"),(None,"# h\n- item\n> quote\n<script>&amp;\n"),(Some("Rust"),"```\n`````    \n~~~\n"),(Some("-"),"\\\n`mid``text`\n"),(Some("0"),"\u{65e5}\u{672c}\n\u{2028}separator\n")]{
  let hint_source=match hint{None=>"none".to_string(),Some(s)=>format!("some {}",serde_json::to_string(s).map_err(err)?)};let quoted=serde_json::to_string(text).map_err(err)?;
  let list=|name:&str|format!("list unordered cons item none body cons paragraph cons \"{name}\" nil nil nil");
  let body=format!("body cons paragraph cons \"before\" nil cons {} cons rawcode {hint_source} {quoted} cons rawcode none \"\" cons {} cons paragraph cons \"after\" nil nil",list("L1"),list("L2"));
  for section in [false,true]{let source=if section{format!("article en \"T\" body cons section s \"S\" {body} nil")}else{format!("article en \"T\" {body}")};let md=projection::from_source(&c,&source)?;let mut blocks:Vec<(String,String)>=vec![];let mut active=false;let(mut lists,mut items,mut paragraphs,mut headings)=(0,0,0,0);let mut outside=String::new();
   for e in Parser::new(&md){match e{Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(h)))=>{blocks.push((h.into_string(),String::new()));active=true;},Event::Text(t) if active=>blocks.last_mut().ok_or("no block")?.1.push_str(&t),Event::End(TagEnd::CodeBlock)=>active=false,Event::Text(t)=>outside.push_str(&t),Event::Start(Tag::List(_))=>lists+=1,Event::Start(Tag::Item)=>items+=1,Event::Start(Tag::Paragraph)=>paragraphs+=1,Event::Start(Tag::Heading{..})=>headings+=1,Event::Html(_)|Event::InlineHtml(_)|Event::Start(Tag::CodeBlock(CodeBlockKind::Indented))=>return Err(format!("wrong block: {md}")),_=>{}}}
   assert_eq!(blocks,vec![(hint.unwrap_or("").to_string(),text.to_string()),(String::new(),String::new())]);assert_eq!((lists,items,paragraphs,headings),(2,2,2,if section{2}else{1}));assert_eq!(outside,if section{"TSbeforeL1L2after"}else{"TbeforeL1L2after"});
  }
 }
 assert!(projection::from_source(&c,"article en \"T\" body cons list unordered cons item none body cons rawcode none \"x\\n\" nil nil nil").is_err_and(|e|e.starts_with("Unsupported")));
 Ok(())
}
#[test]
fn independent_raw_code_rejects_byte_normalization_and_info_mutation()->Result<(),String>{
 let c=compiled()?;let r=&c.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
 for text in ["x","\r","x\r\n","x\rmore\n","\0\n","\u{b}\n","\u{c}\n","\u{7f}\n","\u{85}\n"]{assert!(matches!(projection::markdown(&document(None,text),r,&mut codec,&mut budget()),Err(Error::Text{..})),"{text:?}");}
 for hint in [""," rust","rust ","two words","a\tb","a\nb","a`b","a~b","{.rust}","&amp;","a\\b","\u{65e5}"]{assert!(matches!(projection::markdown(&document(Some(hint),"x\n"),r,&mut codec,&mut budget()),Err(Error::Text{..})),"{hint:?}");}
 Ok(())
}
#[test]
fn independent_raw_code_fence_budget_and_input_are_preserved()->Result<(),String>{
 let c=compiled()?;let r=&c.doc.registry;let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;let text=format!("{}\n\tend\n\n","`".repeat(260));let doc=document(Some("r+1"),&text);let original=doc.clone();let mut full=budget();let value=projection::markdown(&doc,r,&mut codec,&mut full).map_err(err)?;let fence="`".repeat(261);assert_eq!(value,format!("# T\n\n{fence}r+1\n{text}{fence}\n\n"));let mut prep=budget();prepare::inspect(&doc,r,&mut codec,&mut prep).map_err(err)?;assert_eq!(full.usage().output_bytes,prep.usage().output_bytes+value.len() as u64);
 for resource in [Resource::Work,Resource::AllocationUnits,Resource::OutputBytes]{let mut l=budget().limits();match resource{Resource::Work=>l.work=full.usage().work-1,Resource::AllocationUnits=>l.allocation_units=full.usage().allocation_units-1,Resource::OutputBytes=>l.output_bytes=full.usage().output_bytes-1,_=>{}}let mut b=Budget::new(l);assert!(matches!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(_))));assert!(matches!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(_))));}
 let mut b=budget();b.cancel();assert_eq!(projection::markdown(&doc,r,&mut codec,&mut b),Err(Error::Stopped(StopReason::Cancelled)));assert_eq!(doc,original);Ok(())
}
