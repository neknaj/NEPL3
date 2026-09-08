from pathlib import Path
import json,re,importlib.util,hashlib,subprocess
R=Path(__file__).resolve().parent
source=(R/'source.md').read_text(encoding='utf-8');after=(R/'draft.nepld').read_text(encoding='utf-8')
spec=importlib.util.spec_from_file_location('structure',R/'context/structure.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
forms=m.load_forms(R/'context');tree=m.Parser(after,forms).complete('Doc/Article')
def walk(x):
    if isinstance(x,list):
        for y in x: yield from walk(y)
    elif isinstance(x,dict):
        yield x
        for y in x.get('fields',{}).values(): yield from walk(y)
han = re.compile('[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+')
ruby = re.compile(r'\[([^\[\]/]+?)/([^\[\]/]+?)\]')
readings=[]
def pair(base,reading):
    assert han.fullmatch(base) and re.fullmatch('[ぁ-ゖー]+',reading), (base,reading)
    readings.append([base,reading]); return base
def plain(x,protected=False):
    if isinstance(x,list): return ''.join(plain(y,protected) for y in x)
    if not isinstance(x,dict): return ''
    k=x['kind']; f=x.get('fields',{})
    if k=='leaf':
        s=json.loads(x['token'])
        rest=ruby.sub('',s)
        assert not han.search(rest) and not any(c in rest for c in '[]{}\\')
        return ruby.sub(lambda t:pair(t[1],t[2]),s)
    if k=='Ruby': return pair(plain(f['base'],True),plain(f['reading'],True))
    if k=='Link': return plain(f['label'],protected)
    if k in ('Text','InlineCode'):
        if k=='Text' and not protected: assert not han.search(f['text'])
        return f['text']
    return ''.join(plain(y,protected) for y in f.values())
paragraph_nodes=[x for x in walk(tree) if x['kind']=='Paragraph']
paragraphs=[plain(x) for x in paragraph_nodes]
expected=[]
for p in source.strip().split('\n\n')[1:]:
 p=p.replace('\n','');p=re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',p);p=re.sub(r'`([^`]+)`',r'\1',p);expected.append(p)
assert paragraphs==expected,[(a,b) for a,b in zip(paragraphs,expected) if a!=b]
assert len(paragraphs)==5
count_before_title=len(readings)
assert plain(tree['fields']['title'])==source.splitlines()[0][2:]
assert len(readings)==count_before_title+2
sentences=[x for p in paragraph_nodes for x in walk(p) if x['kind'] in ('Sentence','leaf')]
assert len(sentences)==19
for x in sentences:
 n=len(readings);s=plain(x);del readings[n:];assert s.count('\u3002')==1 and s.endswith('\u3002'),s
codes=[x['fields']['text'] for x in walk(tree) if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',source)
links=[x for x in walk(tree) if x['kind']=='Link']
targets=[x['fields']['target']['fields']['path'] for x in links]
assert targets==re.findall(r'\[[^\]]+\]\(([^)]+)\)',source)
assert all(x['fields']['target']['kind']=='RelativeTarget' for x in links)
assert '\r' not in after
result=dict(source_commit='b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2',source_sha256=hashlib.sha256(source.encode()).hexdigest(),draft_sha256=hashlib.sha256(after.encode()).hexdigest(),paragraphs=len(paragraphs),body_sentences=len(sentences),code_values=codes,link_targets=targets,ruby_count=len(readings),ruby_unique=len(set(map(tuple,readings))),structure=True,paragraph_text_exact=True,independent_review=False,production_html=False)
(R/'paragraphs.txt').write_text('\n\n'.join(paragraphs)+'\n',encoding='utf-8');(R/'readings.json').write_text(json.dumps(readings,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');(R/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps(result,ensure_ascii=False,indent=2))
