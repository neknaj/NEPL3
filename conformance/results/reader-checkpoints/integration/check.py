import pathlib,subprocess,json,hashlib,io,zipfile
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-reader-checkpoints'
old=p.parent/'review-reader-checkpoints'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
start=git('rev-parse','92d7cd2').decode().strip();fixed=git('rev-parse','0c583b7').decode().strip();merged=git('rev-parse','7541587').decode().strip();head=git('rev-parse','f7efb1c').decode().strip();main=git('rev-parse','6769420').decode().strip()
patch=git('diff','--binary','--no-ext-diff',start,fixed,'--');assert patch
(p/'92d7cd2-to-0c583b7.patch').write_bytes(patch)
paths=git('diff','--name-only',start,fixed).decode().splitlines()
delta=json.loads((old/'after/delta.json').read_text(encoding='utf-8'))
assert set(paths)=={r['path'] for r in delta['files']}
for r in delta['files']:
 b=git('show',fixed+':'+r['path']);assert hashlib.sha256(b).hexdigest()==r['sha256'] and len(b)==r['bytes']
assert (old/'after/delta.patch').read_bytes()==b''
arch={}
with zipfile.ZipFile(io.BytesIO(git('archive','--format=zip',head,'conformance/results/reader-checkpoints/review'))) as z:
 for i in z.infolist():
  if not i.is_dir():arch[i.filename]=z.read(i)
rows=[];refs=[];prefix='conformance/results/reader-checkpoints/review/'
for mp in ['manifest-before.json','after/manifest.json']:
 raw=(old/mp).read_bytes();m=json.loads(raw);refs.append(dict(path=mp,sha256=hashlib.sha256(raw).hexdigest()))
 entries=[dict(path=pathlib.Path(mp).name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest()),*m['files']]
 parent=pathlib.Path(mp).parent
 for e in entries:
  rel=(parent/e['path'].replace('\\','/')).as_posix();b=arch[prefix+rel]
  assert b==(old/rel).read_bytes() and len(b)==e['bytes'] and hashlib.sha256(b).hexdigest()==e['sha256'],rel
  rows.append(dict(path=prefix+rel,bytes=len(b),sha256=e['sha256']))
assert len(rows)==63 and sum(r['bytes'] for r in rows)==744545
assert set(arch)=={r['path'] for r in rows}
assert set(git('diff','--name-only',merged,head).decode().splitlines())=={r['path'] for r in rows}|{'.gitattributes'}
assert git('show',head+':.gitattributes')==git('show',merged+':.gitattributes')+b'/conformance/results/reader-checkpoints/** -text -whitespace\n'
assert git('rev-list','--parents','-n','1',merged).decode().split()==[merged,fixed,main]
incoming=git('diff','--name-only',fixed,merged).decode().splitlines()
assert all(x=='.gitattributes' or x=='doc/migration/authored/18-html-delivery.nepld' or x.startswith('conformance/results/doc-authoring-18/') for x in incoming)
for path in incoming:
 assert git('show',merged+':'+path)==git('show',main+':'+path),path
unchanged={}
for path in ['crates','tools','interfaces','design','doc/spec','implementation-status.json','Cargo.toml','Cargo.lock','.github']:
 ids=[git('rev-parse',c+':'+path).decode().strip() for c in [fixed,merged,head]];assert len(set(ids))==1,path;unchanged[path]=ids[0]
result=dict(start=start,fixed=fixed,merged=merged,head=head,main=main,patch=dict(bytes=len(patch),sha256=hashlib.sha256(patch).hexdigest(),paths=paths),original_manifests=refs,archived=rows,archive_bytes=sum(r['bytes'] for r in rows),incoming_main_paths=incoming,unchanged_git_ids=unchanged)
(p/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(dict(head=head,patch=result['patch'],archived_files=len(rows),archived_bytes=result['archive_bytes'],incoming_paths=len(incoming),production_unchanged=True),indent=2))
