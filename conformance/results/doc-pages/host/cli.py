from pathlib import Path
import json,subprocess,hashlib,os,time
p=Path(__file__).resolve().parent;d=p/'cases';d.mkdir(exist_ok=True);exe=p/'target/debug/nepl3-tools.exe';rows=[]
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
a='article en "A" body cons paragraph cons sentence cons link page "b" none text "Go" nil nil nil'
b='article en "B" body cons paragraph cons sentence cons link page "a" none text "Back" nil nil nil'
out=run('nested',[entry('a','a.nepld','docs/a/index.html'),entry('b','b.nepld','docs/b/index.html')],files={'a.nepld':a,'b.nepld':b})
m=json.loads((out/'manifest.json').read_text(encoding='utf-8'))
for f in m['files']:assert hashlib.sha256((out/f['path']).read_bytes()).hexdigest()==f['sha256']
assert '../b/index.html' in (out/'docs/a/index.html').read_text(encoding='utf-8')
assert (out/'docs/a/assets/doc.css').is_file()
run('same-folder',[entry('a','a.nepld','a.html'),entry('b','b.nepld','b.html')])
run('existing',pre=True);assert (d/'existing/out/sentinel').read_bytes()==b'keep'
for name,route in [('escape','../escape.html'),('absolute','/abs.html'),('backslash','a\\b.html'),('bad-ext','a.txt'),('css-parent','assets/doc.css/nested.html'),('marker-parent','manifest.json/nested.html'),('device','CON.html')]:run(name,[entry('a','a.nepld',route)])
run('route-case',[entry('a','a.nepld','A.html'),entry('b','b.nepld','a.html')])
run('css-case',[entry('a','a.nepld','ASSETS/DOC.CSS/nested.html')])
run('source-escape',[entry('a','../outside.nepld','a.html')])
run('source-absolute',[entry('a',str(d/'nested/a.nepld'),'a.html')])
run('source-missing',[entry('a','missing.nepld','a.html')])
run('bad-utf8',files={'a.nepld':b'\xff'})
run('oversize-source',files={'a.nepld':b' '*10000001})
run('oversize-manifest',raw=b' '*65537)
run('bad-json',raw=b'{')
run('unknown-field',raw=b'{"version":1,"pages":[],"extra":1}')
run('version',raw=b'{"version":2,"pages":[]}')
run('empty',raw=b'{"version":1,"pages":[]}')
run('count',pages=[entry(str(i),'a.nepld',f'{i}.html') for i in range(129)])
run('source-total',[entry('a','a.nepld','a.html'),entry('b','b.nepld','b.html')],files={'a.nepld':b' '*5000001,'b.nepld':b' '*5000000})
run('trailing',files={'a.nepld':source+' unknown'})
run('missing-page',files={'a.nepld':a})
run('external',files={'a.nepld':'article en "A" body cons paragraph cons sentence cons link external "https://example.org" text "Go" nil nil nil'})
(p/'cli-results.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
