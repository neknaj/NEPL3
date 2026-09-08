from pathlib import Path
import json,hashlib,struct,subprocess
r=Path(__file__).parent
old=json.loads((r/'before/observed-manifest.json').read_text())
new=json.loads((r/'workspace/observed-manifest.json').read_text())
assert (r/'before/observed-files.json').read_bytes()==(r/'workspace/observed-files.json').read_bytes()
assert old['identity']==new['identity']
for a,b in zip(old['pages'],new['pages']):
 for k,v in a.items():assert b[k]==v,(k,v,b[k])
assert old['output_usage']==new['output_usage']
keys=['source_bytes','work','depth','nodes','allocation_units','output_bytes','diagnostics','events']
expected=hashlib.sha256(b'nepl3.local-doc-pages.execution/1\0'+bytes.fromhex(new['identity'])+b''.join(struct.pack('>Q',new['output_budget'][part][k])for part in ['limits','initial_usage']for k in keys)).hexdigest()
assert new['execution_identity']==expected
(r/'native-manifest.json').write_text(json.dumps(new,indent=2)+'\n',encoding='utf-8')
(r/'before-manifest.json').write_text(json.dumps(old,indent=2)+'\n',encoding='utf-8')
(r/'files.json').write_bytes((r/'workspace/observed-files.json').read_bytes())
(r/'comparison.json').write_text(json.dumps({'files_equal':True,'semantic_identity':new['identity'],'execution_identity_independent':expected,'prior_fields_equal':True,'output_usage_equal':True},indent=2)+'\n',encoding='utf-8')
print(expected)
