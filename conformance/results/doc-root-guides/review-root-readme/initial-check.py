from pathlib import Path
import hashlib, importlib.util, json, re, subprocess
R=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('structure',R/'context/structure.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
source=(R/'source.md').read_text(encoding='utf-8')
draft=(R/'draft.nepld').read_text(encoding='utf-8')
tree=m.Parser(draft,m.load_forms(R/'context')).complete('Doc/Article')
def walk(x):
    if isinstance(x,list):
        for y in x:yield from walk(y)
    elif isinstance(x,dict):
        yield x
        for y in x.get('fields',{}).values():yield from walk(y)
han=re.compile('[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+')
ruby=re.compile(r'\[([^\[\]/]+)/([^\[\]/]+)\]')
readings=[]
def pair(base,reading):
    assert han.fullmatch(base) and re.fullmatch('[\u3041-\u3096\u30fc]+',reading)
    readings.append([base,reading]);return base
def plain(x,protected=False):
    if isinstance(x,list):return ''.join(plain(y,protected) for y in x)
    if not isinstance(x,dict):return ''
    k=x['kind'];f=x.get('fields',{})
    if k=='leaf':
        s=json.loads(x['token']);assert not han.search(ruby.sub('',s));return ruby.sub(lambda t:pair(t[1],t[2]),s)
    if k=='Ruby':return pair(plain(f['base'],True),plain(f['reading'],True))
    if k in ('Text','InlineCode'):
        if k=='Text' and not protected:assert not han.search(f['text'])
        return f['text']
    if k=='Link':return plain(f['label'],protected)
    if k=='Image':return plain(f['alt'],protected)
    return ''.join(plain(y,protected) for y in f.values())
nodes=list(walk(tree))
paragraphs=[x for x in nodes if x['kind']=='Paragraph']
texts=[plain(x) for x in paragraphs]
def md(s):
    s=re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',s)
    s=re.sub(r'`([^`]+)`',r'\1',s)
    return s.replace('**','').replace('\n','')
parts=source.strip().split('\n\n')
expected=['CI']+[md(p) for p in parts[2:] if not p.startswith(('|','```'))]
assert texts==expected
assert len(paragraphs)==7
counts=[]
for p in paragraphs:
    sentences=[n for n in walk(p) if n['kind'] in ('Sentence','leaf')]
    counts.append(len(sentences))
    for n in sentences:
        start=len(readings);text=plain(n);del readings[start:]
        assert text=='CI' or (text.endswith('\u3002') and text.count('\u3002')==1)
assert counts==[1,2,5,4,2,3,1]
tables=[x for x in nodes if x['kind']=='Table'];assert len(tables)==1
table=tables[0]
rows=[x for x in walk(table) if x['kind']=='Row']
actual_rows=[[plain(cell) for cell in row['fields']['cells']] for row in rows]
md_rows=[[md(cell.strip()) for cell in line.strip('|').split('|')] for line in source.splitlines() if line.startswith('|') and not line.startswith('| ---')]
assert actual_rows==md_rows and len(rows)==6
codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode']
outside=re.sub(r'```[^\n]*\n.*?```','',source,flags=re.S)
assert codes==re.findall(r'`([^`]+)`',outside)
blocks=[x for x in nodes if x['kind']=='RawCode'];assert len(blocks)==1
raw=blocks[0]['fields'];original=re.search(r'```sh\n(.*?)```',source,re.S)[1]
assert raw['text']==original
links=[x for x in nodes if x['kind']=='Link'];assert len(links)==15
targets=[x['fields']['target']['fields'].get('uri',x['fields']['target']['fields'].get('path')) for x in links]
source_without_image=re.sub(r'!\[[^\]]*\]\([^)]+\)','CI',source)
assert targets==re.findall(r'\[[^\]]+\]\(([^)]+)\)',source_without_image)
images=[x for x in nodes if x['kind']=='Image'];assert len(images)==1
assert links[0]['fields']['label']==images[0]
strong=[plain(x) for x in nodes if x['kind']=='Strong'];assert strong==re.findall(r'\*\*(.*?)\*\*',source)
assert '\r' not in draft
author=R.parent/'author-root-readme';am=(author/'manifest.json').read_bytes()
assert hashlib.sha256(am).hexdigest().startswith('37961')
(R/'author-manifest.json').write_bytes(am)
for e in json.loads(am)['files']:
    b=(author/e['path']).read_bytes();assert len(b)==e['bytes'] and hashlib.sha256(b).hexdigest()==e['sha256']
assert (author/'source.md').read_bytes()==(R/'source.md').read_bytes()
assert (author/'draft.nepld').read_bytes()==(R/'draft.nepld').read_bytes()
(R/'paragraphs.txt').write_text('\n\n'.join(texts)+'\n',encoding='utf-8')
(R/'readings.json').write_text(json.dumps(readings,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
result=dict(source_commit='b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2',draft_commit=subprocess.check_output(['git','rev-parse','d022e9b'],cwd=R.parents[1]).decode().strip(),paragraphs=7,sentence_counts=counts,table=actual_rows,inline_code=codes,rawcode=raw,links=targets,badge=images[0],strong=strong,source_sha256=hashlib.sha256(source.encode()).hexdigest(),draft_sha256=hashlib.sha256(draft.encode()).hexdigest(),structure=True,production_execution=False)
(R/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
