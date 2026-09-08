import pathlib,subprocess,json,hashlib
root=pathlib.Path(__file__).resolve().parents[1]
dest=root/'.tmp/measure'
dest.mkdir(exist_ok=True)
binary=root/'target/debug/nepl3-tools.exe'
record={'base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'diff_sha256':hashlib.sha256(subprocess.check_output(['git','diff'],cwd=root)).hexdigest(),'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(),'runs':[]}
for name in ['05-document','15-site','guide/development','guide/review']:
    source=root/f'doc/migration/authored/{name}.nepld'
    stem=name.replace('/','--')
    command=[str(binary),'doc-html','export',str(source),str(dest/f'{stem}-output')]
    log=dest/f'{stem}.log'
    with log.open('wb') as out:r=subprocess.run(command,cwd=root,stdout=out,stderr=subprocess.STDOUT,timeout=180)
    text=log.read_text(encoding='utf-8')
    record['runs'].append({'source':name,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'command':command,'exit_code':r.returncode,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
    (dest/'results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
    print(name,r.returncode,text[:220],flush=True)
