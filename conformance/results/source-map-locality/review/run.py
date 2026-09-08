import json,os,pathlib,subprocess,sys,time
p=pathlib.Path(__file__).resolve().parent
label=sys.argv[1]; args=sys.argv[2:]
env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
start=time.monotonic()
with (p/(label+'.log')).open('wb') as f:
    try:r=subprocess.run(args,cwd=p/'workspace',env=env,stdout=f,stderr=subprocess.STDOUT,timeout=600);code=r.returncode
    except subprocess.TimeoutExpired:code='timeout'
(p/(label+'.json')).write_text(json.dumps(dict(command=args,cwd=str(p/'workspace'),runner=env['CARGO_TARGET_WASM32_WASIP2_RUNNER'],exit_code=code,seconds=time.monotonic()-start),indent=2)+'\n',encoding='utf-8',newline='\n')
print(label,code)
print((p/(label+'.log')).read_text(encoding='utf-8',errors='replace')[-4500:])
