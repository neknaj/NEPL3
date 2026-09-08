from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-parser-form-selection'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
for name,rev in [('source','3f23aec'),('before','6f84a51')]:
 head=git('rev-parse',rev).decode().strip();dest=r/name;dest.mkdir(exist_ok=True);files=[]
 paths=git('ls-tree','-r','--name-only',head,'crates/foundation','Cargo.toml','Cargo.lock','rust-toolchain.toml').decode().splitlines()
 for path in paths:
  data=git('show',head+':'+path);files.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)));p=dest/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 p=dest/'Cargo.toml';p.write_text(re.sub(r'members = \[[^\n]+','members = ["crates/foundation/core", "crates/foundation/wire", "crates/foundation/reader", "crates/foundation/engine", "probe"]',p.read_text(encoding='utf-8')),encoding='utf-8',newline='\n')
 p=dest/'probe';(p/'tests/parse').mkdir(parents=True,exist_ok=True);(p/'tests/head').mkdir(exist_ok=True)
 (p/'Cargo.toml').write_text('[package]\nname="form-selection-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nnepl3-core={path="../crates/foundation/core"}\nnepl3-reader={path="../crates/foundation/reader"}\nnepl3-engine={path="../crates/foundation/engine"}\nnepl3-wire={path="../crates/foundation/wire"}\n',encoding='utf-8')
 for f in ['parse/support.rs','head/portable.rs']:(p/'tests'/f).write_bytes(git('show',head+':crates/foundation/engine/tests/'+f))
 s=git('show',head+':crates/foundation/engine/tests/head.rs').decode()
 start=s.index('fn run_case(');end=s.index('\nfn answer(',start)
 body=s[start:end]
 body=body.replace('fn run_case(input: &str, case: Case, exercise_rejections: bool) -> Result<ParseReply, String> {','fn observed(input: &str, case: Case, exercise_rejections: bool, forms: usize, no_head: bool, no_leaf: bool, cap: Option<u64>) -> Result<(ParseReply, Vec<String>), String> {')
 body=body.replace('    let identity = package','    for i in 0..forms { let mut f=package.forms[0].clone(); f.spelling=format!("unused{i}"); package.forms.insert(0,f); }\n    if no_leaf { package.leaves.clear(); }\n    let identity = package',1)
 body=body.replace('    if case == Case::Foreign {\n        profile.languages.push','    if no_head { profile.head_providers.clear(); }\n    if case == Case::Foreign {\n        profile.languages.push')
 body=body.replace('    let mut b = nepl3_core::budget::Budget::new(limits);','    if let Some(cap)=cap { limits.work=cap; }\n    let mut b = nepl3_core::budget::Budget::new(limits);')
 body=body.replace('        reader_calls: 0,','        reader_calls: 0,\n        trace: vec![],')
 body=body.replace('    let mut calls = 0;','    let mut trace=host.trace.clone();\n    let mut calls = 0;')
 body=body.replace('        calls += 1;','        trace.push(call_tag(call));\n        calls += 1;')
 body=body.replace('return Ok(reply);','return Ok((reply,trace));').replace('return Ok(result);','return Ok((result,trace));')
 body=body.replace('.map_err(|v| format!("overflow stop {v:?}"));','.map(|v|(v,trace)).map_err(|v| format!("overflow stop {v:?}"));')
 body=body.replace('        _ => return Err(format!("terminal {:?}", reply.outcome)),','        _ => return Ok((reply,trace)),')
 body=body.replace('    if case == Case::Owned {\n        assert_eq!(calls, 4);\n    } // choose shape, two contexts, and ordinary x\'s explicit None.','')
 body=body.replace('    Ok(reply)','    Ok((reply,trace))')
 body=body.replace('return Ok((result,trace));','assert_sticky(&result,&mut b); return Ok((result,trace));').replace('    let tree = match &reply.outcome {','    assert_sticky(&reply,&mut b);\n    let tree = match &reply.outcome {')
 body=body.replace('    if case == Case::Portable {\n        portable::tree_roundtrip', '    if case == Case::Portable && input.starts_with("choose") {\n        portable::tree_roundtrip')
 wrapper='fn run_case(input: &str, case: Case, exercise_rejections: bool) -> Result<ParseReply,String> { observed(input,case,exercise_rejections,0,false,false,None).map(|v|v.0) }\n'
 s=s[:start]+wrapper+body+s[end:]
 s=s.replace('    reader_calls: usize,','    reader_calls: usize,\n    trace: Vec<String>,')
 s=s.replace('    Owned,','    Owned,\n    IndependentList,',1)
 s=s.replace('    for i in 0..forms {','    if case==Case::IndependentList { package.reads.push(ReadSpec::ListOf { element: ReadSpecId(1), cons: kind("List:LocalCons")?, nil:kind("List:Nil")? }); package.forms[0].fields[1].read=ReadSpecId(3); }\n    for i in 0..forms {',1)
 s=s.replace('        self.calls += 1;','        self.trace.push(call_tag(call));\n        self.calls += 1;')
 s+='\n'+(r/'extra.rs').read_text(encoding='utf-8')
 (p/'tests/head.rs').write_text(s,encoding='utf-8',newline='\n')
 (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8');print(name,head,len(files))
(r/'delta.diff').write_bytes(git('diff','6f84a51','3f23aec'))
