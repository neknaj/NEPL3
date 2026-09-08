from pathlib import Path
import subprocess,hashlib,json,concurrent.futures,time
R=Path(__file__).resolve().parent
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(label):
 w=R/label;t=R/('target-'+label);d=R/('runs-'+label);d.mkdir(exist_ok=True)
 example=w/'tools/examples/review_doc.rs';example.parent.mkdir(exist_ok=True);example.write_bytes((R/'probe.rs').read_bytes())
 rows=[]
 def invoke(cmd,name,timeout=1800):
  start=time.monotonic()
  with (d/(name+'.log')).open('wb') as log:
   p=subprocess.run(cmd,cwd=w,stdout=log,stderr=subprocess.STDOUT,timeout=timeout)
  row={'command':list(map(str,cmd)),'cwd':str(w),'exit':p.returncode,'seconds':time.monotonic()-start,'log':name+'.log','log_sha256':sha(d/(name+'.log'))}
  rows.append(row);(d/'results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf8')
  return p.returncode
 if invoke(['cargo','build','--locked','-p','nepl3-tools','--bin','nepl3-tools','--example','review_doc','--target-dir',str(t)],'build')!=0:return label+' build failed'
 exe=t/'debug/nepl3-tools.exe';probe=t/'debug/examples/review_doc.exe'
 (d/'binaries.json').write_text(json.dumps({str(p):sha(p) for p in [exe,probe]},indent=2)+'\n',encoding='utf8')
 for key in ['linear','05','15','development','review']:
  invoke([str(exe),'doc-html','export',str(R/'inputs'/(key+'.nepld')),str(d/(key+'-output'))],key,300)
 invoke([str(probe),str(R/'inputs/linear.nepld'),str(d/'semantic')],'semantic',300)
 return label+' complete'
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
 for result in pool.map(run,['before','after']):print(result,flush=True)
