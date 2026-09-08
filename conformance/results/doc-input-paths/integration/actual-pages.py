import pathlib, subprocess, hashlib, json
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'.tmp/actual-pages'
out.mkdir()
def sha(data): return hashlib.sha256(data).hexdigest()
binary=root/'target/debug/nepl3-tools.exe'
record={'commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'binary_sha256':sha(binary.read_bytes()),'scope':'Two unchanged authored specifications; physical drafts with original logical .md paths. Default budgets. Not canonical migration or deployment.','sources':[]}
pages=[]
for name in ['12-model-invariants','19-html-fragment']:
    source=root/f'doc/migration/authored/{name}.nepld'
    data=source.read_bytes()
    tracked=subprocess.check_output(['git','show',f'HEAD:doc/migration/authored/{name}.nepld'],cwd=root)
    assert data==tracked
    physical=out/f'drafts/{name}.nepld'
    physical.parent.mkdir(exist_ok=True)
    physical.write_bytes(data)
    record['sources'].append({'path':str(source.relative_to(root)),'bytes':len(data),'sha256':sha(data)})
    pages.append({'id':name,'source':f'doc/spec/{name}.md','input':f'drafts/{name}.nepld','route':f'docs/{name}/index.html'})
manifest=out/'input.json'
manifest.write_text(json.dumps({'version':1,'pages':pages},indent=2)+'\n',encoding='utf-8')
command=[str(binary),'doc-html','pages',str(manifest),str(out/'output')]
with (out/'command.log').open('wb') as log:
    result=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT,timeout=180)
record.update(command=command,exit_code=result.returncode,log_sha256=sha((out/'command.log').read_bytes()))
if result.returncode==0:
    html=(out/'output/docs/12-model-invariants/index.html').read_text(encoding='utf-8')
    record['relative_href_preserved']='href="../19-html-fragment/index.html"' in html
    assert record['relative_href_preserved']
    record['files']=[{'path':str(p.relative_to(out)),'sha256':sha(p.read_bytes()),'bytes':p.stat().st_size} for p in sorted((out/'output').rglob('*')) if p.is_file()]
(out/'results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record,indent=2),flush=True)
raise SystemExit(result.returncode)
