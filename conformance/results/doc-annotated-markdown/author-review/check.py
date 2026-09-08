from pathlib import Path
import subprocess,json,hashlib,re,importlib.util,difflib
R=Path(__file__).resolve().parent;repo=R.parents[1];src=repo/'.tmp/author-annotated-projection';HEAD='7b449ca2d1af3ed0ef135b8a0b9ca929e8a54569'
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,b):
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
raw=(src/'manifest.json').read_bytes();assert sha(raw)=='a6ca5d630e41584968edc3a776f92ad67be9958ddeb92d31f33b036c4ba7b24c';save('author-manifest.json',raw)
for e in json.loads(raw)['files']:
 b=(src/e['path']).read_bytes();assert len(b)==e['bytes'] and sha(b)==e['sha256'];save('author/'+e['path'],b)
before=(src/'before.nepld').read_bytes();after=(src/'after.nepld').read_bytes();source=(src/'source.md').read_bytes();addition=(src/'addition.nepld').read_bytes()
assert before==git('show',HEAD+':doc/migration/authored/21-doc-pages.nepld')
assert source==git('show',HEAD+':doc/spec/21-doc-pages.md')==(repo/'doc/spec/21-doc-pages.md').read_bytes()
assert after==(repo/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
assert before.endswith(b'  nil\n') and before[:-6]+addition+b'  nil\n'==after and after.count(b'cons section annotated ')==1
assert not after.startswith(b'\xef\xbb\xbf') and b'\r' not in after
for p in ['tools/audit/structure.py','design/forms.json','doc/authoring.md','AGENTS.md']:save('context/'+p,git('show',HEAD+':'+p))
spec=importlib.util.spec_from_file_location('fixed_structure',R/'context/tools/audit/structure.py');s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
tree=s.Parser(after.decode(),json.loads((R/'context/design/forms.json').read_bytes())['categories']).complete('Doc/Article')
section=tree['fields']['body']['fields']['blocks'][-1];assert section['kind']=='Section' and section['fields']['id']=='annotated'
paras=section['fields']['body']['fields']['blocks'];assert all(p['kind']=='Paragraph' for p in paras)
ruby=[];codes=[];kinds=[];han=re.compile(r'[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+')
def plain(n,protected=False):
 if isinstance(n,list):return ''.join(plain(x,protected) for x in n)
 if not isinstance(n,dict):return ''
 k=n['kind'];kinds.append(k)
 if k=='leaf':
  text=json.loads(n['token'])
  # This fixed addition has only simple Ruby in literals; no escaped/nested delimiters.
  pairs=re.findall(r'\[([^/\[\]]+)/([^\[\]]+)\]',text)
  for b,r in pairs:assert han.fullmatch(b) and r;ruby.append([b,r])
  remaining=re.sub(r'\[([^/\[\]]+)/([^\[\]]+)\]','',text);assert not han.search(remaining) and not any(c in remaining for c in '[{}]')
  return re.sub(r'\[([^/\[\]]+)/([^\[\]]+)\]',r'\1',text)
 f=n['fields']
 if k=='Ruby':
  b=plain(f['base'],True);r=plain(f['reading'],True);assert han.fullmatch(b) and r;ruby.append([b,r]);return b
 if k=='Text':
  if not protected:assert not han.search(f['text'])
  return f['text']
 if k=='InlineCode':codes.append(f['text']);return f['text']
 assert k in ['Sentence','Concat'];return ''.join(plain(v,protected) for v in f.values())
title=plain(section['fields']['title']);assert title=='注釈付きMarkdown閲覧projection'
actual=[[plain(n) for n in p['fields']['items']] for p in paras]
sourcepart=source.decode().split('## 注釈付きMarkdown閲覧projection\n',1)[1].strip()
expected=[''.join(p.splitlines()) for p in sourcepart.split('\n\n')]
assert len(expected)==len(actual)==8
comparisons=[]
for i,(a,e) in enumerate(zip(actual,expected,strict=True)):
 e=re.sub(r'`([^`]+)`',r'\1',e);joined=''.join(a)
 comparisons.append({'paragraph':i+1,'actual':joined,'expected':e,'exact_equal':joined==e,'sentences':a})
 reviewed=e
 if i==0:
  assert 'renderer識別子は\n`nepl3-tools.markdown-annotated/1`' in sourcepart
  reviewed=reviewed.replace('renderer識別子はnepl3-tools.markdown-annotated/1','renderer識別子は nepl3-tools.markdown-annotated/1')
 if i==5:
  assert '同じ\n`n-`' in sourcepart
  reviewed=reviewed.replace('SectionにはHTML backendと同じn-','SectionにはHTML backendと同じ n-')
 assert joined==reviewed,(i,joined,reviewed)
 assert all(x.endswith('。') and x.count('。')==1 for x in a)
assert codes==re.findall(r'`([^`]+)`',sourcepart)
counts=[len(p) for p in actual];assert counts==[3,5,6,6,7,7,5,8]
assert set(kinds)<= {'leaf','Sentence','InlineCode','Text','Ruby','Concat'}
save('paragraphs.json',(json.dumps(comparisons,ensure_ascii=False,indent=2)+'\n').encode());save('readings.json',(json.dumps(sorted(set(tuple(x) for x in ruby)),ensure_ascii=False,indent=2)+'\n').encode())
save('source-addition.md',sourcepart.encode());save('diff.patch',''.join(difflib.unified_diff(before.decode().splitlines(True),after.decode().splitlines(True))).encode())
literal_count=sum(n['kind']=='leaf' for p in paras for n in p['fields']['items'])
result={'result':'PASS with two explicitly reviewed softwrap-to-space boundaries','head_at_capture':HEAD,'source_sha256':sha(source),'before_sha256':sha(before),'after_sha256':sha(after),'addition_sha256':sha(addition),'paragraphs':8,'sentences':counts,'total_sentences':sum(counts),'literal_sentences':literal_count,'prefix_sentences':sum(counts)-literal_count,'inline_codes':codes,'ruby_occurrences_including_heading':len(ruby),'ruby_unique':len(set(tuple(x) for x in ruby)),'exact_paragraph_text_after_removing_MD_code_markers_and_softwraps':all(p['exact_equal'] for p in comparisons),'explicit_spacing_differences':2,'old_bytes_preserved':True,'production_runtime':False}
save('result.json',(json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode());print(json.dumps(result,ensure_ascii=True,indent=2))
