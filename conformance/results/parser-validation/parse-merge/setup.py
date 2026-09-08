from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-parse-source-merge-locality'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
for name,rev in [('source','8efa29e'),('before','4be6b34')]:
 head=git('rev-parse',rev).decode().strip();dest=r/name;dest.mkdir(exist_ok=True);files=[]
 paths=git('ls-tree','-r','--name-only',head,'crates/foundation','Cargo.toml','Cargo.lock','rust-toolchain.toml').decode().splitlines()
 for path in paths:
  data=git('show',head+':'+path);files.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)));p=dest/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 p=dest/'Cargo.toml';p.write_text(re.sub(r'members = \[[^\n]+','members = ["crates/foundation/core", "crates/foundation/wire", "crates/foundation/reader", "crates/foundation/engine", "probe"]',p.read_text(encoding='utf-8')),encoding='utf-8',newline='\n')
 p=dest/'probe';(p/'tests/parse').mkdir(parents=True,exist_ok=True)
 (p/'Cargo.toml').write_text('[package]\nname="parse-merge-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nnepl3-core={path="../crates/foundation/core"}\nnepl3-reader={path="../crates/foundation/reader"}\nnepl3-engine={path="../crates/foundation/engine"}\nnepl3-wire={path="../crates/foundation/wire"}\n',encoding='utf-8')
 for leaf in ['support.rs','foreign.rs']:(p/'tests/parse'/leaf).write_bytes(git('show',head+':crates/foundation/engine/tests/parse/'+leaf))
 main=git('show',head+':crates/foundation/engine/tests/parse.rs').decode()
 mark='                Ok(result.reply)'
 main=main.replace(mark,'                if let host::Action::Closure { mode: 1, .. } = action {\n                    assert!(result.host_error.is_some());\n                    assert!(matches!(result.reply.outcome, ParseOutcome::Await { .. }));\n                }\n'+mark)
 (p/'tests/probe.rs').write_text(main+'\n'+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
 host=git('show',head+':crates/foundation/engine/tests/parse/host.rs').decode().replace('    Serve,','    Serve,\n    Closure { order: u8, mode: u8 },').replace('                Action::Serve => {}','                Action::Serve => {}\n                Action::Closure { mode: 2, .. } => { budget.cancel(); budget.poll()?; }\n                Action::Closure { mode: 3, .. } => return Ok(None),\n                Action::Closure { .. } => {}')
 host=host.replace('        let mut generated = vec![];', '        let mut generated = vec![];\n'+(r/'host-extra.rs').read_text())
 (p/'tests/parse/host.rs').write_text(host,encoding='utf-8',newline='\n')
 (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8')
 print(name,head,len(files))
(r/'delta.diff').write_bytes(git('diff','4be6b34','8efa29e'))
