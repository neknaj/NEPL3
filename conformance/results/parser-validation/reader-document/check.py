from pathlib import Path
import subprocess, hashlib, json, re, importlib.util
R = Path(__file__).resolve().parent
REPO = Path('C:/projects/NEPL3-doc-reader-recovery')
HEAD = '1fee084e3bff960525ac87cbf6180a0af0caa962'
def git(*args):
    return subprocess.check_output(['git', '-C', str(REPO), *args], timeout=60)
def sha(b): return hashlib.sha256(b).hexdigest()
def save(name, data):
    p = R/name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)
def blob(rev, path, name):
    b = git('show', rev+':'+path); save(name,b); return b
base = git('rev-parse','16f9f765').decode().strip()
assert git('rev-parse',HEAD+'^').decode().strip() == base
path = 'doc/migration/authored/03-reader.nepld'
assert git('diff','--name-only',base,HEAD).decode().splitlines() == [path]
before = blob(base,path,'before.nepld'); after = blob(HEAD,path,'after.nepld')
source = blob(HEAD,'doc/spec/03-reader.md','source.md')
prior = blob('5a0b72f','doc/spec/03-reader.md','previous-source.md')
assert source == git('show',base+':doc/spec/03-reader.md')
assert source.startswith(prior)
addition_md = source[len(prior):]; assert len(addition_md.decode().splitlines()) == 36
closing = b'          nil\n      nil\n  nil\n'
assert before.endswith(closing) and after.endswith(closing)
prefix = before[:-len(closing)]
assert after.startswith(prefix)
addition = after[len(prefix):-len(closing)]
assert len(addition) == len(after)-len(before) == 7125
assert b'\r' not in after and not after.startswith(b'\xef\xbb\xbf')
save('addition.md',addition_md); save('addition.nepld',addition)
save('delta.diff',git('diff',base,HEAD,'--',path))
blob(HEAD,'doc/authoring.md','authoring.md'); blob(HEAD,'AGENTS.md','AGENTS.md')
blob(HEAD,'tools/audit/structure.py','context/structure.py')
blob(HEAD,'design/forms.json','context/design/forms.json')
spec = importlib.util.spec_from_file_location('structure',R/'context/structure.py')
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
forms = m.load_forms(R/'context')
m.Parser(after.decode(),forms).complete('Doc/Article')
tree = m.Parser('article ja "" body\n'+addition.decode()+'nil\n',forms).complete('Doc/Article')
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
    if k in ('Text','InlineCode'):
        if k=='Text' and not protected: assert not han.search(f['text'])
        return f['text']
    return ''.join(plain(y,protected) for y in f.values())
paragraph_nodes=[x for x in walk(tree) if x['kind']=='Paragraph']
paragraphs=[plain(x) for x in paragraph_nodes]
expected=[re.sub(r'`([^`]+)`',r'\1',p.replace('\n','')) for p in addition_md.decode().strip().split('\n\n')]
assert paragraphs == expected
sentences=[]
for p in paragraph_nodes:
    sentences += [x for x in walk(p) if x['kind'] in ('Sentence','leaf')]
assert len(sentences)==27
# Exactly one natural sentence per body unit, independent of the empty wrapper title.
for s in sentences:
    n=len(readings); text=plain(s); del readings[n:]
    assert text.endswith('。') and text.count('。')==1
codes=[x['fields']['text'] for x in walk(tree) if x['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',addition_md.decode())
assert len(readings)==197 and len(paragraphs)==5 and len(codes)==2
save('paragraphs.txt', ('\n\n'.join(paragraphs)+'\n').encode())
save('readings.json',(json.dumps(readings,ensure_ascii=False,indent=2)+'\n').encode())
author=REPO/'.tmp/reader-addition'
am=(author/'manifest.json').read_bytes()
assert sha(am)=='49b69289c2cc43828cdf337c3c5cb84e2b28ab126d63d6d77bda8c3e638d91a5'
save('author-manifest.json',am)
for e in json.loads(am)['files']:
    b=(author/e['path']).read_bytes(); assert len(b)==e['bytes'] and sha(b)==e['sha256']
assert (author/'addition.nepld').read_bytes()==addition
assert (author/'addition.md').read_bytes()==addition_md
assert (author/'after.nepld').read_bytes()==after
diffcheck=subprocess.run(['git','-C',str(REPO),'diff','--check',base,HEAD],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
save('diffcheck.log',diffcheck.stdout); assert diffcheck.returncode==0
result=dict(head=HEAD,base=base,changed_paths=[path],before_sha256=sha(before),after_sha256=sha(after),source_sha256=sha(source),addition_sha256=sha(addition),old_prefix_and_closing_exact=True,source_unchanged=True,paragraphs=5,body_sentences=27,inline_code=codes,ruby_count=len(readings),ruby_unique=len(set(map(tuple,readings))),author_manifest_entries_verified=len(json.loads(am)['files']),full_structure_audit=True,production_runtime_executed=False,canonical_switched=False)
save('result.json',(json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode())
print(json.dumps(result,ensure_ascii=False,indent=2))
