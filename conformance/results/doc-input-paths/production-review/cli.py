from pathlib import Path
import json,subprocess,hashlib,os
r=Path(__file__).parent;exe=r/'target/debug/nepl3-tools.exe';root=r/'inputs';root.mkdir();(root/'docs').mkdir();(root/'drafts').mkdir();records=[];outputs=[]
texts={'intro':'article en "Intro" body cons paragraph cons sentence cons link relative "guide.md" none text "Guide" nil nil nil','guide':'article en "Guide" body nil'}
for id,text in texts.items():
 for directory,suffix in [('docs','md'),('drafts','nepld')]:(root/directory/(id+'.'+suffix)).write_text(text,encoding='utf-8')
base={'version':1,'pages':[{'id':id,'source':f'docs/{id}.md','route':f'pages/{id}/index.html'}for id in texts]}
def run(label,manifest,success):
 p=root/(label+'.json');p.write_text(json.dumps(manifest),encoding='utf-8');out=r/('output-'+label);cmd=[str(exe),'doc-html','pages',str(p),str(out)];result=subprocess.run(cmd,capture_output=True);(r/(label+'.stderr')).write_bytes(result.stderr)
 records.append({'label':label,'command':cmd,'exit':result.returncode,'output_exists':out.exists(),'stderr':result.stderr.decode('utf-8')});assert (result.returncode==0)==success,label
 if not success:assert not out.exists()
 else:outputs.append(out)
for label in ['omitted','null','explicit']:
 m=json.loads(json.dumps(base))
 for p in m['pages']:
  if label=='null':p['input']=None
  if label=='explicit':p['input']='drafts/'+p['id']+'.nepld'
 run(label,m,True)
for i,bad in enumerate(['','/absolute','../outside.nepld','a//b','a/./b','a/../b','a\\b','C:/outside','a?b','a#b','a%b','a\nb','あ'*1366,'missing.nepld']):
 m=json.loads(json.dumps(base));m['pages'][0]['input']=bad;run('invalid'+str(i),m,False)
outside=r/'outside';outside.mkdir();(outside/'intro.nepld').write_text(texts['intro'],encoding='utf-8')
try:
 os.symlink(outside/'intro.nepld',root/'escape.nepld');m=json.loads(json.dumps(base));m['pages'][0]['input']='escape.nepld';run('symlink',m,False)
except OSError as e:records.append({'label':'symlink-create','unavailable':str(e)})
junction=root/'escape-dir'
p=subprocess.run(['powershell','-NoProfile','-Command',f"New-Item -ItemType Junction -Path '{junction}' -Target '{outside}' | Out-Null"],capture_output=True)
if p.returncode==0:
 m=json.loads(json.dumps(base));m['pages'][0]['input']='escape-dir/intro.nepld';run('junction',m,False)
else:records.append({'label':'junction-create','unavailable':p.stderr.decode(errors='replace')})
manifests=[json.loads((out/'manifest.json').read_text())for out in outputs]
for m in manifests[1:]:
 assert m['identity']==manifests[0]['identity']and m['execution_identity']==manifests[0]['execution_identity']
for name in ['pages/intro/index.html','pages/guide/index.html','pages/intro/assets/doc.css','pages/guide/assets/doc.css']:
 assert len({(out/name).read_bytes()for out in outputs})==1,name
assert b'href="../guide/index.html"'in(outputs[2]/'pages/intro/index.html').read_bytes()
assert manifests[2]['pages'][0]['input']=='drafts/intro.nepld'and manifests[2]['pages'][0]['source']=='docs/intro.md'
(r/'cli-results.json').write_text(json.dumps({'binary_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'runs':records,'identity':manifests[0]['identity'],'all_artifacts_equal':True},indent=2)+'\n',encoding='utf-8');print([(x['label'],x.get('exit',x.get('unavailable')))for x in records])
