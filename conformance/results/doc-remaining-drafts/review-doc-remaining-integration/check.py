from pathlib import Path
import subprocess,json,hashlib,difflib
R=Path(__file__).resolve().parent
repo=Path('C:/projects/NEPL3-doc-authoring-b')
def git(*a): return subprocess.check_output(['git','-C',str(repo),*a],timeout=60)
def sha(b):return hashlib.sha256(b).hexdigest()
def save(path,b):
 p=R/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
main=git('rev-parse','5a0b72f').decode().strip()
raw=Path('C:/projects/NEPL3-doc-guide-refresh/.tmp/authoring-refresh/drift.json').read_bytes()
save('author-drift-input.json',raw);cases=json.loads(raw)['cases'];assert len(cases)==8
manifests={};rows=[]
for c in cases:
 source=git('show',main+':'+c['source']);old=git('show',c['reviewed_source_commit']+':'+c['source'])
 assert source==old and sha(source)==c['current_source_sha256']
 draft=git('show',c['draft_commit']+':'+c['draft']);assert sha(draft)==c['draft_sha256']
 assert git('ls-tree','--name-only',main,'--',c['draft'])==b''
 assert b'\r' not in draft and not draft.startswith(b'\xef\xbb\xbf')
 save('sources/'+Path(c['source']).name,source);save('drafts/'+str(Path(c['draft']).relative_to('doc/migration/authored')).replace('\\','/'),draft)
 manifest=Path(c['review_manifest']);b=manifest.read_bytes();assert sha(b)==c['review_manifest_sha256']
 manifests[str(manifest)]=b
 rows.append(dict(source=c['source'],source_commit=c['reviewed_source_commit'],source_sha256=sha(source),source_bytes=len(source),draft=c['draft'],commit=c['draft_commit'],draft_sha256=sha(draft),draft_bytes=len(draft),absent_from_main=True,source_unchanged=True,git_blob=git('rev-parse',c['draft_commit']+':'+c['draft']).decode().strip(),commit_change=git('diff-tree','--no-commit-id','--name-status','-r',c['draft_commit']).decode().strip()))
# Development's initial review deliberately held a stale source claim. Require
# the separately reviewed correction, not merely the initial broad review.
extra=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-development-ci-delta/manifest.json')
manifests[str(extra)]=extra.read_bytes()
verified=[];allpayloads=[]
for path,raw in manifests.items():
 p=Path(path);m=json.loads(raw);files=m['files'];entries=[]
 for f in files:
  b=(p.parent/f['path']).read_bytes();assert len(b)==f['bytes'] and sha(b)==f['sha256']
  entries.append(dict(path=f['path'],bytes=len(b),sha256=sha(b)))
  allpayloads.append((p.parent.name,f['path'],b))
 save('reviews/'+p.parent.name+'/manifest.json',raw)
 save('reviews/'+p.parent.name+'/record.md',(p.parent/'record.md').read_bytes())
 verified.append(dict(source=str(p),sha256=sha(raw),entries=len(files),bytes=sum(f['bytes'] for f in files)))
for row in rows:
 draft=git('show',row['commit']+':'+row['draft']);source=git('show',main+':'+row['source'])
 assert any(b==draft for _,_,b in allpayloads),row['draft']
 assert any(b==source for _,_,b in allpayloads),row['source']
assert git('show',main+':doc/authoring.md')==git('show','90f4552:doc/authoring.md')
assert git('show',main+':design/forms.json')==git('show','90f4552:design/forms.json')
save('current-AGENTS.md',git('show',main+':AGENTS.md'))
save('current-authoring.md',git('show',main+':doc/authoring.md'))
save('development-source-correction.diff',git('diff','6769420','337057d','--','doc/development.md'))
save('development-draft-correction.diff',git('diff','446000e','3887fff','--','doc/migration/authored/guide/development.nepld'))
save('invariants-source-correction.diff',git('diff','9dd5a5c','5657677','--','doc/spec/12-model-invariants.md'))
result=dict(main=main,cases=rows,review_manifests=verified,review_payloads_checked=sum(r['entries'] for r in verified),current_authoring_policy_byte_equal=True,current_forms_byte_equal=True,all_eight_current_sources_equal_reviewed=True,all_eight_draft_blobs_equal_reviewed=True,production_or_manuscript_edited=False)
save('result.json',(json.dumps(result,indent=2)+'\n').encode())
print(json.dumps(result,indent=2))
