from pathlib import Path
import hashlib,json,gzip,re,subprocess
R=Path(__file__).resolve().parent
def sha(b):return hashlib.sha256(b).hexdigest()
def load(p):return json.loads(p.read_bytes())
result={'sources':[], 'semantic':[], 'runs':{},'tracked_snapshots_verified':[]}
for label in ['before','after']:
 w=R/label;files=load(R/(label+'-files.json'))
 for e in files:
  b=(w/e['path']).read_bytes();assert len(b)==e['bytes'] and sha(b)==e['sha256']
 extras={p.relative_to(w).as_posix() for p in w.rglob('*') if p.is_file()}-{e['path'] for e in files}
 assert extras=={'tools/examples/review_doc.rs'}
 assert (w/'tools/examples/review_doc.rs').read_bytes()==(R/'probe.rs').read_bytes()
 result['tracked_snapshots_verified'].append({'label':label,'tracked_files':len(files),'only_extra':'tools/examples/review_doc.rs'})
 d=R/('runs-'+label)
 rows=load(d/'results.json')+load(d/'cbor-results.json')
 for row in rows:assert sha((d/row['log']).read_bytes())==row['log_sha256']
 assert [r['exit'] for r in rows]==[0,0,1,1,1,1,0,0,0]
 result['runs'][label]={}
 for key in ['05','15','development','review']:
  log=(d/(key+'.log')).read_text(encoding='utf8')
  m=re.search(r'cursor=Some\((\d+)\)',log)
  usage=re.search(r'usage=Usage \{ (.+) \}',log)
  stage='local preparation / NeedsResolution' if 'NeedsResolution(' in log else 'native host callback inside parse' if 'native host:' in log else 'parse candidate boundary'
  result['runs'][label][key]={'stage':stage,'cursor':int(m[1]) if m else None,'usage':dict((k,int(v)) for k,v in re.findall(r'(\w+): (\d+)',usage[1])) if usage else None,'log':log}
for key in ['linear','05','15','development','review']:
 b=(R/'inputs'/(key+'.nepld')).read_bytes()
 result['sources'].append({'name':key,'bytes':len(b),'sha256':sha(b)})
for name in ['tree.txt','profile.txt','document.txt','document-ndf.txt','document.cbor']:
 b=(R/'runs-before/semantic-cbor'/name).read_bytes();a=(R/'runs-after/semantic-cbor'/name).read_bytes()
 assert b==a,name
 result['semantic'].append({'path':name,'bytes':len(b),'sha256':sha(b),'equal':True})
for label in ['before','after']:
 for name in ['tree.txt','profile.txt','document.txt','document-ndf.txt','parse-usage.txt']:
  assert (R/('runs-'+label)/'semantic'/name).read_bytes()==(R/('runs-'+label)/'semantic-cbor'/name).read_bytes()
old=load(R/'runs-before/linear-output/manifest.json');new=load(R/'runs-after/linear-output/manifest.json')
result['linear_operations']={'before':old.pop('operations'),'after':new.pop('operations')}
assert old==new
for name in ['document.html','assets/doc.css']:
 b=(R/'runs-before/linear-output'/name).read_bytes();assert b==(R/'runs-after/linear-output'/name).read_bytes()
result['linear_manifest_without_usage']=old
result['linear_other_operations_equal']=all(result['linear_operations']['before'][k]==result['linear_operations']['after'][k] for k in ['lower','prepare_render_serialize'])
assert result['linear_other_operations_equal']
(R/'comparison.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf8')
print(json.dumps(result,indent=2))
