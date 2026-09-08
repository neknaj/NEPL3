from pathlib import Path
import subprocess,hashlib,json,re
R=Path(__file__).resolve().parent;W=R/'workspace'
repo='C:/projects/NEPL3-completed-tree-owned-validation';rev='7251b0199ce5ba4216fc78060ed32b012c70e09f'
def git(*args):return subprocess.check_output(['git','-C',repo,*args],timeout=60)
def sha(b):return hashlib.sha256(b).hexdigest()
changes={e['path']:e for e in json.loads((R/'instrumentation.json').read_bytes())}
rows=[]
for entry in git('ls-tree','-rz','--full-tree',rev).split(b'\0'):
 if not entry:continue
 meta,path=entry.split(b'\t',1);mode,kind,oid=meta.decode().split();path=path.decode()
 if kind!='blob':continue
 b=(W/path).read_bytes()
 if path in changes:assert sha(b)==changes[path]['after']
 else:assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==oid,path
 rows.append(dict(path=path,git_blob=oid,sha256=sha(b),bytes=len(b),instrumented=path in changes))
(R/'workspace-files.json').write_text(json.dumps(dict(commit=rev,files=rows),indent=2)+'\n',encoding='utf-8')
doc=git('rev-parse','2681cc0').decode().strip()
assert git('diff','--name-only',rev,doc).decode().splitlines()==['doc/spec/03-reader.md']
(R/'spec03-final.md').write_bytes(git('show',doc+':doc/spec/03-reader.md'))
(R/'spec03-delta.diff').write_bytes(git('diff',rev,doc,'--','doc/spec/03-reader.md'))
(R/'production-delta.diff').write_bytes(git('diff','5c7f4f3',rev,'--','crates/foundation/engine/src/parse/session.rs'))
for path in ['crates/foundation/engine/src/parse/model.rs','crates/foundation/engine/src/parse/build.rs','crates/foundation/core/src/syntax.rs']:
 (R/(path.replace('/','--')+'.context')).write_bytes(git('show',rev+':'+path))
counts={}
for name in ['native','wasi']:
 s=(R/(name+'.log')).read_text(encoding='utf-8')
 counts[name]=dict(passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',s))),restored=re.findall(r'REVIEW_RESTORED ok=(\d+) stopped=(\d+) error=(\d+)',s))
 assert counts[name]['passed']==23 and len(counts[name]['restored'])==1
(R/'validation.json').write_text(json.dumps(dict(production=rev,doc_only_head=doc,counts=counts,source_files_verified=len(rows),only_instrumented=list(changes)),indent=2)+'\n',encoding='utf-8')
print(json.dumps(counts))
