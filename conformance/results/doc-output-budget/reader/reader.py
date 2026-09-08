import pathlib,json,subprocess,hashlib
root=pathlib.Path(__file__).resolve().parents[1]
out=root/'.tmp/reader-output-budget';out.mkdir(parents=True,exist_ok=True)
binary=root/'target/debug/nepl3-tools.exe'
source=pathlib.Path('C:/projects/NEPL3-doc-reader-recovery/doc/migration/authored/03-reader.nepld').read_bytes()
assert hashlib.sha256(source).hexdigest()=='162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3'
results={'source_sha256':hashlib.sha256(source).hexdigest(),'source_bytes':len(source),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'runs':[]}
for label,limits in [('default',None),('explicit-200m',{'source_bytes':10000000,'work':200000000,'depth':1000,'nodes':10000000,'allocation_units':500000000,'output_bytes':10000000,'diagnostics':1000,'events':1000})]:
    directory=out/label;directory.mkdir(parents=True,exist_ok=True)
    (directory/'03-reader.nepld').write_bytes(source)
    manifest={'version':1,'pages':[{'id':'candidate','source':'03-reader.nepld','route':'reference/reader/index.html'}]}
    if limits is not None:manifest['output_limits']=limits
    path=directory/'input.json';path.write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    command=[str(binary),'doc-html','pages',str(path),str(directory/'output')]
    with (directory/'command.log').open('wb') as log:
        result=subprocess.run(command,cwd=root,stdout=log,stderr=subprocess.STDOUT,timeout=180)
    results['runs'].append({'label':label,'command':command,'exit_code':result.returncode,'log_sha256':hashlib.sha256((directory/'command.log').read_bytes()).hexdigest()})
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
    print(label,result.returncode,(directory/'command.log').read_text(encoding='utf-8')[-800:],flush=True)
