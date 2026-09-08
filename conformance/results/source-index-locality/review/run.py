from pathlib import Path
import subprocess,os,json,time,sys
r=Path(__file__).resolve().parent;mode=sys.argv[1];env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
if mode.startswith('managed'):
 cmd=['cargo','test','--locked','-p','nepl3-core'];cwd=r/'source'
else:
 name='before' if mode.startswith('before') else 'source';cmd=['cargo','test','--manifest-path',str(r/(name+'-probe/Cargo.toml')),'--test','tail'];cwd=r
cmd+=['--target-dir',str(r/(mode+'-target'))]
if 'wasi' in mode:cmd+=['--target','wasm32-wasip2']
cmd+=['--','--test-threads=1','--nocapture'];start=time.time()
with (r/(mode+'.log')).open('wb') as log:code=subprocess.run(cmd,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
(r/(mode+'.json')).write_text(json.dumps(dict(command=cmd,cwd=str(cwd),exit_code=code,elapsed_seconds=time.time()-start),indent=2)+'\n',encoding='utf-8')
print(mode,code);print((r/(mode+'.log')).read_text(encoding='utf-8')[-2600:]);sys.exit(code)
