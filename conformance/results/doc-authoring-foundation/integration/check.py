import hashlib,json,subprocess
from pathlib import Path
out=Path(__file__).resolve().parent
repo=out.parents[1]
def git(*a):return subprocess.check_output(['git',*a],cwd=repo)
def blob(c,p):return git('show',c+':'+p)
def sha(b):return hashlib.sha256(b).hexdigest()
def full(c):return git('rev-parse',c).decode().strip()
main=full('ff06696'); base=full('bc1d5d3')
chapters=[('00-contract','cd451bf'),('01-architecture','cd451bf'),('02-foundation','af3b72b'),('03-reader','80062fe'),('04-grammar','07a5672')]
rows=[]
for name,ref in chapters:
    commit=full(ref); p='doc/spec/'+name+'.md'; d='doc/migration/authored/'+name+'.nepld'
    original=blob(base,p); current=blob(main,p); draft=blob(commit,d)
    work=Path('C:/projects/NEPL3-doc-authoring-delivery' if name.startswith('03') else 'C:/projects/NEPL3-doc-authoring-a')
    assert (work/d).read_bytes()==draft
    if name.startswith(('00','01','02')): assert original==current
    if name.startswith('03'):
        assert current==blob('4e18cc1',p)
        assert draft==Path('C:/projects/NEPL3-runtime/.tmp/review-reader-portability-followup/03-reader/after.nepld').read_bytes()
    if name.startswith('04'):
        assert current==blob('2ea3c47',p)
        assert draft==Path('C:/projects/NEPL3-runtime/.tmp/review-doc-authoring-04/snapshot/'+d).read_bytes()
    delta=git('diff',base,main,'--',p)
    (out/(name+'.diff')).write_bytes(delta)
    rows.append(dict(path=p,baseline_commit=base,baseline_sha256=sha(original),current_commit=main,current_sha256=sha(current),draft_commit=commit,draft_path=d,draft_sha256=sha(draft),draft_bytes=len(draft),source_drift=bool(delta),manual_update_required=False))
reviews=[]
for name in ['review-doc-authoring-a','review-doc-authoring-a/revision-cd451bf','review-doc-authoring-02','review-doc-authoring-04','review-reader-portability-followup']:
    p=Path('C:/projects/NEPL3-runtime/.tmp')/name
    b=(p/'manifest.json').read_bytes(); m=json.loads(b)
    for item in m['files']:
        data=(p/item['path']).read_bytes()
        assert sha(data)==item['sha256'],str(p/item['path'])
        if 'bytes' in item: assert len(data)==item['bytes']
    if name=='review-reader-portability-followup': assert sha(b)=='be3860e631778b56404f39e2965d0761b2ba12ba7f7d451111211764e6decaa4'
    reviews.append(dict(path=str(p),manifest_sha256=sha(b),files=len(m['files'])))
p=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/reader-portability-followup/manifest.json')
assert sha(p.read_bytes())=='9560f16dbec86b8e23642df62aebe82ddd5e783ac5866cfb991792c02007bc22'
for item in json.loads(p.read_bytes())['evidence']:
    data=(p.parent/item['path']).read_bytes()
    assert sha(data)==item['sha256'] and len(data)==item['bytes']
for item in json.loads(p.read_bytes())['drafts']:
    data=blob(item['commit'],item['path'])
    assert sha(data)==item['sha256'] and len(data)==item['bytes']
original03=Path('C:/projects/NEPL3-doc-authoring-b/.tmp/review-author-a-6318187')
for item in json.loads((original03/'sources.json').read_bytes())['files']:
    assert sha((original03/item['path']).read_bytes())==item['sha256']
assert (original03/'doc/migration/authored/03-reader.nepld').read_bytes()==blob('6318187','doc/migration/authored/03-reader.nepld')
integration=full('e5be7d7')
def tree(c):
    return {p.decode():m.decode() for row in git('ls-tree','-rz','--full-tree',c).split(b'\0') if row for m,p in [row.split(b'\t',1)]}
mt,it=tree(main),tree(integration)
paths={r['draft_path'] for r in rows}
assert set(it)==set(mt)|paths and not paths.intersection(mt)
assert {p for p in it.keys()|mt.keys() if it.get(p)!=mt.get(p)}==paths
assert all(it[p]==mt[p] for p in mt)
for row in rows:assert blob(integration,row['draft_path'])==blob(row['draft_commit'],row['draft_path'])
delivery=Path('C:/projects/NEPL3-doc-foundation-integration')
for item in json.loads((delivery/'.tmp/manuscript-inputs.json').read_bytes()):
    data=blob(integration,item['path'])
    assert data==blob(item['source_commit'],item['path'])== (delivery/item['path']).read_bytes()
    assert sha(data)==item['sha256'] and len(data)==item['bytes']
git('diff','--check',main,integration)
result=dict(main=main,integration=integration,unchanged_main_entries=len(mt),changed_paths=sorted(paths),chapters=rows,verified_review_manifests=reviews,original03_review=str(original03),result='passed',conclusion='No unincorporated source drift for the selected drafts. 03 includes the checkpoint paragraph; 04 includes borrowed syntax proof paragraph. Integration adds only these exact five selected draft blobs.',limits='Read-only source drift and review provenance audit. No new full semantic or runtime/browser/anchor review.')
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(result,indent=2))
