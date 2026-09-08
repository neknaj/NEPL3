import hashlib,json,pathlib,subprocess
p=pathlib.Path(__file__).resolve().parent
repo=pathlib.Path('C:/projects/NEPL3-doc-authoring-c')
rev=subprocess.check_output(['git','rev-parse','d271484'],cwd=repo).decode().strip()
base=subprocess.check_output(['git','rev-parse','f03220e'],cwd=repo).decode().strip()
names=subprocess.check_output(['git','diff-tree','--no-commit-id','--name-only','-r',rev],cwd=repo).decode().splitlines()
print(names)
authored=next(x for x in names if x.endswith('.nepld'))
rows=[]
for source,path in [(rev,authored),(base,'doc/spec/18-html-delivery.md'),(rev,'doc/authoring.md'),(rev,'AGENTS.md'),(rev,'languages/doc/syntax.neplg'),(rev,'design/forms.json')]:
    data=subprocess.check_output(['git','show',source+':'+path],cwd=repo)
    f=p/'snapshot'/path;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(data)
    rows.append({'commit':source,'path':path,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data)})
(p/'sources.json').write_text(json.dumps({'commit':rev,'original_spec_commit':base,'files':rows},indent=2)+'\n',encoding='utf-8',newline='\n')
print(rev)
