import hashlib,json,pathlib,subprocess,sys,time
p=pathlib.Path(__file__).resolve().parent
results=[]
for command in [[sys.executable,'--version'],[sys.executable,'audit.py'],[sys.executable,'check.py']]:
 started=time.monotonic()
 run=subprocess.run(command,cwd=p,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
 name=['python-version.log','audit.log','check.log'][len(results)]
 (p/name).write_bytes(run.stdout)
 results.append(dict(command=command,cwd=str(p),deadline_seconds=60,exit_code=run.returncode,elapsed_seconds=time.monotonic()-started,log=name,log_bytes=len(run.stdout),log_sha256=hashlib.sha256(run.stdout).hexdigest()))
(p/'execution.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8',newline='\n')
print([(r['command'][-1],r['exit_code']) for r in results])
