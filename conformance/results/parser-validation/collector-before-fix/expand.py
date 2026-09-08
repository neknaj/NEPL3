from pathlib import Path
r=Path(__file__).parent/'workspace'
p=r/'crates/foundation/engine/tests/parse/host.rs'
s=p.read_text(encoding='utf-8').replace('    ResetCancelSecond,','    ResetCancelSecond,\n    ResetErrorSecond,\n    ResetCancelErrorSecond,')
s=s.replace('Action::ResetSecond | Action::ResetCancelSecond => {','Action::ResetSecond | Action::ResetCancelSecond | Action::ResetErrorSecond | Action::ResetCancelErrorSecond => {')
s=s.replace('if self.action == Action::ResetCancelSecond { budget.cancel(); }','if matches!(self.action, Action::ResetCancelSecond | Action::ResetCancelErrorSecond) { budget.cancel(); }\n                    if matches!(self.action, Action::ResetErrorSecond | Action::ResetCancelErrorSecond) { return Err(ParseError::Context); }')
p.write_text(s,encoding='utf-8',newline='\n')
p=r/'crates/foundation/engine/tests/parse.rs'
s=p.read_text(encoding='utf-8').replace('for action in [host::Action::ResetSecond, host::Action::ResetCancelSecond] {','let mut unsafe_results=0;\n    for action in [host::Action::ResetSecond, host::Action::ResetCancelSecond, host::Action::ResetErrorSecond, host::Action::ResetCancelErrorSecond] {')
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
