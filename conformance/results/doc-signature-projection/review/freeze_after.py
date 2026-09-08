import pathlib,subprocess,shutil,json,hashlib
p=pathlib.Path(__file__).resolve().parent;after=p/'after';after.mkdir(exist_ok=True);repo='C:/projects/NEPL3-doc-signatures';head='8bb7a972aa66633da6fb7a38026fb9c843e57329';before='03e9b1cc5826a1a3c05db484a25892d293c68978'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
paths=git('diff','--name-only',before,head).decode().splitlines();assert paths==['tools/generate/signatures.py','tools/generate/test_signatures.py']
shutil.copytree(p/'workspace',after/'workspace',dirs_exist_ok=True);shutil.copyfile(p/'check.py',after/'check.py');rows=[]
for path in paths:
 b=git('show',head+':'+path);(after/'workspace'/path).write_bytes(b);rows.append(dict(path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(after/'sources.json').write_text(json.dumps(dict(head=head,before=before,overrides=rows,otherwise_identical=True),indent=2)+'\n',encoding='utf-8',newline='\n');(after/'changes.patch').write_bytes(git('diff',before,head));print(head)
