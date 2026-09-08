import hashlib,json,os,pathlib,subprocess,sys,time
base=pathlib.Path(__file__).resolve().parent
target=sys.argv[1]
env=os.environ.copy()
env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
env['PYTHONIOENCODING']='utf-8'
for suite, args in [('independent',['--test','review_sentences']),('managed-projection',['--test','doc','projection::'])]:
    command=['cargo','test','--locked','-p','nepl3-tools',*args]
    if target=='wasi': command+=['--target','wasm32-wasip2']
    command+=['--','--nocapture','--test-threads=1']
    start=time.time()
    result=subprocess.run(command,cwd=base/'workspace',env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=600)
    log=base/(target+'-'+suite+'.log')
    log.write_bytes(result.stdout)
    record={'command':command,'cwd':str(base/'workspace'),'target':target,'exit_code':result.returncode,'elapsed_seconds':time.time()-start,'sha256':hashlib.sha256(result.stdout).hexdigest(),'process_deadline_seconds':600}
    (base/(target+'-'+suite+'.json')).write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps(record),flush=True)
    if result.returncode:
        print(result.stdout.decode('utf-8',errors='replace'),flush=True)
        sys.exit(result.returncode)
