use super::*;
#[test]
fn nested_callback_stops_keep_reason_and_accepted_prefix()->TestResult{
 for action in [host::Action::NestedReaderStop,host::Action::NestedSourceStop]{
  let reply=run_scenario("let x y",true,Scenario{provider:true,native:Some(action),caller_depth:7,..Scenario::default()})?;
  println!("nested action {action:?}: outcome={:?}, accepted diagnostics={}",reply.outcome,reply.report.diagnostics.len());
  assert!(matches!(reply.outcome,ParseOutcome::Stopped{reason:nepl3_core::budget::StopReason::Cancelled,..}));
  assert_eq!(reply.report.diagnostics.len(),1);
 }
 Ok(())
}
