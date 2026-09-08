from pathlib import Path
r=Path(__file__).parent/'workspace'
p=r/'crates/foundation/engine/tests/parse/host.rs'
s=p.read_text(encoding='utf-8').replace('    Serve,','    Serve,\n    ReviewResetSecond,\n    ReviewResetCancelSecond,')
s=s.replace('                Action::Serve => {}','''                Action::ReviewResetSecond | Action::ReviewResetCancelSecond => {
                    *budget = Budget::new(budget.limits());
                    if self.action == Action::ReviewResetCancelSecond { budget.cancel(); }
                    return Ok(None);
                }
                Action::Serve => {}''')
p.write_text(s,encoding='utf-8',newline='\n')
p=r/'crates/foundation/engine/tests/parse.rs'
s=p.read_text(encoding='utf-8')+'''
#[test]
fn reviewer_usage_rollback_must_not_be_successful_stop() -> TestResult {
    for action in [host::Action::ReviewResetSecond, host::Action::ReviewResetCancelSecond] {
        let result = run_scenario("let x y", true, Scenario {
            provider: true, native: Some(action), ..Scenario::default()
        });
        eprintln!("rollback {action:?}: {result:?}");
        assert!(result.is_err(), "budget rollback must be fatal");
    }
    Ok(())
}
#[test]
fn reviewer_long_owned_native_semantics() -> TestResult {
    for count in [12, 24] {
        let input = format!("{}y", "let x ".repeat(count));
        let owned = run_scenario(&input, true, Scenario { provider: true, ..Scenario::default() })?;
        let native = run_scenario(&input, true, Scenario { provider: true, native:Some(host::Action::Serve), ..Scenario::default() })?;
        assert_eq!(owned.outcome, native.outcome);
        assert_eq!(owned.report.diagnostics, native.report.diagnostics);
        assert_eq!(owned.sources, native.sources);
        assert_eq!(owned.source_maps, native.source_maps);
        eprintln!("long {count}: owned {:?} native {:?}", owned.report.usage, native.report.usage);
    }
    Ok(())
}
'''
p.write_text(s,encoding='utf-8',newline='\n')
