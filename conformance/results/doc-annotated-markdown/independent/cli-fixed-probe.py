from pathlib import Path
import subprocess,json,hashlib
root=Path(__file__).resolve().parent
binary=root/'target/debug/nepl3-tools.exe'
inputs=root/'cli-fixed';inputs.mkdir(exist_ok=True)
source=inputs/'source--&note.nepld'
source.write_text('article en "T" body cons paragraph cons "Body." nil nil',encoding='utf-8')
alias=inputs/'aliases.json'; alias.write_text('[{"name":"safe"}]',encoding='utf-8')
results=[]
def run(name,src=source,a=alias,success=False):
 out=inputs/(name+'.md');cmd=[str(binary),'doc-markdown','annotated',str(src),str(a),str(out)]
 p=subprocess.run(cmd,cwd=root/'workspace',capture_output=True)
 (inputs/(name+'.log')).write_bytes(p.stdout+p.stderr)
 assert (p.returncode==0)==success,(name,p.returncode,p.stderr)
 assert out.exists()==success,name
 results.append(dict(name=name,command=cmd,exit=p.returncode,output=out.exists()))
 return out
out=run('valid',success=True); content=out.read_bytes();text=content.decode()
assert text.count('<!--')==1 and text.count('-->')==1
assert 'source&#45;&#45;&amp;note.nepld' in text
assert hashlib.sha256(source.read_bytes()).hexdigest() in text
assert hashlib.sha256(alias.read_bytes()).hexdigest() in text
p=subprocess.run([str(binary),'doc-markdown','annotated',str(source),str(alias),str(out)],capture_output=True)
assert p.returncode!=0 and out.read_bytes()==content
results.append(dict(name='nooverwrite',exit=p.returncode,preserved=True))
for name,data in [('duplicate',b'[{"name":"a","name":"b"}]'),('unknown',b'[{"name":"a","x":1}]'),('null',b'null'),('oversize',b' '*1048577),('utf8',b'\xff'),('unsafe',b'[{"name":"x\"onclick"}]'),('missing-section',b'[{"name":"x","section":"absent"}]')]:
 a=inputs/(name+'.json');a.write_bytes(data);run(name,a=a)
for name,data in [('invalid-source',b'bad'),('source-utf8',b'\xff'),('source-oversize',b' '*10000001),('relative',b'article en "T" body cons paragraph cons sentence cons link relative "x.md" none text "x" nil nil nil')]:
 s=inputs/(name+'.nepld');s.write_bytes(data);run(name,src=s)
(root/'cli-results-fixed.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'results':results},indent=2),encoding='utf-8')
print(len(results),'cases passed')
