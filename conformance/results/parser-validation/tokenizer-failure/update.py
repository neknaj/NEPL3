from pathlib import Path
import subprocess,json,hashlib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-tokenizer-owned-failure'
p=r/'extra.rs';p.write_text(p.read_text().replace('local_kind:u32::MAX','local_kind:u64::MAX').replace('session.read_accepted(','session.read_with_accepted('),encoding='utf-8')
(r/'initial-source-manifest.json').write_bytes((r/'source-manifest.json').read_bytes())
(r/'initial-delta.diff').write_bytes((r/'delta.diff').read_bytes())
head=subprocess.check_output(['git','-C',repo,'rev-parse','edda943']).decode().strip();m=json.loads((r/'source-manifest.json').read_text());m['commit']=head
for e in m['files']:
 data=subprocess.check_output(['git','-C',repo,'show',head+':'+e['path']]);e['sha256']=hashlib.sha256(data).hexdigest();e['bytes']=len(data)
 if e['path'] not in ['Cargo.toml','Cargo.lock']:(r/'source'/e['path']).write_bytes(data)
(r/'source-manifest.json').write_text(json.dumps(m,indent=2)+'\n',encoding='utf-8')
(r/'delta.diff').write_bytes(subprocess.check_output(['git','-C',repo,'diff','be6aa8b',head]));print(head)
