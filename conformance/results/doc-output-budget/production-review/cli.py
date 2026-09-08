from pathlib import Path
import json,subprocess
r=Path(__file__).parent;exe=r/'target/debug/nepl3-tools.exe';records=[]
for i,value in enumerate([None,{}, {'work':1}, {'source_bytes':1,'work':-1,'depth':1,'nodes':1,'allocation_units':1,'output_bytes':1,'diagnostics':1,'events':1},{'source_bytes':1,'work':1.5,'depth':1,'nodes':1,'allocation_units':1,'output_bytes':1,'diagnostics':1,'events':1}]):
 p=r/f'invalid-{i}.json';out=r/f'invalid-output-{i}';p.write_text(json.dumps({'version':1,'pages':[{'id':'one','source':'missing.nepld','route':'index.html'}],'output_limits':value}),encoding='utf-8')
 result=subprocess.run([str(exe),'doc-html','pages',str(p),str(out)],capture_output=True)
 records.append({'case':i,'exit':result.returncode,'stdout':result.stdout.decode('utf-8'),'stderr':result.stderr.decode('utf-8'),'output_exists':out.exists()});assert result.returncode!=0 and not out.exists()
(r/'cli.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
print(records)
