import pathlib,subprocess,json,hashlib
p=pathlib.Path(__file__).resolve().parent;repo='C:/projects/NEPL3-doc-signatures';head='03e9b1cc5826a1a3c05db484a25892d293c68978'
def git(*a):return subprocess.check_output(['git','-C',repo,*a])
base=git('rev-parse','c39ed0a').decode().strip();assert git('show','-s','--format=%P',head).decode().strip()==base
changed=git('diff','--name-only',base,head).decode().splitlines()
paths=set(changed)|{'AGENTS.md','design/forms.json','tools/audit/structure.py','doc/authoring.md','doc/spec/04-grammar.md','doc/spec/05-document.md','doc/spec/16-doc-migration.md','languages/doc/syntax.neplg'}
rows=[]
for path in sorted(paths):
 b=git('show',head+':'+path);q=p/'workspace'/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b);rows.append(dict(path=path,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(p/'sources.json').write_text(json.dumps(dict(head=head,base=base,changed=changed,files=rows),indent=2)+'\n',encoding='utf-8',newline='\n');(p/'changes.patch').write_bytes(git('diff',base,head));print(head,len(rows))
