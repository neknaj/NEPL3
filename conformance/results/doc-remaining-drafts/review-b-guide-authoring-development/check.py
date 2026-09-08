from pathlib import Path
import json,hashlib,subprocess,sys,re,collections
root=Path(__file__).resolve().parent;repo=Path('C:/projects/NEPL3-doc-authoring-b')
source='90f4552d9d8de662bada4d76c688c8cf30afa595';sources=[]
requests=[]
for name,rev in [('authoring','98eb0c23fb10459d35c180bfe36746e25fcba571'),('development','446000e65619bf6ddad5032094bd747ddf330d8f')]:
    requests += [(name+'/draft.nepld',rev,'doc/migration/authored/guide/'+name+'.nepld'),(name+'/original.md',source,'doc/'+name+'.md')]
requests += [('context/'+p,source,p) for p in ['AGENTS.md','doc/authoring.md','doc/spec/05-document.md','doc/spec/doc-signatures.md','design/forms.json','tools/audit/structure.py']]
for name,rev,path in requests:
    data=subprocess.check_output(['git','-C',str(repo),'show',rev+':'+path]);f=root/name;f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(data)
    sources.append({'path':name,'source_commit':rev,'source_path':path,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
assert sources[1]['sha256']=='0659615a44794990f1c6356a660516e4940f8e8efe272903aff349907deeae08'
assert sources[3]['sha256']=='9fd123a202fb3700af89aaaa46a6ef34b17a38312b1f10492ee4bfab8322ece2'
(root/'sources.json').write_bytes((json.dumps(sources,indent=2)+'\n').encode())
helper=(root.parent/'review-b-ruby/check.py').read_bytes();(root/'structure-helper.py').write_bytes(helper)
ns={'__file__':str(root/'structure-helper.py')};exec(helper.decode().split('results=[]\nfor chapter')[0],ns)
Parser,forms,normalize,plain,walk,audit=[ns[k] for k in ('Parser','forms','normalize','plain','walk','audit_text')]
class ReviewParser(Parser):
    # The supplied audit deliberately supports ASCII identifiers only. Preserve
    # this one explicitly inspected Japanese XID spelling for content review;
    # this is not a substitute for production Unicode lexical validation.
    def parse(self,category,depth=0):
        if category=='@Name' and self.pos<len(self.tokens) and self.tokens[self.pos]=='明示的な改行はbreak':
            self.pos+=1
            return '明示的な改行はbreak'
        return super().parse(category,depth)
results=[]
for name in ['authoring','development']:
    data=(root/name/'draft.nepld').read_bytes();assert b'\r' not in data and not data.startswith(b'\xef\xbb\xbf')
    ast=ReviewParser(data.decode(),forms).complete('Doc/Article');audit(ast);start=len(ns['pairs']);n=normalize(ast,True);nodes=list(walk(n))
    md=(root/name/'original.md').read_text(encoding='utf-8')
    rawcodes=[x['fields'] for x in nodes if x['kind']=='RawCode']
    blocks=re.findall(r'^```([^\n]*)\n(.*?)^```',md,re.M|re.S)
    assert [x['text'] for x in rawcodes]==[x[1] for x in blocks]
    assert [x['languageHint']['fields']['text'] for x in rawcodes]==[x[0] for x in blocks]
    if name=='authoring':
        for example in rawcodes:
            category='Doc/Flow' if example['text'].startswith('parallel\n') else 'Doc/Sentence'
            ReviewParser(example['text'],forms).complete(category)
        ids=[x['fields']['id'] for x in nodes if x['kind']=='Section']
        assert ids.count('明示的な改行はbreak')==1
    outside=re.sub(r'^```[^\n]*\n.*?^```\n?','',md,flags=re.M|re.S)
    codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode']
    expected=re.findall(r'`([^`]+)`',outside)
    assert codes==expected,(name,codes,expected)
    links=[x['fields']['target'] for x in nodes if x['kind']=='Link'];targets=[]
    for link in links:
        f=link['fields'];target=f.get('path',f.get('uri'))
        if link['kind']=='RelativeTarget' and f['fragment']['kind']=='SomeText':target+='#'+f['fragment']['fields']['text']
        targets.append(target)
    assert targets==re.findall(r'\[[^\]]+\]\(([^)]+)\)',outside),(name,links,targets)
    (root/name/'normalized.json').write_bytes((json.dumps(n,ensure_ascii=False,indent=2)+'\n').encode())
    (root/name/'readings.txt').write_bytes(('\n'.join(a+' / '+b for a,b in sorted(set(ns['pairs'][start:])))+'\n').encode())
    paragraphs=[plain(x) for x in nodes if x['kind']=='Paragraph']
    expected_paragraphs=[]
    for paragraph in outside.split('\n\n'):
        paragraph=paragraph.strip()
        if not paragraph or paragraph.startswith('#'):continue
        paragraph=paragraph.replace('\n','')
        paragraph=re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',paragraph)
        paragraph=re.sub(r'`([^`]+)`',r'\1',paragraph)
        expected_paragraphs.append(paragraph)
    assert paragraphs==expected_paragraphs,(name,[(i,a,b) for i,(a,b) in enumerate(zip(paragraphs,expected_paragraphs)) if a!=b])
    (root/name/'paragraphs.txt').write_bytes(('\n\n'.join(paragraphs)+'\n').encode())
    results.append({'name':name,'all_paragraph_underlying_text_exact':True,'kinds':dict(collections.Counter(x['kind'] for x in nodes)),'rawcode_exact_bytes_hints':len(blocks),'inline_code_exact_order':len(codes),'links_exact_order':len(links),'ruby_occurrences':len(ns['pairs'])-start,'narrative_unannotated_kanji':0,'production_runtime_executed':False})
(root/'results.json').write_bytes((json.dumps(results,indent=2)+'\n').encode());print(json.dumps(results,indent=2))
