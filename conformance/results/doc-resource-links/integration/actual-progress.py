import pathlib,subprocess,json,hashlib
root=pathlib.Path(__file__).resolve().parents[1]
dest=root/'.tmp/actual-progress';dest.mkdir(exist_ok=True)
def sha(data):return hashlib.sha256(data).hexdigest()
binary=root/'target/debug/nepl3-tools.exe'
subprocess.run(['cargo','build','--locked','-p','nepl3-tools'],cwd=root,check=True)
inputs=[]
def copy(original,name):
 data=(root/original).read_bytes();(dest/name).write_bytes(data)
 inputs.append({'original':original,'stored':name,'bytes':len(data),'sha256':sha(data)})
copy('doc/migration/authored/progress/foundation-runtime.nepld','progress.nepld')
files=[]
for index,path in enumerate(['conformance/results/foundation-slice/validation.json','conformance/results/reader-builtins/validation.json','conformance/results/foundation-slice/main-ci.json']):
 name=f'file-{index}.json';copy(path,name)
 files.append({'id':f'file{index}','source':path,'input':name,'route':f'evidence/{name}'})
limits={'source_bytes':10000000,'work':400000000,'depth':1000,'nodes':20000000,'allocation_units':1500000000,'output_bytes':10000000,'diagnostics':1000,'events':1000}
manifest={'version':1,'pages':[{'id':'progress','source':'doc/progress/foundation-runtime.md','input':'progress.nepld','route':'docs/progress/index.html'}],'files':files,'output_limits':limits}
(dest/'input.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
record={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'binary_sha256':sha(binary.read_bytes()),'inputs':inputs,'scope':'Actual unchanged authored draft and three registered original JSON files. Parse/lower defaults; explicitly preselected finite output allowance. Not canonical migration or browser/deploy acceptance.'}
command=[str(binary),'doc-html','pages',str(dest/'input.json'),str(dest/'output')]
with (dest/'command.log').open('wb') as log:
 result=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT,timeout=300)
record.update(command=command,exit_code=result.returncode,log_sha256=sha((dest/'command.log').read_bytes()))
if result.returncode==0:
 for file in files:assert (dest/'output'/file['route']).read_bytes()==(dest/file['input']).read_bytes()
 html=(dest/'output/docs/progress/index.html').read_text(encoding='utf-8')
 for file in files:assert f'href="../../{file["route"]}"' in html
 record['files']=[{'path':p.relative_to(dest/'output').as_posix(),'bytes':len(p.read_bytes()),'sha256':sha(p.read_bytes())} for p in (dest/'output').rglob('*') if p.is_file()]
(dest/'results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
print('actual progress export',result.returncode,flush=True)
