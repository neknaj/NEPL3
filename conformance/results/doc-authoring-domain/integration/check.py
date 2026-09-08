import hashlib, json, subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent
REPO = Path('C:/projects/NEPL3-doc-domain-integration')
AUDIT = Path('C:/projects/NEPL3-doc-authoring-b/.tmp/audit-domain-drift')
def sha(data): return hashlib.sha256(data).hexdigest()
def git(*args):
    return subprocess.check_output(['git', '-C', str(REPO), *args], timeout=60)
def save(name, data):
    p = OUT / name
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(data)
def verify_manifest(root, expected):
    data = (root / 'manifest.json').read_bytes()
    assert sha(data) == expected, root
    rows = json.loads(data)['files']
    for e in rows:
        b = (root / e['path']).read_bytes()
        assert len(b) == e['bytes'] and sha(b) == e['sha256'], (root,e['path'])
    return len(rows)

head = git('rev-parse','ce0e378').decode().strip()
base = git('rev-parse','7f250ee').decode().strip()
assert git('rev-parse','HEAD').decode().strip() == head
assert not git('status','--porcelain')
verify_manifest(AUDIT, '225d9d82eb9d2a32431c1429a7b463c59d85d8a11cbf81b7321e2a59468547d2')
audit = json.loads((AUDIT/'result.json').read_bytes())
assert audit['main'] == base
save('audit-manifest.json', (AUDIT/'manifest.json').read_bytes())
save('audit-results.json', (AUDIT/'result.json').read_bytes())
reviews=[]
for e in audit['verified_review_manifests']:
    root=Path(e['path'])
    count=verify_manifest(root,e['manifest_sha256'])
    assert count == e['files']
    reviews.append(dict(path=str(root),sha256=e['manifest_sha256'],entries=count))
changed=git('diff','--name-status',base,head).decode().splitlines()
expected=['A\t'+e['draft_path'] for e in audit['chapters']]
assert changed == expected, changed
rows=[]
for e in audit['chapters']:
    path=e['draft_path']
    data=git('show',head+':'+path)
    assert data == git('show',e['draft_commit']+':'+path)
    assert len(data)==e['draft_bytes'] and sha(data)==e['draft_sha256']
    n=path.split('/')[-1][:2]
    if n=='05': reviewed=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-sentence-payload/after.nepld')
    elif n=='09': reviewed=Path('C:/projects/NEPL3-runtime/.tmp/review-reader-portability-followup/09-portability/after.nepld')
    else: reviewed=Path('C:/projects/NEPL3-runtime/.tmp/review-doc-authoring-b-'+n+'-revision/after.nepld')
    assert data==reviewed.read_bytes(), reviewed
    src=git('show',base+':'+e['path'])
    assert src==git('show',head+':'+e['path'])
    assert sha(src)==e['current_sha256']
    origin={'05':'90f4552','09':'4e18cc1'}.get(n,e['baseline_commit'])
    assert src==git('show',origin+':'+e['path'])
    save('sources/'+n+'.diff',git('diff',e['baseline_commit'],base,'--',e['path']))
    rows.append(dict(path=path,commit=e['draft_commit'],sha256=sha(data),bytes=len(data),review_snapshot=str(reviewed),canonical_sha256=sha(src),canonical_review_source=origin))
save('changed.txt',git('diff','--name-status',base,head))
save('diff-check.log',git('diff','--check',base,head))
assert not git('status','--porcelain')
result=dict(head=head,base=base,result='passed',manuscripts=rows,review_manifests=reviews,only_five_added_manuscripts=True,all_other_tree_paths_identical=True)
save('result.json',(json.dumps(result,ensure_ascii=False,indent=2)+'\n').encode())
print(json.dumps(dict(result='passed',head=head,files=len(rows),bytes=sum(e['bytes'] for e in rows),review_entries=sum(e['entries'] for e in reviews))))
