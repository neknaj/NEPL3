from pathlib import Path
import subprocess, hashlib, json, re, importlib.util, difflib
R = Path(__file__).resolve().parent
REPO = Path('C:/projects/NEPL3-doc-input-paths')
HEAD = 'a6e304cd02ebee9f279c05a9e26b9ce617e727d8'
def git(*args):
    return subprocess.check_output(['git', '-C', str(REPO), *args], timeout=60)
def sha(b): return hashlib.sha256(b).hexdigest()
def save(name, data):
    p = R/name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(data)
def blob(rev, path, name):
    b = git('show', rev+':'+path); save(name,b); return b
base = git('rev-parse',HEAD+'^').decode().strip()
assert base == 'a604864a16bb6db33036d019b2ee4d8a99404b78'
path = 'doc/migration/authored/21-doc-pages.nepld'
assert git('diff','--name-only',base,HEAD).decode().splitlines() == [path]
before = blob(base,path,'before.nepld'); after = blob(HEAD,path,'after.nepld')
source = blob(base,'doc/spec/21-doc-pages.md','source.md')
prior = blob(base+'^','doc/spec/21-doc-pages.md','previous-source.md')
assert source == git('show',HEAD+':doc/spec/21-doc-pages.md')
def insertion(old,new):
 a=old.splitlines(keepends=True);b=new.splitlines(keepends=True)
 diffs=[op for op in difflib.SequenceMatcher(None,a,b,autojunk=False).get_opcodes() if op[0]!='equal']
 assert len(diffs)==1 and diffs[0][0]=='insert',diffs
 _,i,j,k,l=diffs[0];assert i==j
 added=b''.join(b[k:l]);assert old[:sum(map(len,a[:i]))]+added+old[sum(map(len,a[:i])):]==new
 return added
addition_md = insertion(prior,source)
addition = insertion(before,after)
save('raw-insertion.nepld', addition)
# Diff matched the repeated closing nil at the opposite edge. Move that exact line to close the isolated slice.
line=b'        nil\n'
assert addition.startswith(line)
addition=addition[len(line):]+line
assert after.replace(addition,b'',1)==before
assert len(addition)==4364
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
assert len(sentences)==13
# Exactly one natural sentence per body unit, independent of the empty wrapper title.
for s in sentences:
    n=len(readings); text=plain(s); del readings[n:]
    assert text.endswith('。') and text.count('。')==1
codes=[x['fields']['text'] for x in walk(tree) if x['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',addition_md.decode())
assert len(paragraphs)==2 and len(codes)==10
save('paragraphs.txt', ('\n\n'.join(paragraphs)+'\n').encode())
save('readings.json',(json.dumps(readings,ensure_ascii=False,indent=2)+'\n').encode())
author=REPO/'.tmp/author-input-paths'
am=(author/'manifest.json').read_bytes()
assert sha(am)=='9e105a02f276bd2f9dc23936dc8eec2fee1b2ca615e598a3b1f9a31f218f64da'
save('author-manifest.json',am)
for e in json.loads(am)['files']:
    b=(author/e['path']).read_bytes(); assert len(b)==e['bytes'] and sha(b)==e['sha256']
assert (author/'addition.nepld').read_bytes()==addition
assert (author/'source.md').read_bytes()==source
assert (author/'after.nepld').read_bytes()==after
diffcheck=subprocess.run(['git','-C',str(REPO),'diff','--check',base,HEAD],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60)
save('diffcheck.log',diffcheck.stdout); assert diffcheck.returncode==0
result=dict(head=HEAD,base=base,changed_paths=[path],before_sha256=sha(before),after_sha256=sha(after),source_sha256=sha(source),addition_sha256=sha(addition),old_prefix_and_closing_exact=True,source_unchanged=True,paragraphs=2,body_sentences=13,inline_code=codes,ruby_count=len(readings),ruby_unique=len(set(map(tuple,readings))),author_manifest_entries_verified=len(json.loads(am)['files']),full_structure_audit=True,production_runtime_executed=False,canonical_switched=False)
save('result.json',(json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode())
print(json.dumps(result,ensure_ascii=False,indent=2))
