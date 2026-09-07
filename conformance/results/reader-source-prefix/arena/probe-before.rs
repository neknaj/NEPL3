
#[test]
fn review_nested_generated_prefix_keeps_every_source_and_position() -> TestResult {
 for parents in [1usize,3,6] {
  let input=format!("{}\"y\"", "let \"x\\n\" ".repeat(parents));
  let mut replies=vec![];
  for native in [None,Some(host::Action::Serve)] {
   let reply=run_scenario(&input,true,Scenario{text:true,provider:true,native,..Scenario::default()})?;
   let ParseOutcome::Complete{tree,cursor,..}=&reply.outcome else{return Err(format!("{reply:?}").into());};
   assert_eq!(*cursor,input.len() as u64);assert_eq!(tree.bundle.nodes.len(),parents*2+1);
   assert_eq!(reply.sources.len(),parents+1);assert_eq!(reply.sources.iter().filter(|s|s.text()=="x\n").count(),parents);assert_eq!(reply.sources.iter().filter(|s|s.text()=="y").count(),1);
   assert_eq!(reply.report.usage.source_bytes,(input.len()+parents*2+1) as u64);
   for token in &tree.bundle.tokens { assert_eq!(token.head.snapshot_ref().source.0,"input"); }
   replies.push(reply);
  }
  assert_eq!(replies[0].outcome,replies[1].outcome);assert_eq!(replies[0].sources,replies[1].sources);assert_eq!(replies[0].source_maps,replies[1].source_maps);
  let full=replies[1].report.usage.work;
  for work in [0,full/4,full/2,full-1,full] {
   let reply=run_scenario(&input,true,Scenario{text:true,provider:true,native:Some(host::Action::Serve),work:Some(work),..Scenario::default()})?;
   match reply.outcome {ParseOutcome::Complete{..}=>assert_eq!(reply.sources.len(),parents+1),ParseOutcome::Stopped{reason,..}=>assert_eq!(reason,nepl3_core::budget::StopReason::WorkLimit),other=>return Err(format!("unexpected {other:?}").into())};
   assert!(reply.report.usage.work<=work);
  }
 }
 Ok(())
}
