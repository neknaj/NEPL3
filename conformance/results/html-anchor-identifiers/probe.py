from pathlib import Path
import subprocess,json,shutil
r=Path(__file__).resolve().parent;records=[]
for mode in ['native','wasi']:
 cmd=['cargo','build','--offline','--manifest-path',str(r/'probe/Cargo.toml'),'--target-dir',str(r/'target')]
 if mode=='wasi':cmd+=['--target','wasm32-wasip2']
 p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=600);(r/('probe-build-'+mode+'.log')).write_bytes(p.stdout);records.append({'command':cmd,'exit':p.returncode,'deadline':600});print('build',mode,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
 artifact=r/'target'/('debug/anchor-probe.exe' if mode=='native' else 'wasm32-wasip2/debug/anchor-probe.wasm');dest=r/('probe-native.exe' if mode=='native' else 'probe-wasi.wasm');shutil.copyfile(artifact,dest)
 cmd=[str(dest)] if mode=='native' else ['wasmtime','run',str(dest)]
 p=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60);(r/('probe-'+mode+'.log')).write_bytes(p.stdout);records.append({'command':cmd,'exit':p.returncode,'deadline':60});(r/'probe-runs.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8');print('run',mode,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
