
#[test]
fn reviewer_before_after_actual_api() -> TestResult {
 for (name,input,options) in [
  ("simple", "let x y".to_string(),Scenario::default()),
  ("prefix", "let x let y z".to_string(),Scenario::default()),
  ("missing", "let x".to_string(),Scenario::default()),
  ("unknown", "mystery".to_string(),Scenario{unknown:true,..Scenario::default()}),
 ] {
  let reply=run_scenario(&input,true,options)?;
  eprintln!("REVIEW_OUTCOME {name} {:?}",reply.outcome);
  eprintln!("REVIEW_USAGE {name} {:?}",reply.usage);
 }
 Ok(())
}
