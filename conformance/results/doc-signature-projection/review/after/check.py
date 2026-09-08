import pathlib,subprocess,json,hashlib,importlib.util,sys,copy,shutil,re
sys.stdout.reconfigure(encoding='utf-8')
p=pathlib.Path(__file__).resolve().parent;workspace=p/'workspace';sys.path.insert(0,str(workspace/'tools/audit'));sys.path.insert(0,str(workspace/'tools/generate'))
from structure import Parser,load_forms,AuditError
import signatures
def nodes(x,kind):
 if isinstance(x,dict):
  if x.get('kind')==kind:yield x
  for y in x.values():yield from nodes(y,kind)
 elif isinstance(x,list):
  for y in x:yield from nodes(y,kind)
def cell(x):
 if x['kind']=='leaf':return json.loads(x['token'])
 assert x['kind']=='Sentence';v=x['fields']['inlines'];assert len(v)==1 and v[0]['kind']=='InlineCode';return v[0]['fields']['text']
def types(x):
 # Iterative independent formatter; only the formal singleton-list wrapper.
 depth=0
 while isinstance(x,dict):assert list(x)==['list'];depth+=1;x=x['list']
 assert isinstance(x,str) and x
 return 'List<'*depth+x+'>'*depth
forms=load_forms(workspace);rows=[]
for lang in ['Grammar','Doc','Math','Circuit']:
 raw=(workspace/'doc/migration/generated'/f'{lang.lower()}-signatures.nepld').read_bytes();assert b'\r' not in raw and raw.endswith(b'\n') and not raw.startswith(b'\xef\xbb\xbf')
 assert signatures.generate(forms,lang).encode()==raw
 tree=Parser(raw.decode(),forms).complete('Doc/Article');sections=list(nodes(tree,'Section'));expected=[(k,v) for k,v in forms.items() if k.split('/')[0]==lang];assert len(sections)==len(expected)
 count=0;fields=0;leaves=[];anchors=[]
 for section,(category,definition) in zip(sections,expected,strict=True):
  assert cell(section['fields']['title'])==category;anchor=section['fields']['id'];assert bytes.fromhex(anchor.removeprefix('category_')).decode()==category;anchors.append(anchor)
  table,=nodes(section,'Table');assert [v['kind'] for v in table['fields']['columns']]==['AlignmentLeft']*3+['AlignmentRight']
  actual=[[cell(c) for c in row['fields']['cells']] for row in table['fields']['rows']]
  want=[]
  for spelling,form in definition['forms'].items():
   f=form['fields'];want.append([spelling,lang+'.'+form['kind'],', '.join(x['name']+': '+types(x['read']) for x in f) if f else 'なし',str(len(f))]);fields+=len(f)
  assert actual==want;count+=len(want)
  leaf=definition.get('leaf');paragraphs=list(nodes(section,'Paragraph'));assert len(paragraphs)==bool(leaf)
  if leaf:
   code,=nodes(paragraphs[0],'InlineCode');assert code['fields']['text']==leaf;leaves.append([category,leaf])
 assert len(set(anchors))==len(anchors)
 rows.append(dict(language=lang,categories=len(expected),forms=count,fields=fields,leaves=leaves,sha256=hashlib.sha256(raw).hexdigest()))
# Hostile punctuation remains InlineCode Text, not Ruby/Anno or syntax injection.
special='漢字\"\\\n\r\t[]{} / <&> # % 😀'
custom={'Doc/A':{'forms':{special:{'kind':special,'fields':[{'name':'second','read':{'list':{'list':'@Text'}}},{'name':'first','read':special}]}},'leaf':special}}
tree=Parser(signatures.generate(custom,'Doc'),forms).complete('Doc/Article');table,=nodes(tree,'Table');actual=[cell(x) for x in table['fields']['rows'][0]['fields']['cells']]
assert actual==[special,'Doc.'+special,'second: List<List<@Text>>, first: '+special,'2']
assert list(nodes(tree,'Paragraph'))[-1]['fields']
failures=[]
for bad in ['',{},1,False,{'list':None},{'list':'x','extra':'y'}]:
 try:signatures.read_type(bad)
 except ValueError:failures.append(repr(bad))
 else:raise AssertionError(bad)
for cp in [0,1,8,11,12,31]:
 try:signatures.quoted(chr(cp))
 except ValueError:pass
 else:raise AssertionError(cp)
commands=[]
def run(label,args,cwd=workspace):
 r=subprocess.run(args,cwd=cwd,capture_output=True,timeout=60);(p/(label+'.log')).write_bytes(r.stdout+r.stderr);commands.append(dict(label=label,args=args,cwd=str(cwd),deadline_seconds=60,exit_code=r.returncode));return r
assert run('managed',[sys.executable,'-m','unittest','discover','-s','tools/generate','-p','test_signatures.py']).returncode==0
assert run('stale-clean',[sys.executable,'tools/generate/signatures.py']).returncode==0
assert run('write1',[sys.executable,'tools/generate/signatures.py','--write']).returncode==0
assert run('write2',[sys.executable,'tools/generate/signatures.py','--write']).returncode==0
for row in rows:assert hashlib.sha256((workspace/'doc/migration/generated'/f'{row["language"].lower()}-signatures.nepld').read_bytes()).hexdigest()==row['sha256']
mut=p/'mutations';shutil.copytree(workspace,mut,dirs_exist_ok=True);target=mut/'doc/migration/generated/math-signatures.nepld';original=target.read_bytes();target.write_bytes(original+b'\n');assert run('stale-altered',[sys.executable,'tools/generate/signatures.py'],mut).returncode!=0;assert target.read_bytes()==original+b'\n';target.unlink();assert run('stale-missing',[sys.executable,'tools/generate/signatures.py'],mut).returncode!=0;assert not target.exists()
assert run('repair',[sys.executable,'tools/generate/signatures.py','--write'],mut).returncode==0;assert target.read_bytes()==original
# Duplicate normative keys: compare generator acceptance with existing strict loader.
f=mut/'design/forms.json';parsed=json.loads(f.read_text(encoding='utf-8'));cat=parsed['categories'];duplicate=json.dumps({'categories':cat},ensure_ascii=False)[:-2]+',"Math/Expr":{"forms":{"replacement":{"kind":"Replacement","fields":[]}}}}}'
f.write_text(duplicate,encoding='utf-8',newline='\n')
try:load_forms(mut)
except AuditError:strict_rejected=True
else:strict_rejected=False
r=run('duplicate-keys',[sys.executable,'tools/generate/signatures.py','--write'],mut)
(p/'duplicate-forms.json').write_bytes(f.read_bytes());(p/'duplicate-math.nepld').write_bytes(target.read_bytes())
result=dict(rows=rows,negative_read_types=failures,special_text=repr(special),commands=commands,duplicate_key=dict(strict_loader_rejected=strict_rejected,generator_exit=r.returncode,replacement_generated=b'Math.Replacement' in target.read_bytes()))
(p/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(result,ensure_ascii=False,indent=2))
