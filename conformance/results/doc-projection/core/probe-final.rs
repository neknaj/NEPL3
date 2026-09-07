use super::*;
use nepl3_tools::doc::projection::{self,Error};
use pulldown_cmark::{Event,Tag};
#[test]
fn independent_projection_adjacent_code_preserves_both_values_or_rejects()->Result<(),String>{
 let c=compiled()?;
 for (left,right) in [("a","b"),("`a`","b"),("a","``b")]{
  let input=format!("article en \"T\" body cons paragraph cons sentence cons code \"{left}\" cons code \"{right}\" nil nil nil");
  match projection::from_source(&c,&input){
   Ok(md)=>{let codes=pulldown_cmark::Parser::new(&md).filter_map(|e|if let Event::Code(s)=e{Some(s.into_string())}else{None}).collect::<Vec<_>>();println!("input={input:?} markdown={md:?} codes={codes:?}");assert_eq!(codes,[left,right]);},
   Err(e)=>println!("explicit rejection: {e}"),
  }
 }
 Ok(())
}
#[test]
fn independent_projection_list_boundary_across_section_must_not_merge()->Result<(),String>{
 let c=compiled()?;let list=|text:&str|format!("list unordered cons item none body cons paragraph cons \"{text}\" nil nil nil");
 let input=format!("article en \"T\" body cons section s \"S\" body cons {} nil cons {} nil",list("a"),list("b"));
 match projection::from_source(&c,&input){Ok(md)=>{let lists=pulldown_cmark::Parser::new(&md).filter(|e|matches!(e,Event::Start(Tag::List(_)))).count();println!("input={input:?} markdown={md:?} lists={lists}");assert_eq!(lists,2);},Err(e)=>println!("explicit rejection: {e}")}
 Ok(())
}
#[test]
fn independent_projection_charges_generated_markdown_output()->Result<(),String>{
 use nepl3_doc_core::{model::*,prepare};
 let c=compiled()?;let r=&c.doc.registry;
 let doc=DocumentSyntax{value:DocValue{root:DocRoot::Article(ArticleRef(0)),nodes:vec![
 DocKind::Article{language:"en".into(),title:SentenceRef(1),body:BodyRef(3)},
 DocKind::Sentence{inlines:vec![InlineRef(2)]},DocKind::Text{text:"T".into()},
 DocKind::Body{blocks:vec![BlockRef(4)]},DocKind::Paragraph{items:vec![FlowRef(5)]},
 DocKind::Sentence{inlines:vec![InlineRef(6)]},DocKind::Text{text:"a!?&".into()},
 ].into_iter().map(|kind|DocNode{kind,locations:vec![],span:None,origin:None}).collect(),embeds:vec![]},sources:vec![],origins:vec![],views:vec![],source_maps:vec![]};
 let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;let mut baseline=budget();prepare::inspect(&doc,r,&mut codec,&mut baseline).map_err(err)?;
 let mut limits=budget().limits();limits.output_bytes=baseline.usage().output_bytes;let mut b=Budget::new(limits);
 let value=projection::markdown(&doc,r,&mut codec,&mut b);println!("inspect output={} projection={value:?} used={}",limits.output_bytes,b.usage().output_bytes);
 assert!(matches!(value,Err(Error::Stopped(StopReason::OutputLimit))));Ok(())
}

#[test]
fn independent_projection_code_whitespace_and_markers_remain_literal()->Result<(),String>{
 let c=compiled()?;
 for value in ["a","`","``","a`b","`a`"," a","a "," a "," ","   ","***","<&amp;>","\\","\u{65e5}\u{672c}"] {
  let value=value.replace("\\u{65e5}","\u{65e5}").replace("\\u{672c}","\u{672c}");
  let quoted=serde_json::to_string(&value).map_err(err)?;
  let input=format!("article en \"T\" body cons paragraph cons sentence cons code {quoted} nil nil nil");
  let md=projection::from_source(&c,&input)?;
  let codes=pulldown_cmark::Parser::new(&md).filter_map(|e|if let Event::Code(s)=e{Some(s.into_string())}else{None}).collect::<Vec<_>>();
  assert_eq!(codes,[value]);
 }
 for input in [
  "article en \"T\" body nil",
  "article en \"T\" body cons section a \"A\" body nil cons section b \"B\" body nil nil",
  "article en \"T\" body cons paragraph cons \"a\" nil cons paragraph cons \"b\" nil nil",
 ] { assert!(projection::from_source(&c,input).is_ok(),"{input}"); }
 Ok(())
}
