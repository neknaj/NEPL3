from pathlib import Path
import subprocess,json,hashlib
r=Path(__file__).parent;repo=r.parents[1];source=repo/'.tmp/reader-output-budget';exe=repo/'.tmp/review-output-budget/target/debug/nepl3-tools.exe';sha=lambda b:hashlib.sha256(b).hexdigest()
records=[]
for label in ['default','explicit-200m']:
 dest=r/label;dest.mkdir();
 for name in ['input.json','03-reader.nepld']:(dest/name).write_bytes((source/label/name).read_bytes())
 assert sha((dest/'03-reader.nepld').read_bytes())=='162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3'
 cmd=[str(exe),'doc-html','pages',str(dest/'input.json'),str(dest/'output')]
 result=subprocess.run(cmd,capture_output=True);(dest/'stdout.log').write_bytes(result.stdout);(dest/'stderr.log').write_bytes(result.stderr)
 records.append({'label':label,'command':cmd,'exit':result.returncode,'output_exists':(dest/'output').exists()})
 assert result.returncode==(1 if label=='default' else 0)
 if label=='default':assert not (dest/'output').exists() and b'WorkLimit' in result.stderr
output=r/'explicit-200m/output';rootoutput=source/'explicit-200m/output'
files=[]
for name in ['reference/reader/index.html','reference/reader/assets/doc.css','manifest.json']:
 data=(output/name).read_bytes();assert data==(rootoutput/name).read_bytes(),name
 files.append({'path':name,'bytes':len(data),'sha256':sha(data),'root_bytes_equal':True})
result={'source_commit':'b84e9a3854dd23538154194054c28713714ba123','binary_sha256':sha(exe.read_bytes()),'runs':records,'files':files}
(r/'results.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print(result)
