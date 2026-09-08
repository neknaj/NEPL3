from pathlib import Path
import subprocess, tarfile, hashlib, json
R=Path(__file__).resolve().parent
repo=R.parents[1]
revs={'before':'e568d95574de27c23ee8e533f7ce5d211af4ecba','after':'889c5a40c4cb8226d3edbbc70b418ebca9512733'}
records=[]
for label,rev in revs.items():
    archive=R/(label+'.tar')
    with archive.open('wb') as out:subprocess.run(['git','-C',str(repo),'archive',rev],stdout=out,check=True)
    workspace=R/label;workspace.mkdir(exist_ok=True)
    with tarfile.open(archive) as t:t.extractall(workspace,filter='data')
    files=[]
    for p in sorted(workspace.rglob('*')):
        if p.is_file():
            b=p.read_bytes();files.append({'path':p.relative_to(workspace).as_posix(),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()})
    (R/(label+'-files.json')).write_text(json.dumps(files,indent=2)+'\n',encoding='utf8')
    records.append({'label':label,'commit':rev,'archive_sha256':hashlib.sha256(archive.read_bytes()).hexdigest(),'files':len(files)})
inputs={
 'linear':'examples/document/linear-combination.nepld',
 '05':'doc/migration/authored/05-document.nepld',
 '15':'doc/migration/authored/15-site.nepld',
 'development':'doc/migration/authored/guide/development.nepld',
 'review':'doc/migration/authored/guide/review.nepld'}
(R/'inputs').mkdir(exist_ok=True)
for key,path in inputs.items():
    b=(R/'before'/path).read_bytes();assert b==(R/'after'/path).read_bytes()
    (R/'inputs'/(key+'.nepld')).write_bytes(b)
(R/'snapshots.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf8')
print(json.dumps(records))
