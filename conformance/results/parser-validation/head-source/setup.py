from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tree-head-source-locality'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
for name,rev in [('source','6f84a51'),('before','e7124f9')]:
 head=git('rev-parse',rev).decode().strip();dest=r/name;dest.mkdir(exist_ok=True);files=[]
 paths=git('ls-tree','-r','--name-only',head,'crates/foundation','Cargo.toml','Cargo.lock','rust-toolchain.toml').decode().splitlines()
 for path in paths:
  data=git('show',head+':'+path);files.append(dict(path=path,sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)));p=dest/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 p=dest/'Cargo.toml';p.write_text(re.sub(r'members = \[[^\n]+','members = ["crates/foundation/core", "crates/foundation/wire", "crates/foundation/reader", "crates/foundation/engine", "probe"]',p.read_text(encoding='utf-8')),encoding='utf-8',newline='\n')
 p=dest/'probe';(p/'tests/parse').mkdir(parents=True,exist_ok=True)
 (p/'Cargo.toml').write_text('[package]\nname="tree-head-probe"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nnepl3-core={path="../crates/foundation/core"}\nnepl3-reader={path="../crates/foundation/reader"}\nnepl3-engine={path="../crates/foundation/engine"}\nnepl3-wire={path="../crates/foundation/wire"}\n',encoding='utf-8')
 original=git('show',head+':crates/foundation/engine/tests/package.rs').decode()
 pre=original.split('#[test]')[0];pre=re.sub(r'#\[path = "package/[^\n]+\nmod [^\n]+\n','',pre)
 test=original[original.index('#[test]'):original.index('    let source = SourceSnapshot::new(')]
 test=test.replace('persistent_tree_checks_parent_reads_spelling_payload_and_concrete_owner','independent_selection_matrix')
 (p/'tests/selection.rs').write_text(pre+test+(r/'extra.rs').read_text(),encoding='utf-8',newline='\n')
 (p/'tests/parse/support.rs').write_bytes(git('show',head+':crates/foundation/engine/tests/parse/support.rs'))
 foreign=git('show',head+':crates/foundation/engine/tests/parse/foreign.rs').decode().replace('use super::support::*;','#[path="parse/support.rs"] mod support;\nuse support::*;')
 foreign=foreign.replace('fn foreign_root_mode_is_guest_owned_and_normal_child_and_host_restore_defaults','fn independent_foreign_scope')
 marker='    assert_eq!(reply.report.usage.source_bytes, input.len() as u64);'
 assert foreign.count(marker)==1;foreign=foreign.replace(marker,marker+'\n'+(r/'foreign-extra.rs').read_text())
 (p/'tests/foreign.rs').write_text(foreign,encoding='utf-8',newline='\n')
 (r/(name+'-manifest.json')).write_text(json.dumps(dict(commit=head,files=files),indent=2)+'\n',encoding='utf-8');print(name,head,len(files))
(r/'delta.diff').write_bytes(git('diff','e7124f9','6f84a51'))
