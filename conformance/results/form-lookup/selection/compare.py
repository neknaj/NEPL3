from pathlib import Path
import re,json
r=Path(__file__).resolve().parent
def rows(name):
 d={}
 for line in (r/(name+'.log')).read_text(encoding='utf-8').splitlines():
  start=re.search(r'(?:CASE|NOHEAD|UNKNOWN|LIST|CANCEL)\|',line)
  if start:
   line=line[start.start():]
   m=re.search(r'Usage \{([^}]+)\}',line);assert m,line
   usage={k:int(v) for k,v in re.findall(r'(\w+): (\d+)',m[1])};key=line[:m.start()];tail=line[m.end():]
   d[key]=(usage,tail)
 return d
a=rows('before-native');b=rows('native');assert a.keys()==b.keys()
out=[]
for key,(u,tail) in a.items():
 v,t=b[key];assert t==tail,key
 assert all(v[k]==val for k,val in u.items() if k!='work'),key
 assert v['work']<=u['work'],key
 out.append(dict(case=key,before=u['work'],after=v['work'],saved=u['work']-v['work']))
result=dict(cases=len(out),outcomes_and_call_traces_equal=True,non_work_usage_equal=True,work=out)
if (r/'wasi.json').exists() and json.loads((r/'wasi.json').read_text())['exit_code']==0:
 w=rows('wasi');assert w.keys()==b.keys()
 for key,(u,t) in b.items():
  v,wt=w[key];assert wt==t,key
  assert all(v[k]==n for k,n in u.items() if k!='allocation_units'),key
 result['wasi_semantics_and_non_allocation_usage_equal']=True
(r/'comparison.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,indent=2))
