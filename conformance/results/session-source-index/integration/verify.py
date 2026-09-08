import pathlib,subprocess,json,hashlib
root=pathlib.Path(__file__).resolve().parents[1]
dest=root/'.tmp/verification'
dest.mkdir(exist_ok=True)
commands=[['cargo','fmt','--all','--','--check'],['cargo','test','--locked','--workspace'],['cargo','clippy','--locked','--workspace','--all-targets','--','-D','warnings'],['cargo','run','--locked','-p','nepl3-tools','--','check']]
record={'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'runs':[]}
for i,command in enumerate(commands):
    log=dest/f'{i}.log'
    with log.open('wb') as out:result=subprocess.run(command,cwd=root,stdout=out,stderr=subprocess.STDOUT)
    record['runs'].append({'command':command,'exit_code':result.returncode,'log':log.name,'sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
    (dest/'results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
    print(i,result.returncode,flush=True)
    if result.returncode:raise SystemExit(result.returncode)
