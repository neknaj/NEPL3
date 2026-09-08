import difflib,hashlib,json,pathlib,shutil,subprocess,sys
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-doc-review-integration'
commit=subprocess.check_output(['git','-C',repo,'rev-parse','4b549f2']).decode().strip()
path='doc/migration/authored/guide/review.nepld'
b=subprocess.check_output(['git','-C',repo,'show',commit+':'+path])
assert hashlib.sha256(b).hexdigest()=='93219488d2b5b3c93d3b7c02a2f3598b5544d0fe61111b0537071b4354549bee'
old=(p/'snapshot'/path).read_bytes()
lines=old.splitlines(keepends=True)
needle=b'cons code "structure"'
ix=[i for i,l in enumerate(lines) if needle in l]
assert len(ix)==1 and ix[0]==1085
beforeline=lines[ix[0]]
suffix='cons text "した。" nil\n'.encode()
assert beforeline.endswith(suffix)
lines[ix[0]]=beforeline[:-len(suffix)]+'cons text "した。 " nil\n'.encode()
assert b==b''.join(lines), 'Root change must be exactly the preserved source space'
q=p/'after';q.mkdir(exist_ok=True)
for row in json.loads((p/'sources.json').read_text(encoding='utf-8'))['files']:
 src=p/'snapshot'/row['path'];target=q/'snapshot'/row['path'];target.parent.mkdir(parents=True,exist_ok=True)
 assert hashlib.sha256(src.read_bytes()).hexdigest()==row['sha256']
 target.write_bytes(b if row['path']==path else src.read_bytes())
helpers=[]
for name in ['annotation_parser.py','check.py','audit.py','run.py']:
 shutil.copyfile(p/name,q/name)
 digest=hashlib.sha256((p/name).read_bytes()).hexdigest()
 assert digest==hashlib.sha256((q/name).read_bytes()).hexdigest()
 helpers.append(dict(path=name,sha256=digest))
(q/'delta.patch').write_text(''.join(difflib.unified_diff(old.decode().splitlines(keepends=True),b.decode().splitlines(keepends=True),fromfile='7c6f29b/'+path,tofile=commit+'/'+path)),encoding='utf-8',newline='\n')
(q/'source.json').write_text(json.dumps(dict(repo=repo,commit=commit,path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest(),exact_delta='One U+0020 in structure sentence final Text, draft line1086',unchanged_helpers=helpers),indent=2)+'\n',encoding='utf-8',newline='\n')
r=subprocess.run([sys.executable,'run.py'],cwd=q,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=90)
(q/'driver.log').write_bytes(r.stdout);assert r.returncode==0
checks=json.loads((q/'checks.json').read_text(encoding='utf-8'))
assert not checks['paragraph_mismatches']
assert all(x['exit_code']==0 for x in json.loads((q/'execution.json').read_text(encoding='utf-8')))
print(commit,len(b),checks['paragraph_counts'], 'all commands PASS; original checker unchanged')
