from pathlib import Path
import subprocess,os,json,time,sys,tarfile,io,hashlib,shutil
r=Path(__file__).resolve().parent
mode=sys.argv[1]
env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
if mode=='before-freeze':
    repo='C:/projects/NEPL3-borrow-mapping-union'
    commit=subprocess.check_output(['git','-C',repo,'rev-parse','0b4f7e3']).decode().strip()
    dest=r/'before-source';dest.mkdir(exist_ok=True)
    tar=tarfile.open(fileobj=io.BytesIO(subprocess.check_output(['git','-C',repo,'archive',commit])))
    for m in tar.getmembers():
        assert (dest/m.name).resolve().is_relative_to(dest.resolve()) and not m.issym() and not m.islnk()
    tar.extractall(dest,filter='data')
    fs=[]
    for p in sorted(dest.rglob('*')):
        if p.is_file():
            b=p.read_bytes();fs.append(dict(path=p.relative_to(dest).as_posix(),bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
    (r/'before-source-manifest.json').write_text(json.dumps(dict(commit=commit,files=fs),indent=2)+'\n',encoding='utf-8')
    shutil.copytree(r/'probe',r/'before-probe',dirs_exist_ok=True)
    p=r/'before-probe/Cargo.toml';p.write_text(p.read_text(encoding='utf-8').replace('../source/','../before-source/'),encoding='utf-8')
    print(commit,len(fs));sys.exit(0)
if mode.startswith('managed'):
    cmd=['cargo','test','--locked','-p','nepl3-core','-p','nepl3-reader','--target-dir',str(r/('target-wasi' if 'wasi' in mode else 'target-native'))]
    cwd=r/'source'
else:
    before=mode.startswith('before')
    cmd=['cargo','test','--locked','--manifest-path',str(r/('before-probe' if before else 'probe')/'Cargo.toml'),'--test','union','independent_union','--target-dir',str(r/(mode+'-target'))]
    cwd=r
if 'wasi' in mode:cmd+=['--target','wasm32-wasip2']
cmd+=['--','--test-threads=1','--nocapture']
start=time.time()
with (r/(mode+'.log')).open('wb') as f:
    ret=subprocess.run(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT).returncode
record=dict(command=cmd,cwd=str(cwd),exit_code=ret,elapsed_seconds=time.time()-start)
(r/(mode+'.json')).write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record));print((r/(mode+'.log')).read_text(encoding='utf-8')[-3500:]);sys.exit(ret)
