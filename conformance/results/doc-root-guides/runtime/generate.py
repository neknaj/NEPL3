import subprocess,pathlib,json,hashlib
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'.tmp/root-guides-generation-corrected';out.mkdir(exist_ok=True)
binary=pathlib.Path('C:/projects/NEPL3-doc-output-budget/target/debug/nepl3-tools.exe')
records={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'binary_source_commit':'b84e9a3854dd23538154194054c28713714ba123','binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'runs':[]}
for name in ['README','CODEX','AGENTS']:
    source=root/f'doc/migration/authored/project/{name}.nepld'
    command=[str(binary),'doc-html','export',str(source),str(out/name)]
    log=out/(name+'.log')
    with log.open('wb') as stream:
        result=subprocess.run(command,cwd=root,stdout=stream,stderr=subprocess.STDOUT,timeout=180)
    records['runs'].append({'name':name,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'command':command,'exit_code':result.returncode,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
    (out/'results.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    print(name,result.returncode,log.read_text(encoding='utf-8')[-1000:],flush=True)
