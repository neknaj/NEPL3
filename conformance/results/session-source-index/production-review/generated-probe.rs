
#[test]
fn reviewer_generated_source_cost() -> TestResult {
 for count in [6,24] {
  let input=format!("{}y", "let \"x\\n\" ".repeat(count));
  for action in [host::Action::Serve,host::Action::DeclineSecond,host::Action::FailSecond] {
   let reply=run_scenario(&input,true,Scenario{provider:true,text:true,native:Some(action),..Scenario::default()})?;
   assert!(matches!(reply.outcome,ParseOutcome::Complete{..}));assert!(reply.sources.len()>=count);
   eprintln!("REVIEW_TEXT_OUTCOME {count} {action:?} {:?}",reply.outcome);
   eprintln!("REVIEW_TEXT_SOURCES {count} {action:?} {:?}",reply.sources);
   eprintln!("REVIEW_TEXT_MAPS {count} {action:?} {:?}",reply.source_maps);
   eprintln!("REVIEW_TEXT_USAGE {count} {action:?} {:?}",reply.report.usage);
  }
 }
 Ok(())
}
