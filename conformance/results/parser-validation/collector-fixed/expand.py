from pathlib import Path
r=Path(__file__).parent/'workspace'
p=r/'crates/foundation/engine/tests/parse/host.rs'
s=p.read_text(encoding='utf-8').replace('    ReviewResetCancelSecond,','    ReviewResetCancelSecond,\n    ReviewResetErrorSecond,\n    ReviewResetCancelErrorSecond,')
s=s.replace('Action::ReviewResetSecond | Action::ReviewResetCancelSecond => {','Action::ReviewResetSecond | Action::ReviewResetCancelSecond | Action::ReviewResetErrorSecond | Action::ReviewResetCancelErrorSecond => {')
s=s.replace('if self.action == Action::ReviewResetCancelSecond { budget.cancel(); }','if matches!(self.action, Action::ReviewResetCancelSecond | Action::ReviewResetCancelErrorSecond) { budget.cancel(); }\n                    if matches!(self.action, Action::ReviewResetErrorSecond | Action::ReviewResetCancelErrorSecond) { return Err(ParseError::Context); }')
p.write_text(s,encoding='utf-8',newline='\n')
p=r/'crates/foundation/engine/tests/parse.rs'
s=p.read_text(encoding='utf-8').replace('for action in [host::Action::ReviewResetSecond, host::Action::ReviewResetCancelSecond] {','let mut unsafe_results=0;\n    for action in [host::Action::ReviewResetSecond, host::Action::ReviewResetCancelSecond, host::Action::ReviewResetErrorSecond, host::Action::ReviewResetCancelErrorSecond] {')
s=s.replace('assert!(result.is_err(), "budget rollback must be fatal");','if result.is_ok() { unsafe_results+=1; }')
s=s.replace('fn reviewer_long_owned_native_semantics()', 'fn reviewer_long_owned_native_semantics()')
needle='''    Ok(())
}
#[test]
fn reviewer_long_owned_native_semantics'''
s=s.replace(needle,'''    assert_eq!(unsafe_results,0,"budget rollback must be fatal");
    Ok(())
}
#[test]
fn reviewer_long_owned_native_semantics''')
p.write_text(s,encoding='utf-8',newline='\n')
