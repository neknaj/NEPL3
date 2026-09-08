from pathlib import Path
import os, subprocess, sys, time, json
base=Path(__file__).resolve().parent
mode=sys.argv[1]
env=dict(os.environ)
env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
cmd=['cargo','test','--locked']
cwd=base
if mode.startswith('managed'):
    cmd+=['-p','nepl3-tools','--test','doc_capacity']
    cwd=base/'fixed'
else:
    cmd+=['--manifest-path',str(base/'probe/Cargo.toml')]
target='target-wasi' if mode.endswith('wasi') else 'target-native'
cmd+=['--target-dir',str(base/target)]
if mode.endswith('wasi'):cmd+=['--target','wasm32-wasip2']
cmd+=['--','--nocapture','--test-threads=1']
start=time.perf_counter()
with (base/(mode+'-utf8.log')).open('wb') as log:
    result=subprocess.run(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
data={'command':cmd,'cwd':str(cwd),'exit_code':result.returncode,'elapsed_seconds':time.perf_counter()-start}
(base/(mode+'-run.json')).write_bytes((json.dumps(data,indent=2)+'\n').encode('utf-8'))
print(json.dumps(data))
sys.exit(result.returncode)
