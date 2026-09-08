from pathlib import Path
import subprocess,tarfile,io,json,hashlib
root=Path(__file__).resolve().parent;repo=Path('C:/projects/NEPL3-borrow-mapping-union')
head=subprocess.check_output(['git','-C',str(repo),'rev-parse','2b06d1c']).decode().strip()
target=root/'source';target.mkdir(exist_ok=True)
tar=tarfile.open(fileobj=io.BytesIO(subprocess.check_output(['git','-C',str(repo),'archive',head])))
for member in tar.getmembers():
    p=(target/member.name).resolve();assert p.is_relative_to(target.resolve()) and not member.issym() and not member.islnk()
tar.extractall(target,filter='data')
files=[]
for p in sorted(target.rglob('*')):
    if p.is_file():
        b=p.read_bytes();files.append({'path':str(p.relative_to(target)).replace('\\','/'),'sha256':hashlib.sha256(b).hexdigest(),'bytes':len(b)})
(root/'source-manifest.json').write_text(json.dumps({'commit':head,'files':files},indent=2)+'\n',encoding='utf-8')
(root/'delta.diff').write_bytes(subprocess.check_output(['git','-C',str(repo),'diff','0b4f7e3',head]))
print(head,len(files),str(target))
