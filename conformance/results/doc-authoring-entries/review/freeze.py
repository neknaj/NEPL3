import pathlib,subprocess,json,hashlib,shutil
p=pathlib.Path(__file__).resolve().parent;repo='C:/projects/NEPL3-doc-authoring-delivery'
raw=(pathlib.Path(repo)/'.tmp/history-migration-authoring-files.json').read_bytes();assert hashlib.sha256(raw).hexdigest()=='8efbc586243523de3b56725e61760573095736a1f5ddff0635e342bfa9f08b7a';(p/'author-manifest.json').write_bytes(raw);m=json.loads(raw);rows=[]
for row in m['files']:
 scope=row['draft_path'].split('/')[-2]
 for commit,path,expected in [(row['draft_commit'],row['draft_path'],row['draft_sha256']),(row['source_commit'],'doc/'+scope+'/README.md',row['source_sha256'])]:
  b=subprocess.check_output(['git','-C',repo,'show',commit+':'+path]);assert hashlib.sha256(b).hexdigest()==expected
  q=p/'snapshot'/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b);rows.append(dict(commit=commit,path=path,bytes=len(b),sha256=expected))
for path in ['AGENTS.md','doc/authoring.md','doc/spec/16-doc-migration.md','design/forms.json','languages/doc/syntax.neplg']:
 b=subprocess.check_output(['git','-C',repo,'show',m['source_commit']+':'+path]);q=p/'snapshot'/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b);rows.append(dict(commit=m['source_commit'],path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
shutil.copyfile(p.parent/'review-doc-authoring-foundation-progress/annotation_parser.py',p/'annotation_parser.py')
(p/'sources.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8',newline='\n');print('PASS',len(rows))
