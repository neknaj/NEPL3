from pathlib import Path
import subprocess,json,hashlib,difflib,sys,re
from bs4 import BeautifulSoup
import mistune
p=Path(__file__).resolve().parent;root=p.parents[1]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
exec(compile((p/'helpers.py').read_text(encoding='utf-8'),'helpers.py','exec'))
commit='75e73a79022e238ac7355c37588c37b910759398'
author=root/'.tmp/author-extension-deltas'
mbytes=(author/'manifest.json').read_bytes()
assert hashlib.sha256(mbytes).hexdigest()=='bc1c644b09ce2f98c25764f152668091e78dd0873ee817330cc13028ba97d359'
m=json.loads(mbytes)
for f in m['files']:
    b=(author/f['path']).read_bytes();assert len(b)==f['bytes'] and hashlib.sha256(b).hexdigest()==f['sha256'],f['path']
(p/'author-manifest.json').write_bytes(mbytes)
for name in ['runtime.json','guide.log','01.log','11.log']:(p/('author-'+name)).write_bytes((author/name).read_bytes())
def git(ref,path):return subprocess.check_output(['git','show',ref+':'+path],cwd=root)
def visible(n):
    if isinstance(n,list):return ''.join(visible(x) for x in n)
    if not isinstance(n,dict):return ''
    k=n['kind'];f=n['fields']
    if k=='Link':return visible(f['label'])
    if k in ('Text','InlineCode'):return f['text']
    return ''.join(visible(x) for x in f.values())
results=[]
for short,draft,source in [('guide','guide/README','doc/README.md'),('01','01-architecture','doc/spec/01-architecture.md'),('11','11-conformance','doc/spec/11-conformance.md')]:
    path='doc/migration/authored/'+draft+'.nepld'
    before=git(commit,path);after=(root/path).read_bytes()
    assert before==(author/'before'/path).read_bytes()
    assert after==(author/'after'/path).read_bytes()
    old=git(commit+'^',source);new=git(commit,source)
    # Author's before/ directory holds pre-edit drafts but already-updated
    # canonical inputs. The true canonical parent is obtained from Git here.
    assert new==(author/'before'/source).read_bytes()
    for name,b in [('draft-before',before),('draft-after',after),('source-before',old),('source-after',new)]:
        (p/(short+'-'+name+'.txt')).write_bytes(b)
    additions=[]
    aa=before.decode('utf-8').splitlines(True);bb=after.decode('utf-8').splitlines(True)
    for tag,a,b,c,d in difflib.SequenceMatcher(None,aa,bb,autojunk=False).get_opcodes():
        assert tag in ('equal','insert'),(short,tag)
        if tag=='insert':additions.extend(bb[c:d])
    (p/(short+'-draft.patch')).write_text(''.join(difflib.unified_diff(aa,bb)),encoding='utf-8',newline='\n')
    srcadd=[]
    for tag,a,b,c,d in difflib.SequenceMatcher(None,old.decode().splitlines(True),new.decode().splitlines(True),autojunk=False).get_opcodes():
        assert tag in ('equal','insert')
        if tag=='insert':srcadd.extend(new.decode().splitlines(True)[c:d])
    Parser(after.decode(),load_forms(root)).complete('Doc/Article')
    wrapped='article ja "review" body '+('cons list ordered 1 ' if short=='guide' else '')+''.join(additions)+(' nil' if short=='guide' else '')+' nil'
    tree=Parser(wrapped,load_forms(root)).complete('Doc/Article')
    audit_text(tree);pairs.clear();base=normalize(tree,True)
    bodytext=visible(base['fields']['body'])
    html=BeautifulSoup(mistune.html(''.join(srcadd)),'html.parser')
    expected=html.get_text().strip().replace('\n','')
    assert bodytext==expected,(short,bodytext,expected)
    links=[(n['fields']['target']['fields'],visible(n['fields']['label'])) for n in walk(base) if n['kind']=='Link']
    source_links=[(a['href'],a.get_text()) for a in html.find_all('a')]
    assert [(d['path'],label) for d,label in links]==source_links,links
    results.append({'name':short,'draft':path,'source':source,'source_sha256':hashlib.sha256(new).hexdigest(),'draft_sha256':hashlib.sha256(after).hexdigest(),'inserted_lines':len(additions),'ruby_count':len(pairs),'readings':list(pairs),'visible_added_text':bodytext,'links':links,'insert_only':True,'original_all_bytes_preserved':True,'formal_structure':True,'added_text_exact':True})
(p/'result.json').write_text(json.dumps({'source_commit':commit,'author_manifest_payloads_verified':len(m['files']),'results':results,'independent_runtime':False,'author_runtime':'NeedsResolution, not HTML success','canonical_cutover':False},ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'results':[{k:v for k,v in x.items() if k not in ('visible_added_text','readings')} for x in results]},ensure_ascii=True))
