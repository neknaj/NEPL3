from pathlib import Path
import json,hashlib,subprocess,re,tomllib
r=Path(__file__).resolve().parent
verified={}
for folder,manifest in [('source','source-manifest.json'),('before-source','before-source-manifest.json')]:
    data=json.loads((r/manifest).read_text(encoding='utf-8'))
    for f in data['files']:
        b=(r/folder/f['path']).read_bytes()
        assert len(b)==f['bytes'] and hashlib.sha256(b).hexdigest()==f['sha256'],f['path']
    verified[folder]={'commit':data['commit'],'files':len(data['files'])}
assert (r/'probe/tests/independent.rs').read_bytes()==(r/'before-probe/tests/independent.rs').read_bytes()==(r/'independent.rs').read_bytes()
def locked(path):
    return sorted((p['name'],p['version'],p.get('checksum')) for p in tomllib.loads(path.read_text(encoding='utf-8'))['package'] if 'source' in p)
probe=locked(r/'probe/Cargo.lock')
assert probe==locked(r/'before-probe/Cargo.lock')
assert set(probe).issubset(set(locked(r/'source/Cargo.lock')))
counts={}
for name in ['managed-native','managed-wasi','probe-native','probe-wasi','before-native']:
    data=json.loads((r/(name+'.json')).read_text(encoding='utf-8'));assert data['exit_code']==0
    log=(r/(name+'.log')).read_text(encoding='utf-8')
    counts[name]=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',log)))
versions={}
for cmd in [['rustc','-Vv'],['cargo','-V'],['wasmtime','--version']]:
    versions[' '.join(cmd)]=subprocess.check_output(cmd).decode().strip()
summary={'verified_snapshots':verified,'test_counts':counts,'probe_registry_versions_match_root_lock':True,'versions':versions,
 'native_clean_second_resume':{'before':{'actual_requested_allocation':53769,'allocation_units':82804,'work':310803},'after':{'actual_requested_allocation':23083,'allocation_units':51702,'work':280779}}}
(r/'results.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
files=[]
for p in sorted(r.iterdir()):
    if p.is_file() and p.name!='manifest.json':
        b=p.read_bytes();files.append({'path':p.name,'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
for name in ['probe/Cargo.toml','probe/Cargo.lock','before-probe/Cargo.toml','before-probe/Cargo.lock']:
    b=(r/name).read_bytes();files.append({'path':name,'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
(r/'manifest.json').write_text(json.dumps({'scope':'Independent review; no production edits','files':files},indent=2)+'\n',encoding='utf-8')
print(json.dumps(summary,indent=2));print('manifest',hashlib.sha256((r/'manifest.json').read_bytes()).hexdigest())
