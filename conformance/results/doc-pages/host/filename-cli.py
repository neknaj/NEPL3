from pathlib import Path
import json,subprocess,hashlib,os,time
p=Path(__file__).resolve().parent;d=p/'filename-cases';d.mkdir(exist_ok=True);exe=p/'target/debug/nepl3-tools.exe';rows=[]
source='article en "Title" body nil'
def run(name,pages=None,raw=None,files=None,pre=False):
 root=d/name;root.mkdir(exist_ok=True)
 for path,content in (files or {'a.nepld':source,'b.nepld':source}).items():
  f=root/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(content if isinstance(content,bytes) else content.encode('utf-8'))
 m=root/'input.json';m.write_bytes(raw if raw is not None else json.dumps({'version':1,'pages':pages or [{'id':'a','source':'a.nepld','route':'index.html'}]}).encode())
 out=root/'out'
 if pre:out.mkdir(exist_ok=True);(out/'sentinel').write_bytes(b'keep')
 args=[str(exe),'doc-html','pages',str(m),str(out)];start=time.monotonic()
 try:r=subprocess.run(args,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=30);code=r.returncode;log=r.stdout
 except subprocess.TimeoutExpired as e:code='timeout';log=e.stdout or b''
 (root/'run.log').write_bytes(log);row={'name':name,'exit':code,'seconds':time.monotonic()-start,'output_exists':out.exists(),'manifest_exists':(out/'manifest.json').exists(),'log':log.decode('utf-8',errors='replace')};rows.append(row);print(name,code,row['output_exists'],flush=True);return out
entry=lambda id,src,route:dict(id=id,source=src,route=route)

for name,route in [('reserved-con','CON.html'),('reserved-com1','a/com1.html'),('reserved-lpt9','Lpt9.html'),('trailing-dot','dir./a.html'),('reserved-nul','NUL/a.html')]:
 out=run(name,[entry('a','a.nepld',route)]);assert not out.exists()
for name,left,right in [('route-case','A.html','a.html'),('folder-case','DOCS/a.html','docs/b.html'),('css-parent-case','index.html','ASSETS/DOC.CSS/a.html')]:
 out=run(name,[entry('a','a.nepld',left),entry('b','b.nepld',right)]);assert not out.exists()
out=run('normal',[entry('a','a.nepld','docs/a.html'),entry('b','b.nepld','docs/b.html')]);assert (out/'manifest.json').is_file()
run('existing',pre=True);assert (d/'existing/out/sentinel').read_bytes()==b'keep'
(p/'filename-cli-results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
