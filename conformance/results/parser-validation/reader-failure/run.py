from pathlib import Path
import subprocess,os,json,time,sys
r=Path(__file__).resolve().parent;mode=sys.argv[1];name='before' if mode.startswith('before') else ('probe' if mode.startswith('probe') else 'source');env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
cmd=['cargo','test','--offline','-p','nepl3-reader','--target-dir',str(r/(mode+'-target'))]
if 'wasi' in mode:cmd+=['--target','wasm32-wasip2']
if mode.startswith('probe'):cmd+=['--lib','review_owned::independent_']
cmd+=['--','--test-threads=1','--nocapture'];start=time.time()
with (r/(mode+'.log')).open('wb') as log:code=subprocess.run(cmd,cwd=r/name,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
(r/(mode+'.json')).write_text(json.dumps(dict(command=cmd,exit_code=code,elapsed_seconds=time.time()-start),indent=2)+'\n',encoding='utf-8');print(mode,code);print((r/(mode+'.log')).read_text(encoding='utf-8')[-1500:]);sys.exit(code)
