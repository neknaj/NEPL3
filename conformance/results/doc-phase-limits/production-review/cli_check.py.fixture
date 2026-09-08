import pathlib,subprocess,json,hashlib,struct
p=pathlib.Path('.tmp/review-phase-limits'); d=p/'cli';d.mkdir(exist_ok=True)
exe=(p/'target/debug/nepl3-tools.exe').resolve()
source='article ja "Title" body cons paragraph cons "abc" nil nil\n';(d/'input.nepld').write_text(source,encoding='utf8')
limits=dict(zip(['source_bytes','work','depth','nodes','allocation_units','output_bytes','diagnostics','events'],[10000000,100000000,1000,10000000,500000000,10000000,1000,1000]))
m={'version':1,'pages':[{'id':'page','source':'input.nepld','route':'index.html'}]}
results=[]
def run(name,raw):
 f=d/(name+'.json');f.write_text(raw,encoding='utf8'); out=d/(name+'-out'); r=subprocess.run([str(exe),'doc-html','pages',str(f.resolve()),str(out.resolve())],capture_output=True);(d/(name+'.log')).write_bytes(r.stdout+r.stderr);results.append({'name':name,'exit':r.returncode,'output_exists':out.exists()});return out
run('default',json.dumps(m))
for field in ['parse_limits','lower_limits']:
 for name,v in [('zero',0),('changed',100000001)]:
  x=dict(m);x[field]=dict(limits,work=v);run(field+'-'+name,json.dumps(x))
 base=json.dumps(dict(m,**{field:limits}))
 for name,raw in [('overflow',base.replace('100000000,','18446744073709551616,')),('duplicate',base.replace('"work": 100000000','"work": 1,"work": 100000000')),('float',base.replace('"work": 100000000','"work": 1e8')),('negative',base.replace('"work": 100000000','"work": -1')),('unknown',base.replace('"work": 100000000','"unknown": 1,"work": 100000000'))]:run(field+'-'+name,raw)
for r in results:
 assert r['exit']==(0 if r['name']=='default' or r['name'].endswith('changed') else 1),r
 assert r['output_exists']==(r['exit']==0),r
keys=list(limits)
for name in ['default','parse_limits-changed','lower_limits-changed']:
 a=json.loads((d/(name+'-out/manifest.json')).read_text());b=b'nepl3.local-doc-pages.phases/1\0'+bytes.fromhex(a['execution_identity'])+struct.pack('>Q',len(a['pages']))
 for page in a['pages']:
  b+=bytes.fromhex(page['profile_sha256'])
  for field in ['parse_limits','parse_initial_usage','lower_limits','lower_initial_usage']:b+=struct.pack('>8Q',*[page[field][k] for k in keys])
 assert hashlib.sha256(b).hexdigest()==a['phase_execution']['identity']
 r=next(r for r in results if r['name']==name);r['phase_sha']=hashlib.sha256(b).hexdigest();r['html_sha']=hashlib.sha256((d/(name+'-out/index.html')).read_bytes()).hexdigest()
a=json.loads((d/'default-out/manifest.json').read_text());b=json.loads((d/'lower_limits-changed-out/manifest.json').read_text());assert a['execution_identity']==b['execution_identity'];assert a['pages'][0]['profile_sha256']==b['pages'][0]['profile_sha256'];assert b['pages'][0]['operation_limits'] is None
p.joinpath('cli-results.json').write_text(json.dumps({'binary_sha':hashlib.sha256(exe.read_bytes()).hexdigest(),'results':results},indent=2)+'\n',encoding='utf8')
print(json.dumps(results,indent=2))
