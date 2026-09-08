import pathlib,subprocess,json,sys,hashlib,shutil,importlib.util,re
sys.stdout.reconfigure(encoding='utf-8');p=pathlib.Path(__file__).resolve().parent;w=p/'workspace';sys.path.insert(0,str(w/'tools/generate'));import signatures
helper=p.parent.parent/'review-doc-authoring-foundation-progress/annotation_parser.py';shutil.copyfile(helper,p/'annotation_parser.py');spec=importlib.util.spec_from_file_location('annotations',p/'annotation_parser.py');parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser);parser.forms=json.loads((w/'design/forms.json').read_text())['categories']
def text(x):
 if isinstance(x,list):
  if x and x[0]=='Text':return x[1]
  return ''.join(text(y) for y in x)
 return ''
annotation=[]
for f in sorted((w/'doc/migration/generated').glob('*.nepld')):
 s='\n'.join(l for l in f.read_text(encoding='utf-8').splitlines() if not l.startswith('#'));v=parser.Parser(s).run();ruby=[];missing=[]
 def walk(x,inside=False):
  if not isinstance(x,list):return
  if x and isinstance(x[0],str):
   if x[0]=='InlineCode':return
   if x[0]=='Ruby':ruby.append([text(x[1]),text(x[2])]);return
   if x[0]=='Text' and re.search('[\u3400-\u9fff々]',x[1]):missing.append(x[1])
  for y in x:walk(y,inside)
 walk(v);assert not missing;assert all(re.fullmatch('[\u3400-\u9fff々]+',a) and re.fullmatch('[ぁ-ゖー]+',b) for a,b in ruby);annotation.append(dict(file=f.name,rubies=ruby))
# Verify all nested duplicate failures occur before any published output changes.
temp=p/'nested-duplicate';shutil.copytree(w,temp,dirs_exist_ok=True)
outputs={f.name:f.read_bytes() for f in (temp/'doc/migration/generated').glob('*.nepld')}
fixtures=['{"categories":{},"categories":{}}','{"categories":{"Doc/A":{},"Doc/A":{}}}','{"categories":{"Doc/A":{"forms":{"x":{},"x":{}}}}}','{"categories":{"Doc/A":{"forms":{"x":{"fields":[{"read":"@Text","read":"@Name"}]}}}}}']
cases=[]
for n,raw in enumerate(fixtures):
 (temp/'design/forms.json').write_text(raw,encoding='utf-8',newline='\n');r=subprocess.run([sys.executable,'tools/generate/signatures.py','--write'],cwd=temp,capture_output=True,timeout=60);assert r.returncode!=0 and b'Duplicate' in r.stderr
 assert all((temp/'doc/migration/generated'/name).read_bytes()==b for name,b in outputs.items())
 cases.append(dict(input=raw,exit=r.returncode,stderr=r.stderr.decode('utf-8')))
(p/'boundaries.json').write_text(json.dumps(dict(annotation=annotation,nested_duplicates=cases),ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n');print('PASS',len(annotation),len(cases))
