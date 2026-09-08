from pathlib import Path
import hashlib,json,re,subprocess,sys
repo=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
head='9ca3a7fd193d2965d1c513e01e6be6e48f693f70';base='b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
paths=['AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py','doc/migration/authored/project/AGENTS.nepld']
hashes=[]
for path in paths:
 data=git('show',head+':'+path);p=out/'source'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
 hashes.append({'path':path,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
assert git('show',head+':AGENTS.md')==git('show',base+':AGENTS.md')
assert git('diff','--name-only',head+'^',head).decode().splitlines()==[paths[-1]]
author=repo/'.tmp/author-root-agents';raw=(author/'manifest.json').read_bytes()
assert hashlib.sha256(raw).hexdigest()=='83942ec8e1a3be6ab50b0767cdac765c5d88489f2b4bb4ce2b9e59f87744603b'
for item in json.loads(raw)['files']:
 b=(author/item['path']).read_bytes();assert len(b)==item['bytes'] and hashlib.sha256(b).hexdigest()==item['sha256']
assert (author/'AGENTS.md').read_bytes()==git('show',head+':AGENTS.md')
assert (author/'AGENTS.nepld').read_bytes()==git('show',head+':'+paths[-1])
(out/'original-author-manifest.json').write_bytes(raw)
sys.path.insert(0,str(out/'source'))
from tools.audit.structure import Parser,load_forms
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
exec(helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')])
raw=(out/'source'/paths[-1]).read_bytes();assert b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf')
tree=Parser(raw.decode(),load_forms(out/'source')).complete('Doc/Article');audit_text(tree);n=normalize(tree,True)
nodes=list(walk(n));md=(out/'source/AGENTS.md').read_text(encoding='utf-8')
def strip(s):return re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',s).replace('`','')
expected=[];kind=[]
for block in md.strip().split('\n\n'):
 if block.startswith('#'):continue
 if block.startswith('- '):
  for line in block.splitlines():assert line.startswith('- ');expected.append(strip(line[2:]));kind.append('listitem')
 else:expected.append(strip(block.replace('\n','')));kind.append('paragraph')
paragraphs=[x for x in nodes if x['kind']=='Paragraph'];actual=[plain(x) for x in paragraphs]
assert actual==expected,list(zip(actual,expected))
sections=[x for x in nodes if x['kind']=='Section'];assert [plain(x['fields']['title']) for x in sections]==re.findall(r'^## (.*)$',md,re.M)
items=[x for x in nodes if x['kind']=='ListItem'];lists=[x for x in nodes if x['kind']=='List']
assert len(sections)==6 and len(items)==20 and len(paragraphs)==28 and kind.count('paragraph')==8
assert [len(x['fields']['items']) for x in lists]==[12,8]
assert all(x['fields']['style']['kind']=='Unordered' for x in lists)
sentences=[s for p in paragraphs for s in p['fields']['items']]
assert len(sentences)==81 and all(plain(s).endswith('\u3002') and plain(s).count('\u3002')==1 for s in sentences)
codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',md) and len(codes)==17
links=[(plain(x['fields']['label']),x['fields']['target']['fields'].get('path',x['fields']['target']['fields'].get('uri'))) for x in nodes if x['kind']=='Link'];assert links==re.findall(r'\[([^\]]+)\]\(([^)]+)\)',md) and len(links)==2
assert plain(n['fields']['title'])==md.splitlines()[0][2:]
result={'head':head,'canonical':base,'source_hashes':hashes,'source_unchanged':True,'only_draft_added':True,'original_author_manifest_verified':True,
        'sections':6,'standalone_paragraphs':8,'list_items':20,'paragraphs_within_lists_included':28,'sentences':81,'inline_codes':codes,'links':links,'ruby_pairs':len(pairs),'narrative_kanji_unannotated':0,
        'scope':'Independent full content and structure review; no production execution or canonical cutover claim'}
(out/'paragraphs.txt').write_text('\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps(result,ensure_ascii=False,indent=2))
