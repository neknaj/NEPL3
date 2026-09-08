from pathlib import Path
import subprocess,json,hashlib
r=Path(__file__).resolve().parent;repo='C:/projects/NEPL3-parse-source-merge-locality';old=r/'initial-8efa';old.mkdir(exist_ok=True)
for name in ['source-manifest.json','delta.diff','managed-native.log','managed-native.json','managed-wasi.log','managed-wasi.json','probe-native.log','probe-native.json']:(old/name).write_bytes((r/name).read_bytes())
head=subprocess.check_output(['git','-C',repo,'rev-parse','85dc3a6']).decode().strip()
manifest=json.loads((r/'source-manifest.json').read_text());manifest['commit']=head
for entry in manifest['files']:
 data=subprocess.check_output(['git','-C',repo,'show',head+':'+entry['path']]);entry['sha256']=hashlib.sha256(data).hexdigest();entry['bytes']=len(data)
 if entry['path'] not in ['Cargo.toml','Cargo.lock']:(r/'source'/entry['path']).write_bytes(data)
(r/'source-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
(r/'delta.diff').write_bytes(subprocess.check_output(['git','-C',repo,'diff','4be6b34',head]))
(r/'adjacent.diff').write_bytes(subprocess.check_output(['git','-C',repo,'diff','8efa29e',head]))
print(head)
