import pathlib,hashlib,json,subprocess,io,zipfile
p=pathlib.Path(__file__).resolve().parent;repo='C:/projects/NEPL3-stream-digest';rev=subprocess.check_output(['git','-C',repo,'rev-parse','1de3e1c']).decode().strip()
raw=subprocess.check_output(['git','-C',repo,'archive','--format=zip',rev,'Cargo.toml','Cargo.lock','crates','tools','interfaces','design','languages','examples','doc','conformance/fixtures','conformance/targets/rp2040','AGENTS.md']);rows=[]
with zipfile.ZipFile(io.BytesIO(raw)) as z:
 for i in z.infolist():
  if i.is_dir():continue
  b=z.read(i);q=p/'workspace'/i.filename;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b);rows.append(dict(path=i.filename,bytes=len(b),sha256=hashlib.sha256(b).hexdigest()))
(p/'sources.json').write_text(json.dumps(dict(commit=rev,archive_sha256=hashlib.sha256(raw).hexdigest(),files=rows),indent=2)+'\n',encoding='utf-8',newline='\n')
(p/'changes.patch').write_bytes(subprocess.check_output(['git','-C',repo,'diff','--binary','3c42d75',rev]))
print(rev,len(rows))
