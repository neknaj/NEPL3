from pathlib import Path
import subprocess,json,hashlib,re,importlib.util
R=Path(__file__).resolve().parent;repo=R.parents[1]
HEAD='38a76c57f2b1e2e4b1ec39b575d0c5848cb4b88b';BASE='da40ffcef088e54e135da9bb19bcbda26519a662'
P='doc/migration/authored/21-doc-pages.nepld';S='doc/spec/21-doc-pages.md'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,b):
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def blob(rev,path,out):
 b=git('show',rev+':'+path);save(out,b);return b
assert git('rev-parse',HEAD+'^').decode().strip()==BASE
assert git('diff','--name-only',BASE,HEAD).decode().splitlines()==[P]
before=blob(BASE,P,'before.nepld');after=blob(HEAD,P,'after.nepld')
source=blob(BASE,S,'source.md');prior=blob(BASE+'^',S,'previous-source.md')
assert source==git('show',HEAD+':'+S)
old='        cons "parse/lower、bare-metal、previewの[既定値/きていち]やParseProfileのLimitsは[変更/へんこう]しない。"\n'
new='        cons "output_limits[自体/じたい]はparse/lower、bare-metal、previewの[既定値/きていち]やParseProfileのLimitsを[変更/へんこう]しない。"\n'
text=after.decode();i=text.index('          cons code "parse_limits"');i=text.rfind('      cons paragraph\n',0,i)
j=text.index('      cons paragraph\n        cons "Section',i)
addition=text[i:j]
assert text.replace(addition,'',1).replace(new,old,1).encode()==before
md=source.decode();i=md.index('pages入力manifestには `parse_limits`');j=md.index('Sectionの明示ID、',i);addition_md=md[i:j]
old_md='parse/lower、bare-metal、previewの既定値やParseProfileのLimitsは変更しない。'
new_md='output_limits自体はparse/lower、bare-metal、previewの既定値やParseProfileのLimitsを変更しない。'
assert md.replace(addition_md,'',1).replace(new_md,old_md,1).encode()==prior
save('addition.nepld',addition.encode());save('addition.md',addition_md.encode())
save('draft.diff',git('diff',BASE,HEAD,'--',P));save('source.diff',git('diff',BASE+'^',BASE,'--',S))
for path in ['doc/authoring.md','AGENTS.md','tools/audit/structure.py','design/forms.json']:
 blob(HEAD,path,'context/'+path)
spec=importlib.util.spec_from_file_location('structure',R/'context/tools/audit/structure.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
forms=m.load_forms(R/'context');m.Parser(text,forms).complete('Doc/Article')
tree=m.Parser('article ja "" body\n'+addition+'nil\n',forms).complete('Doc/Article')
def walk(x):
 if isinstance(x,list):
  for y in x:yield from walk(y)
 elif isinstance(x,dict):
  yield x
  for y in x.get('fields',{}).values():yield from walk(y)
han=re.compile('[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+');ruby=re.compile(r'\[([^\[\]/]+?)/([^\[\]/]+?)\]');readings=[]
def pair(b,r):
 assert han.fullmatch(b) and re.fullmatch('[ぁ-ゖー]+',r),(b,r)
 readings.append([b,r]);return b
def plain(x,protected=False):
 if isinstance(x,list):return ''.join(plain(y,protected) for y in x)
 if not isinstance(x,dict):return ''
 k=x['kind'];f=x.get('fields',{})
 if k=='leaf':
  s=json.loads(x['token']);rest=ruby.sub('',s)
  assert not han.search(rest) and not any(c in rest for c in '[]{}\\')
  return ruby.sub(lambda t:pair(t[1],t[2]),s)
 if k=='Ruby':return pair(plain(f['base'],True),plain(f['reading'],True))
 if k in ['Text','InlineCode']:
  if k=='Text' and not protected:assert not han.search(f['text'])
  return f['text']
 return ''.join(plain(y,protected) for y in f.values())
paragraphs=[x for x in walk(tree) if x['kind']=='Paragraph']
actual=[plain(p) for p in paragraphs]
expected=[re.sub(r'`([^`]+)`',r'\1',p.replace('\n','')) for p in addition_md.strip().split('\n\n')]
assert actual==expected
counts=[];prefix=0;literal=0
for p in paragraphs:
 units=[x for x in walk(p) if x['kind'] in ['Sentence','leaf']];counts.append(len(units))
 for unit in units:
  n=len(readings);s=plain(unit);del readings[n:];assert s.endswith('。') and s.count('。')==1
  prefix+=unit['kind']=='Sentence';literal+=unit['kind']=='leaf'
assert counts==[7,9,11]
codes=[x['fields']['text'] for x in walk(tree) if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',addition_md)
changed=m.Parser('article ja "" body cons paragraph\n'+new+'        nil\nnil\n',forms).complete('Doc/Article')
cp=[x for x in walk(changed) if x['kind']=='Paragraph'];n=len(readings);assert plain(cp[0])==new_md;changed_readings=readings[n:];del readings[n:]
assert b'\r' not in after and not after.startswith(b'\xef\xbb\xbf')
save('paragraphs.txt',('\n\n'.join(actual)+'\n').encode());save('readings.json',(json.dumps(readings,ensure_ascii=False,indent=2)+'\n').encode())
author=repo/'.tmp/author-phase-limits';am=(author/'manifest.json').read_bytes();assert sha(am)=='f98de22f89a953de1c9d1404b254253a80e833c0d1c1e23153fb03b5c4e6962d'
save('author-manifest.json',am)
for e in json.loads(am)['files']:
 b=(author/e['path']).read_bytes();assert len(b)==e['bytes'] and sha(b)==e['sha256']
assert (author/'addition.nepld').read_bytes()==addition.encode() and (author/'source.md').read_bytes()==source and (author/'after.nepld').read_bytes()==after
git('diff','--check',BASE,HEAD)
result={'head':HEAD,'source_commit':BASE,'old_content_preserved_except_explicit_one_sentence':True,'source_unchanged_by_author':True,'before_sha256':sha(before),'after_sha256':sha(after),'source_sha256':sha(source),'addition_sha256':sha(addition.encode()),'paragraph_sentences':counts,'added_sentences':sum(counts),'changed_sentences':1,'inline_code':codes,'prefix_sentences':prefix,'literal_sentences':literal,'ruby_count':len(readings),'ruby_unique':len(set(map(tuple,readings))),'changed_sentence_ruby':changed_readings,'author_manifest_payloads':len(json.loads(am)['files']),'full_structure_audit':True,'runtime_executed':False}
save('result.json',(json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode());print(json.dumps(result,ensure_ascii=False,indent=2))
