fn call_tag(call: &HeadCall) -> String {
    let kind=match &call.request { HeadRequest::Shape => "shape".into(), HeadRequest::ChildContext{index,..}=>format!("child{index}") };
    format!("{kind}:{}",String::from_utf8_lossy(&call.head.window.bytes))
}
fn assert_sticky(reply:&ParseReply,b:&mut nepl3_core::budget::Budget) {
 if let ParseOutcome::Stopped{reason,..}=&reply.outcome { let old=b.usage(); assert_eq!(b.charge(nepl3_core::budget::Resource::Work,0),Err(*reason));assert_eq!(b.usage(),old); }
}
fn tree_tag(reply:&ParseReply)->String {
 match &reply.outcome {
 ParseOutcome::Complete{tree,cursor,..}|ParseOutcome::Recovered{tree,cursor,..}=>format!("{cursor}|{tree:?}"),
 other=>format!("{other:?}")
 }
}
#[test]
fn independent_dispatch_matrix()->Result<(),String>{
 for forms in [0,8,64] {
  for (input,expected) in [
   ("let n x trailing",vec!["shape:x"]),
   ("let let x trailing",vec!["shape:x"]),
   ("x trailing",vec!["shape:x"]),
   ("let n let z x trailing",vec!["shape:x"]),
   ("choose alt @let z x trailing",vec!["shape:choose","child0:choose","child1:choose","shape:x"]),
  ] {
   let (owned,trace)=observed(input,Case::Owned,true,forms,false,false,None)?;
   assert_eq!(trace,expected,"{input}");
   let tag=tree_tag(&owned);
   for case in [Case::Native,Case::NativeFallback,Case::Portable,Case::Sealed] {
    let (reply,other)=observed(input,case,false,forms,false,false,None)?;
    let mut expected=expected.clone();
    if case==Case::NativeFallback && input.starts_with("choose") { expected.insert(1,"child0:choose"); }
    assert_eq!(other,expected,"{input}/{case:?}");assert_eq!(tree_tag(&reply),tag,"{input}/{case:?}");
   }
   let (simple,trace)=observed(input,Case::Owned,false,forms,false,false,None)?;
   println!("CASE|{forms}|{input}|{:?}|{:?}|{}",simple.report.usage,trace,tree_tag(&simple));
  }
 }
 for forms in [0,8,64] {
  for input in ["x trailing","let n x trailing","let n let z x trailing"] {
   for no_leaf in [false,true] {
    let (reply,trace)=observed(input,Case::Owned,false,forms,true,no_leaf,None)?;
    assert!(trace.is_empty());println!("NOHEAD|{forms}|{no_leaf}|{input}|{:?}|{}",reply.report.usage,tree_tag(&reply));
   }
  }
 }
 // With no matching leaf, a valid Shape(None) must resume to explicit unknown,
 // not reissue Shape for the same head or invent a zero-arity known leaf.
 for case in [Case::Owned,Case::Native,Case::Portable] {
  let (reply,trace)=observed("mystery trailing",case,true,8,false,true,None)?;
  assert_eq!(trace,["shape:mystery"]);assert!(matches!(reply.outcome,ParseOutcome::Recovered{..}));
  println!("UNKNOWN|{case:?}|{:?}|{}",reply.report.usage,tree_tag(&reply));
 }
 for forms in [0,8,64] {
  for (input,expected,recovered) in [("let let cons x cons y nil trailing",vec!["shape:x","shape:y"],false),("let n nil trailing",vec![],false),("let n bogus trailing",vec![],true)] {
   let (reply,trace)=observed(input,Case::IndependentList,false,forms,false,false,None)?;
   assert_eq!(trace,expected);assert_eq!(matches!(reply.outcome,ParseOutcome::Recovered{..}),recovered);
   println!("LIST|{forms}|{input}|{:?}|{:?}|{}",reply.report.usage,trace,tree_tag(&reply));
  }
 }
 let (reply,trace)=observed("choose alt @let z x trailing",Case::Cancel,false,8,false,false,None)?;
 assert!(matches!(reply.outcome,ParseOutcome::Stopped{reason:nepl3_core::budget::StopReason::Cancelled,..}));
 println!("CANCEL|{:?}|{:?}",reply.report.usage,trace);
 Ok(())
}
#[test]
fn independent_work_boundaries()->Result<(),String>{
 for input in ["let n x trailing","x trailing","choose alt @let z x trailing"] {
  for cap in [0,1,10,100,1000,3000,10000,30000] {
   let result=observed(input,Case::Owned,false,8,false,false,Some(cap));
   match result {
    Ok((reply,trace))=>{assert!(reply.report.usage.work<=cap);println!("STOP|{input}|{cap}|{:?}|{:?}|{}",reply.report.usage,trace,tree_tag(&reply));},
    Err(error)=>{assert!(error.contains("WorkLimit"),"{error}");println!("STOPERR|{input}|{cap}|{error}");}
   }
  }
 }
 Ok(())
}
