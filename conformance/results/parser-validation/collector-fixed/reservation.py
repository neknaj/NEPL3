from pathlib import Path
r=Path(__file__).parent/'workspace'
p=r/'crates/foundation/engine/tests/parse/host.rs'
s=p.read_text(encoding='utf-8').replace('    Serve,','    Serve,\n    ReviewReservation { cancel: bool, fail: bool },')
s=s.replace('                Action::ResetSecond { cancel, fail } => {','''                Action::ReviewResetSecond | Action::ReviewResetCancelSecond | Action::ReviewResetErrorSecond | Action::ReviewResetCancelErrorSecond => {
                    *budget = Budget::new(budget.limits());
                    if matches!(self.action, Action::ReviewResetCancelSecond | Action::ReviewResetCancelErrorSecond) { budget.cancel(); }
                    if matches!(self.action, Action::ReviewResetErrorSecond | Action::ReviewResetCancelErrorSecond) { return Err(ParseError::Context); }
                    return Ok(None);
                }
                Action::ResetSecond { cancel, fail } => {''')
s=s.replace('Action::Serve | Action::ResetReservation { .. } => {}','Action::Serve | Action::ResetReservation { .. } | Action::ReviewReservation {..} => {}')
s=s.replace('        if let Action::ResetReservation { cancel, fail } = self.action {','''        if let Action::ReviewReservation { cancel, fail } = self.action {
            *budget = Budget::new(budget.limits());
            if cancel { budget.cancel(); }
            return if fail { Err(ParseError::Context) } else { Ok(None) };
        }
        if let Action::ResetReservation { cancel, fail } = self.action {''')
p.write_text(s,encoding='utf-8',newline='\n')
p=r/'crates/foundation/engine/tests/parse.rs'
s=p.read_text(encoding='utf-8')+'''
#[test]
fn reviewer_reservation_budget_replacement_is_fatal() -> TestResult {
    for cancel in [false,true] {
        for fail in [false,true] {
            let result=run_scenario("let \\\"x\\\\n\\\" y",true,Scenario {
                provider:true,text:true,native:Some(host::Action::ReviewReservation{cancel,fail}),..Scenario::default()
            });
            eprintln!("reservation cancel={cancel} fail={fail}: {result:?}");
            let error=result.expect_err("budget integrity cannot become normal result");
            assert!(error.contains("BrokenCollector"),"{error}");
            if cancel { assert!(error.contains("Cancelled"),"{error}"); }
        }
    }
    Ok(())
}
'''
p.write_text(s,encoding='utf-8',newline='\n')
