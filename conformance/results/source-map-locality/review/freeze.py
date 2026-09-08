import pathlib,subprocess,hashlib,json,zipfile,io,shutil
p=pathlib.Path(__file__).resolve().parent;repo='C:/projects/NEPL3-source-map-locality';head='4b92e612a7bcbe0c147e88ca70b475bb6accb6fa';base='468ecbb7240a690d67b8dca613a9a8a52dba7c5f'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
assert git('show','-s','--format=%P',head).decode().strip()==base
raw=git('archive','--format=zip',head,'Cargo.toml','Cargo.lock','crates','tools','interfaces','design','languages','examples','doc','conformance/fixtures','conformance/targets/rp2040','AGENTS.md');rows=[]
with zipfile.ZipFile(io.BytesIO(raw)) as z:
 for f in z.infolist():
  if f.is_dir():continue
  b=z.read(f);path=p/'workspace'/f.filename;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b);rows.append(dict(path=f.filename,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
old=p/'before';old.mkdir(exist_ok=True);shutil.copytree(p/'workspace',old/'workspace',dirs_exist_ok=True)
changes=git('diff','--name-only',base,head).decode().splitlines();assert changes==['crates/foundation/core/src/origin.rs','crates/foundation/core/tests/maps.rs']
prior=[]
for path in changes:
 b=git('show',base+':'+path);(old/'workspace'/path).write_bytes(b)
 for root in [p,old]:
  q=root/'snapshot'/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes((root/'workspace'/path).read_bytes())
 prior.append(dict(path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(p/'sources.json').write_text(json.dumps(dict(head=head,base=base,archive_sha256=hashlib.sha256(raw).hexdigest(),files=rows),indent=2)+'\n',encoding='utf-8',newline='\n')
(old/'sources.json').write_text(json.dumps(dict(base=base,otherwise_identical_to=head,overrides=prior),indent=2)+'\n',encoding='utf-8',newline='\n')
(p/'changes.patch').write_bytes(git('diff','--binary',base,head))
for root in [p,old]:
 shutil.copyfile(p.parent/'review-source-map-adjacency/run.py',root/'run.py')
 shutil.copyfile(p.parent/'review-source-map-adjacency/review_adjacency.rs',root/'workspace/crates/foundation/core/tests/review_adjacency.rs')
print(head,len(rows))
