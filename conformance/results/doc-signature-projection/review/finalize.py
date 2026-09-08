import pathlib,subprocess,json,hashlib,sys
p=pathlib.Path(__file__).resolve().parent;original=json.loads((p/'sources.json').read_text());delta=json.loads((p/'after/sources.json').read_text());overrides={f['path']:f for f in delta['overrides']}
runs=[]
for root,script in [(p,'check.py'),(p/'after','check.py'),(p/'after','boundaries.py')]:
 r=subprocess.run([sys.executable,script],cwd=root,capture_output=True,timeout=60);label='final-'+script.removesuffix('.py');(root/(label+'.log')).write_bytes(r.stdout+r.stderr);runs.append(dict(cwd=str(root),command=[sys.executable,script],timeout_seconds=60,exit_code=r.returncode));assert r.returncode==0
for root in [p,p/'after']:
 for f in original['files']:
  expected=overrides.get(f['path'],f) if root!=p else f;b=(root/'workspace'/f['path']).read_bytes();assert len(b)==expected['bytes'] and hashlib.sha256(b).hexdigest()==expected['sha256']
 assert (root/'check.py').read_bytes()==(p/'check.py').read_bytes()
before=json.loads((p/'result.json').read_text(encoding='utf-8'));after=json.loads((p/'after/result.json').read_text(encoding='utf-8'));assert before['rows']==after['rows'];assert before['duplicate_key']==dict(strict_loader_rejected=True,generator_exit=0,replacement_generated=True);assert after['duplicate_key']==dict(strict_loader_rejected=True,generator_exit=1,replacement_generated=False)
assert (p/'duplicate-forms.json').read_bytes()==(p/'after/duplicate-forms.json').read_bytes()
assert (p/'after/duplicate-math.nepld').read_bytes()==(p/'workspace/doc/migration/generated/math-signatures.nepld').read_bytes()
(p/'execution.json').write_text(json.dumps(dict(python=sys.version,runs=runs,fixed_sources_reverified=True,original_counterexample_input_identical=True,normal_generation_identical=True),indent=2)+'\n',encoding='utf-8',newline='\n')
files=[]
for f in sorted(p.rglob('*')):
 if f.is_file() and f.name!='manifest.json' and not any(x in f.relative_to(p).parts for x in ['__pycache__','mutations','nested-duplicate']):
  b=f.read_bytes();files.append(dict(path=f.relative_to(p).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
b=(json.dumps(dict(files=files),indent=2)+'\n').encode();(p/'manifest.json').write_bytes(b);print(len(files),sum(f['bytes'] for f in files),hashlib.sha256(b).hexdigest())
