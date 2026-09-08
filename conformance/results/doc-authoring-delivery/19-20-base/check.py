import pathlib,subprocess,json,hashlib
p=pathlib.Path(__file__).resolve().parent
repo='C:/projects/NEPL3-doc-authoring-delivery'
rows=[]
for path in ['doc/spec/19-html-fragment.md','doc/spec/20-doc-html.md']:
    cols=[]
    for rev in ['c90d306','0741866']:
        commit=subprocess.check_output(['git','-C',repo,'rev-parse',rev]).decode().strip()
        raw=subprocess.check_output(['git','-C',repo,'show',commit+':'+path])
        blob=subprocess.check_output(['git','-C',repo,'rev-parse',commit+':'+path]).decode().strip()
        cols.append(dict(commit=commit,blob=blob,sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw)))
    assert cols[0]['blob']==cols[1]['blob'] and cols[0]['sha256']==cols[1]['sha256']
    rows.append(dict(path=path,versions=cols))
(p/'result.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8',newline='\n')
print('Both original Markdown files are byte-identical across c90d306 and authored source main0741866.')
