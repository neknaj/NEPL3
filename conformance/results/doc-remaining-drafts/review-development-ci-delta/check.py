from pathlib import Path
import json,hashlib,subprocess,copy
root=Path(__file__).resolve().parent;repo=Path('C:/projects/NEPL3-doc-authoring-b')
def git(*args):return subprocess.check_output(['git','-C',str(repo),*args])
source='337057d71838c85a6fb9a304e3ec4164436bb310';draft='3887fffc9fc08ae449a374bb97ff301db7786df1';old='446000e65619bf6ddad5032094bd747ddf330d8f'
assert git('rev-parse',draft+'^').decode().strip()==old
assert git('diff','--name-only',source+'^',source).decode().splitlines()==['doc/development.md']
assert git('diff','--name-only',old,draft).decode().splitlines()==['doc/migration/authored/guide/development.nepld']
sources=[]
requests=[('old.md',source+'^','doc/development.md'),('new.md',source,'doc/development.md'),('ci.yml',source,'.github/workflows/ci.yml'),('old.nepld',old,'doc/migration/authored/guide/development.nepld'),('new.nepld',draft,'doc/migration/authored/guide/development.nepld')]
requests += [('context/'+p,source,p) for p in ['design/forms.json','tools/audit/structure.py']]
for name,rev,path in requests:
    b=git('show',rev+':'+path);f=root/name;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(b)
    sources.append({'path':name,'commit':git('rev-parse',rev).decode().strip(),'source_path':path,'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
assert sources[4]['sha256']=='eb3fefe2e22ce0116a9827d08e724569e57efb186a8e7051a36b21e2e3287bbe'
(root/'sources.json').write_bytes((json.dumps(sources,indent=2)+'\n').encode())
helper=(root.parent/'review-b-ruby/check.py').read_bytes();(root/'structure-helper.py').write_bytes(helper)
ns={'__file__':str(root/'structure-helper.py')};exec(helper.decode().split('results=[]\nfor chapter')[0],ns)
Parser,forms,norm,plain,walk,audit=[ns[k] for k in ['Parser','forms','normalize','plain','walk','audit_text']]
before=Parser((root/'old.nepld').read_text(encoding='utf-8'),forms).complete('Doc/Article');after=Parser((root/'new.nepld').read_text(encoding='utf-8'),forms).complete('Doc/Article');audit(after)
a=norm(before);b=norm(after,True)
oldmd=(root/'old.md').read_text(encoding='utf-8');newmd=(root/'new.md').read_text(encoding='utf-8')
oldp=[p for p in oldmd.split('\n\n') if p.startswith('WASI・ブラウザWasm・LSP・operation provider')];newp=[p for p in newmd.split('\n\n') if p.startswith('共通基盤のWASI試験、')]
assert len(oldp)==len(newp)==1
assert oldmd.replace(oldp[0],newp[0])==newmd
seen=[]
def replace(n):
    if isinstance(n,list):return [replace(x) for x in n]
    if isinstance(n,dict):
        if n.get('kind')=='Paragraph' and plain(n)==newp[0]:
            original=[p for p in walk(a) if p['kind']=='Paragraph' and plain(p)==oldp[0]]
            assert len(original)==1;seen.append(n);return original[0]
        return {k:replace(v) for k,v in n.items()}
    return n
assert replace(copy.deepcopy(b))==a and len(seen)==1
ci=(root/'ci.yml').read_text(encoding='utf-8')
for marker in ['--target wasm32-wasip2','--target wasm32-unknown-unknown','--target thumbv6m-none-eabi','firmware.py build','firmware.py execute','needs: [native, wasi, baremetal-build, baremetal-rp2040]']:
    assert marker in ci
result={'source_commit':source,'draft_commit':draft,'source_one_paragraph_only':True,'draft_one_paragraph_only':True,'new_draft_paragraph_base_text_exact_source':True,'other_draft_structure_preserved':True,'workflow_markers_confirmed':True,'production_runtime_executed':False}
(root/'results.json').write_bytes((json.dumps(result,indent=2)+'\n').encode());print(json.dumps(result,indent=2))
