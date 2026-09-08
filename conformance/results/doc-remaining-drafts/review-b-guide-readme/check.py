from pathlib import Path
import json,hashlib,subprocess,re,sys
root=Path(__file__).resolve().parent;repo=Path('C:/projects/NEPL3-doc-authoring-b')
commit='ca6081affbee132a217d0e0bd327a96845ff5e95';source='90f4552d9d8de662bada4d76c688c8cf30afa595'
requests=[('draft.nepld',commit,'doc/migration/authored/guide/README.nepld'),('original.md',source,'doc/README.md')]
requests += [('context/'+p,source,p) for p in ['AGENTS.md','doc/authoring.md','doc/spec/05-document.md','doc/spec/doc-signatures.md','design/forms.json','tools/audit/structure.py']]
sources=[]
for name,rev,path in requests:
    data=subprocess.check_output(['git','-C',str(repo),'show',rev+':'+path]);f=root/name;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(data)
    sources.append({'path':name,'source_commit':rev,'source_path':path,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
assert sources[1]['sha256']=='4e87bdd9154599be5b364755da38e5905c15c4cc87255802a06ed34b3d4fc978'
(root/'sources.json').write_bytes((json.dumps(sources,indent=2)+'\n').encode())
helper=(root.parent/'review-b-ruby/check.py').read_bytes();(root/'structure-helper.py').write_bytes(helper)
ns={'__file__':str(root/'structure-helper.py')};exec(helper.decode().split('results=[]\nfor chapter')[0],ns)
Parser,forms,normalize,plain,walk,audit=[ns[k] for k in ('Parser','forms','normalize','plain','walk','audit_text')]
data=(root/'draft.nepld').read_bytes();assert b'\r' not in data and not data.startswith(b'\xef\xbb\xbf')
ast=Parser(data.decode(),forms).complete('Doc/Article');audit(ast);n=normalize(ast,True);nodes=list(walk(n))
md=(root/'original.md').read_text(encoding='utf-8')
links=[x['fields']['target'] for x in nodes if x['kind']=='Link']
targets=[x['fields'].get('path',x['fields'].get('uri')) for x in links]
assert targets==re.findall(r'\[[^\]]+\]\(([^)]+)\)',md)
codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',md)
def unmark(s):
    s=re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',s)
    return re.sub(r'`([^`]+)`',r'\1',s)
rows=[[plain(c) for c in x['fields']['cells']] for x in nodes if x['kind']=='Row']
expected=[[unmark(c.strip()) for c in l.strip('|').split('|')] for l in md.splitlines() if l.startswith('|') and not re.fullmatch(r'[|:\- ]+',l)]
assert rows==expected
ordered=[x for x in nodes if x['kind']=='List' and x['fields']['style']['kind']=='Ordered']
assert len(ordered)==1
items=ordered[0]['fields']['items']
expecteditems=[unmark(x) for x in re.findall(r'^\d+\. (.*)$',md,re.M)]
assert [plain(x) for x in items]==expecteditems
assert ordered[0]['fields']['style']['fields']['start']=='1'
source_text=[]
for line in md.splitlines():
    if not line:continue
    if line.startswith('|'):
        if re.fullmatch(r'[|:\- ]+',line):continue
        source_text.append(''.join(unmark(c.strip()) for c in line.strip('|').split('|')))
    else:
        line=re.sub(r'^#{1,6} ','',line)
        line=re.sub(r'^(?:\d+\. |- )','',line)
        source_text.append(unmark(line))
assert plain(n)==''.join(source_text)
(root/'normalized.json').write_bytes((json.dumps(n,ensure_ascii=False,indent=2)+'\n').encode())
(root/'readings.txt').write_bytes(('\n'.join(a+' / '+b for a,b in sorted(set(ns['pairs'])))+'\n').encode())
result={'draft_commit':commit,'source_commit':source,'all_underlying_text_exact':True,'ordered_reading_items':len(items),'table_rows_with_header':len(rows),'links_exact_order':len(targets),'inline_code_exact_order':len(codes),'ruby_occurrences':len(ns['pairs']),'narrative_unannotated_kanji':0,'production_runtime_executed':False}
(root/'results.json').write_bytes((json.dumps(result,indent=2)+'\n').encode());print(json.dumps(result,indent=2))
