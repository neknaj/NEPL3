from pathlib import Path
r=Path(__file__).resolve().parent
for name in ['source','before']:
 p=r/name/'probe/tests/head.rs';s=p.read_text(encoding='utf-8')
 s=s.replace('return Ok(result);','return Ok((result,trace));').replace('.map_err(|v| format!("overflow stop {v:?}"));','.map(|v|(v,trace)).map_err(|v| format!("overflow stop {v:?}"));')
 while '.map(|v|(v,trace)).map(|v|(v,trace))' in s:s=s.replace('.map(|v|(v,trace)).map(|v|(v,trace))','.map(|v|(v,trace))')
 s=s[:s.index('\nfn call_tag(')]+'\n'+(r/'extra.rs').read_text(encoding='utf-8')
 s=s.replace('    if case == Case::Portable {\n        portable::tree_roundtrip', '    if case == Case::Portable && input.starts_with("choose") {\n        portable::tree_roundtrip')
 if '    IndependentList,' not in s:s=s.replace('    Owned,','    Owned,\n    IndependentList,',1)
 if 'if case==Case::IndependentList {' not in s:s=s.replace('    for i in 0..forms {','    if case==Case::IndependentList { package.reads.push(ReadSpec::ListOf { element: ReadSpecId(1), cons: kind("List:LocalCons")?, nil:kind("List:Nil")? }); package.forms[0].fields[1].read=ReadSpecId(3); }\n    for i in 0..forms {',1)
 if 'assert_sticky(&result,&mut b);' not in s:s=s.replace('return Ok((result,trace));','assert_sticky(&result,&mut b); return Ok((result,trace));')
 if 'assert_sticky(&reply,&mut b);\n    let tree' not in s:s=s.replace('    let tree = match &reply.outcome {','    assert_sticky(&reply,&mut b);\n    let tree = match &reply.outcome {')
 p.write_text(s,encoding='utf-8',newline='\n')
