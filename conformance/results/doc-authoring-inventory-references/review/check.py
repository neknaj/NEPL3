from pathlib import Path
import subprocess,hashlib,json,re,collections
root=Path(__file__).resolve().parent
repo=Path('C:/projects/NEPL3-doc-authoring-b')
source=subprocess.check_output(['git','-C',str(repo),'rev-parse','337057d']).decode().strip()
cases=[('inventory','074af5f','doc/migration/authored/guide/doc-inventory.nepld','doc/doc-inventory.md','13637103c1e5581238cc68a099f6e28553fbee21cc0bfd33fe557ca461093264'),('references','9fa716d','doc/migration/authored/references.nepld','doc/spec/references.md','c9ba86ece67d1bfe46b420d8b91b2dce95eb6e57a6b63d8c14e17534cafe9217')]
sources=[]
def freeze(name,rev,path):
    b=subprocess.check_output(['git','-C',str(repo),'show',rev+':'+path]);p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(b)
    sources.append({'path':name,'source_commit':rev,'source_path':path,'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()});return b
for p in ['AGENTS.md','doc/authoring.md','doc/spec/05-document.md','doc/spec/doc-signatures.md','design/forms.json','tools/audit/structure.py']:
    freeze('context/'+p,source,p)
helper=(root.parent/'review-b-ruby/check.py').read_bytes();(root/'structure-helper.py').write_bytes(helper)
ns={'__file__':str(root/'structure-helper.py')};exec(helper.decode().split('results=[]\nfor chapter')[0],ns)
Parser,forms,normalize,plain,walk,audit=[ns[k] for k in ['Parser','forms','normalize','plain','walk','audit_text']]
results=[]
def unmark(t):
    return re.sub(r'`([^`]+)`',r'\1',re.sub(r'\[([^\]]+)\]\(([^)]+)\)',r'\1',t))
for name,rev,draftpath,sourcepath,sha in cases:
    rev=subprocess.check_output(['git','-C',str(repo),'rev-parse',rev]).decode().strip()
    old=freeze(name+'/original.md',source,sourcepath);assert hashlib.sha256(old).hexdigest()==sha
    raw=freeze(name+'/draft.nepld',rev,draftpath);assert not raw.startswith(b'\xef\xbb\xbf') and b'\r' not in raw
    tree=Parser(raw.decode(),forms).complete('Doc/Article');audit(tree);start=len(ns['pairs']);n=normalize(tree,True);nodes=list(walk(n));md=old.decode()
    nofences=re.sub(r'^```[^\n]*\n.*?^```\n?', '', md,flags=re.M|re.S)
    codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',nofences)
    rawcodes=[x['fields'] for x in nodes if x['kind']=='RawCode'];blocks=re.findall(r'^```([^\n]*)\n(.*?)^```',md,re.M|re.S)
    assert [x['text'] for x in rawcodes]==[x[1] for x in blocks]
    assert [x['languageHint']['fields']['text'] for x in rawcodes]==[x[0] for x in blocks]
    links=[x['fields'] for x in nodes if x['kind']=='Link'];targets=[x['target']['fields'].get('uri',x['target']['fields'].get('path')) for x in links]
    expected=re.findall(r'\[[^\]]+\]\(([^)]+)\)',md) if name=='inventory' else re.findall(r'https://[^\s]+',md)
    assert targets==expected,(name,'targets',targets,expected)
    if name=='references':
        assert all(x['target']['kind']=='ExternalTarget' for x in links)
        assert [plain(x['label']) for x in links]==expected
        assert len(links)==18
    rows=[[plain(c) for c in x['fields']['cells']] for x in nodes if x['kind']=='Row']
    expectedrows=[[unmark(c.strip()) for c in line.strip('|').split('|')] for line in md.splitlines() if line.startswith('|') and not re.fullmatch(r'[|:\- ]+',line)]
    assert rows==expectedrows,(name,'rows')
    sections=[plain(x['fields']['title']) for x in nodes if x['kind']=='Section']
    assert sections==re.findall(r'^## (.*)$',md,re.M)
    tables=[x['fields'] for x in nodes if x['kind']=='Table']
    alignments=[[c['kind'] for c in x['columns']] for x in tables]
    expectedalignments=[]
    for line in md.splitlines():
        if line.startswith('|') and re.fullmatch(r'[|:\- ]+',line):
            expectedalignments.append(['AlignmentCenter' if c.strip().startswith(':') and c.strip().endswith(':') else 'AlignmentRight' if c.strip().endswith(':') else 'AlignmentLeft' if c.strip().startswith(':') else 'AlignmentDefault' for c in line.strip('|').split('|')])
    assert alignments==expectedalignments
    if name=='references':
        items=[x for x in nodes if x['kind']=='ListItem']
        assert [plain(x) for x in items]==re.findall(r'^- (.*)$',md,re.M)
    # Whole base text check retains whitespace within lines and code blocks;
    # only Markdown structural punctuation and paragraph line wrapping are removed.
    pieces=[];inside=False;block=[]
    for line in md.splitlines(keepends=True):
        if line.startswith('```'):
            if inside:pieces.append(''.join(block));block=[]
            inside=not inside;continue
        if inside:block.append(line);continue
        line=line.rstrip('\n')
        if not line:continue
        if line.startswith('|'):
            if re.fullmatch(r'[|:\- ]+',line):continue
            pieces.append(''.join(unmark(c.strip()) for c in line.strip('|').split('|')))
        else:pieces.append(unmark(re.sub(r'^(?:#{1,6} |[-*] |\d+\. )','',line)))
    expectedtext=''.join(pieces);actual=plain(n)
    assert actual==expectedtext,(name,'whole text',next(((i,actual[max(0,i-30):i+60],expectedtext[max(0,i-30):i+60]) for i,(a,b) in enumerate(zip(actual,expectedtext)) if a!=b),('length',len(actual),len(expectedtext))))
    (root/name/'normalized.json').write_bytes((json.dumps(n,ensure_ascii=False,indent=2)+'\n').encode())
    (root/name/'readings.txt').write_bytes(('\n'.join(a+' / '+b for a,b in sorted(set(ns['pairs'][start:])))+'\n').encode())
    results.append({'name':name,'source_commit':source,'draft_commit':rev,'all_base_text_exact':True,'codes_exact':len(codes),'rawcode_bytes_hints_exact':len(rawcodes),'rawcode_bytes':[len(x['text'].encode()) for x in rawcodes],'links_exact':len(links),'external_labels_equal_targets':name=='references','table_rows_including_headers_exact':len(rows),'table_alignments_exact':alignments,'section_titles_exact':len(sections),'ruby':len(ns['pairs'])-start,'kinds':dict(collections.Counter(x['kind'] for x in nodes)),'narrative_kanji_missing_ruby':0,'runtime_executed':False,'reachability_checked':False})
(root/'sources.json').write_text(json.dumps(sources,indent=2)+'\n',encoding='utf-8')
(root/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
print(json.dumps(results,indent=2))
