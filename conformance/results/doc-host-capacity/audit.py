from pathlib import Path
import hashlib,json,subprocess,tomllib
base=Path(__file__).resolve().parent
repo=base.parents[1]
def git(*args):return subprocess.check_output(['git','-C',str(repo),*args])
rows=[]
for path in ['tools/src/doc/source.rs','tools/tests/doc_capacity.rs','doc/spec/21-doc-pages.md']:
    b=git('show','0516d9fa245a201277f5abd684a69df5eda1ca53:'+path)
    assert b==(base/'fixed'/path).read_bytes()
    rows.append({'path':path,'commit':'0516d9fa245a201277f5abd684a69df5eda1ca53','sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
path='.github/workflows/ci.yml'
b=git('show','521075b60aa266db25ce8ce7ad53f23a8864f57f:'+path)
(base/'ci-final.yml').write_bytes(b)
rows.append({'path':path,'commit':'521075b60aa266db25ce8ce7ad53f23a8864f57f','sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
(base/'changes.patch').write_bytes(git('diff','16b471d','521075b','--','tools/src/doc/source.rs','tools/tests/doc_capacity.rs','doc/spec/21-doc-pages.md','.github/workflows/ci.yml'))
assert set(git('diff','--name-only','16b471d','0516d9f').decode().splitlines())=={'tools/src/doc/source.rs','tools/tests/doc_capacity.rs','doc/spec/21-doc-pages.md'}
assert git('diff','--name-only','0516d9f','521075b').decode().splitlines()==['.github/workflows/ci.yml']
production=json.loads((base/'files.json').read_text())
for row in production['files']:
    b=(base/'fixed'/row['path']).read_bytes()
    assert len(b)==row['bytes'] and hashlib.sha256(b).hexdigest()==row['sha256'],row['path']
locks=[]
for path in ['fixed/Cargo.lock','probe/Cargo.lock']:
    data=tomllib.loads((base/path).read_text())
    locks.append(sorted((p['name'],p['version'],p.get('checksum')) for p in data['package'] if 'source' in p))
assert locks[0]==locks[1]
versions={name:subprocess.check_output([name,'--version']).decode().strip() for name in ['rustc','cargo','wasmtime']}
(base/'reviewed-files.json').write_bytes((json.dumps({'production':rows,'fixed_files_unchanged':len(production['files']),'probe_external_packages_match_workspace_lock':True,'versions':versions},indent=2)+'\n').encode())
print(json.dumps(versions))
