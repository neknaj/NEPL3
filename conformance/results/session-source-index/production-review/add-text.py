from pathlib import Path
r=Path(__file__).parent
test=r'''
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
'''
for name in ['workspace','before']:
 with (r/name/'crates/foundation/engine/tests/parse.rs').open('a',encoding='utf-8',newline='\n')as f:f.write(test)
(r/'generated-probe.rs').write_text(test,encoding='utf-8')
