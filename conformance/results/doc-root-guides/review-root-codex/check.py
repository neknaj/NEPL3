from pathlib import Path
import hashlib, json, re, subprocess, sys

repo = Path(__file__).resolve().parents[2]
out = Path(__file__).resolve().parent
head = 'd022e9bf84003e7272df86147a7afbee9220d316'
source_commit = 'b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2'
def git(*args): return subprocess.check_output(['git','-C',str(repo),*args])
paths = ['CODEX.md','AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py','doc/migration/authored/project/CODEX.nepld']
hashes=[]
for path in paths:
    raw=git('show',head+':'+path)
    p=out/'source'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(raw)
    hashes.append({'path':path,'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()})
assert git('show',head+':CODEX.md')==git('show',source_commit+':CODEX.md')
before=git('show','3ab81c3:CODEX.md')
after=git('show',source_commit+':CODEX.md')
bp=before.decode().split('\n\n');ap=after.decode().split('\n\n')
assert len(bp)==len(ap) and [i for i,(x,y) in enumerate(zip(bp,ap)) if x!=y]==[3]
(out/'canonical-before.md').write_bytes(before)
(out/'canonical.diff').write_bytes(git('diff','3ab81c3',source_commit,'--','CODEX.md'))
author=repo/'.tmp/author-root-codex'
manifest=(author/'manifest.json').read_bytes()
assert hashlib.sha256(manifest).hexdigest()=='6f5e474fff5d3aa22026e631bc5fc69a800113d314f541c30372ca606ad428d4'
for item in json.loads(manifest)['files']:
    raw=(author/item['path']).read_bytes()
    assert len(raw)==item['bytes'] and hashlib.sha256(raw).hexdigest()==item['sha256']
assert (author/'source.md').read_bytes()==after
assert (author/'draft.nepld').read_bytes()==git('show',head+':'+paths[-1])
(out/'original-author-manifest.json').write_bytes(manifest)
sys.path.insert(0,str(out/'source'))
from tools.audit.structure import Parser,load_forms
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
exec(helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')])
raw=(out/'source'/paths[-1]).read_bytes()
assert b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf')
tree=Parser(raw.decode(),load_forms(out/'source')).complete('Doc/Article')
audit_text(tree)
n=normalize(tree,True)
nodes=list(walk(n));paragraphs=[x for x in nodes if x['kind']=='Paragraph']
expected=[re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',p.replace('\n','')).replace('`','') for p in after.decode().strip().split('\n\n')[1:]]
actual=[plain(p) for p in paragraphs]
assert actual==expected
sentences=[p['fields']['items'] for p in paragraphs]
assert [len(s) for s in sentences]==[1,4,4,3,7]
assert all(plain(s).endswith('\u3002') and plain(s).count('\u3002')==1 for ss in sentences for s in ss)
code=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode']
assert code==['nepl3-design-2026-09-06-r4','design/tasks.json','doc/decisions/']
links=[(plain(x['fields']['label']),x['fields']['target']['fields']['path']) for x in nodes if x['kind']=='Link']
assert links==re.findall(r'\[([^\]]+)\]\(([^)]+)\)',after.decode())
assert not any(x['kind'] in ('RawCode','Table','Anno','Strong') for x in nodes)
assert plain(n['fields']['title'])==after.decode().splitlines()[0][2:]
result={'head':head,'canonical':source_commit,'source_hashes':hashes,'author_manifest_verified':True,
        'canonical_correction_only_paragraph':3,'paragraphs_exact':len(actual),'sentences':sum(map(len,sentences)),
        'sentence_counts':list(map(len,sentences)),'code_exact':code,'links_exact':links,'ruby_count':len(pairs),
        'unannotated_narrative_kanji':0,'utf8_lf':True,'production_execution':False,'independent_content_review':True}
(out/'paragraphs.txt').write_text('\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
