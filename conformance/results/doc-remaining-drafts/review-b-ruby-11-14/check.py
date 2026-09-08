from pathlib import Path
import json,re,sys,hashlib,collections
root=Path(__file__).resolve().parent
sys.path.insert(0,str(root/'context'))
from tools.audit.structure import Parser,load_forms
forms=load_forms(root/'context')
han=re.compile('[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+')
pairs=[]
def node(kind,**fields):return {'kind':kind,'fields':fields}
def literal(token):
    s=token[1:-1];pos=0
    def seq(ends):
        nonlocal pos
        items=[];buf=''
        def flush():
            nonlocal buf
            if buf:items.append(node('Text',text=buf));buf=''
        while pos<len(s):
            c=s[pos]
            if c in ends:break
            pos+=1
            if c=='\\':
                assert pos<len(s)
                e=s[pos];pos+=1
                buf+={'n':'\n','r':'\r','t':'\t'}.get(e,e)
            elif c in '[{':
                flush();parts=[];close=']' if c=='[' else '}'
                while True:
                    children=seq('/'+close);parts.append(node('Concat',inlines=children))
                    assert pos<len(s)
                    d=s[pos];pos+=1
                    if d==close:break
                assert len(parts)==2 if c=='[' else len(parts)>=2
                items.append(node('Ruby',base=parts[0],reading=parts[1]) if c=='[' else node('Anno',base=parts[0],notes=parts[1:]))
            else:buf+=c
        flush();return items
    result=node('Sentence',inlines=seq(''))
    assert pos==len(s)
    return result
def normalize(n,collect=False):
    if isinstance(n,list):return [normalize(x,collect) for x in n]
    if not isinstance(n,dict):return n
    if n['kind']=='leaf':return normalize(literal(n['token']),collect)
    f=n['fields'];k=n['kind']
    if k=='Ruby':
        b=normalize(f['base'],collect);r=normalize(f['reading'],collect)
        if collect:
            base=plain(b);reading=plain(r);pairs.append((base,reading))
            assert han.fullmatch(base),(base,reading)
            assert reading
        return b
    fields={key:normalize(value,collect) for key,value in f.items()}
    if k in ('Sentence','Concat'):
        result=[]
        def add(x):
            if x['kind']=='Concat':
                for child in x['fields']['inlines']:add(child)
            elif x['kind']=='Text' and result and result[-1]['kind']=='Text':
                result[-1]['fields']['text']+=x['fields']['text']
            else:result.append(x)
        for x in fields['inlines']:add(x)
        if k=='Concat' and len(result)==1:return result[0]
        fields['inlines']=result
    return {'kind':k,'fields':fields}
def plain(n):
    if isinstance(n,list):return ''.join(map(plain,n))
    if not isinstance(n,dict):return ''
    f=n['fields']
    if n['kind'] in ('Text','InlineCode','RawCode'):return f['text']
    return ''.join(plain(v) for v in f.values())
def walk(n):
    if isinstance(n,list):
        for x in n:yield from walk(x)
    elif isinstance(n,dict):
        yield n
        for v in n.get('fields',{}).values():yield from walk(v)
def audit_text(n,protected=False):
    if isinstance(n,list):
        for x in n:audit_text(x,protected)
    elif isinstance(n,dict):
        if n['kind']=='leaf':audit_text(literal(n['token']),protected);return
        k=n['kind'];f=n['fields']
        if k=='Text' and not protected:assert not han.search(f['text']),f['text']
        if k=='Ruby':
            audit_text(f['base'],True)
            audit_text(f['reading'],True)
        else:
            for v in f.values():audit_text(v,protected)
results=[]
for chapter in ['11','12']:
    before=Parser((root/chapter/'before.nepld').read_text(encoding='utf-8'),forms).complete('Doc/Article')
    raw=(root/chapter/'after.nepld').read_bytes();assert b'\r' not in raw
    after=Parser(raw.decode(),forms).complete('Doc/Article')
    audit_text(after)
    a=normalize(before);start=len(pairs);b=normalize(after,True)
    if a!=b:
        (root/chapter/'before-normalized.json').write_bytes(json.dumps(a,ensure_ascii=False,indent=2).encode())
        (root/chapter/'after-normalized.json').write_bytes(json.dumps(b,ensure_ascii=False,indent=2).encode())
        def diff(x,y,path=''):
            if type(x)!=type(y):print(path,repr(x),repr(y));return
            if isinstance(x,dict):
                for k in x:diff(x[k],y.get(k),path+'/'+k)
            elif isinstance(x,list):
                if len(x)!=len(y):print(path,'length',len(x),len(y))
                for i,(u,v) in enumerate(zip(x,y)):diff(u,v,path+'/'+str(i))
            elif x!=y:print(path,repr(x),repr(y))
        diff(a,b)
        raise AssertionError(chapter+' changed underlying content/structure')
    nodes=list(walk(b));counts=collections.Counter(n['kind'] for n in nodes)
    code=[n['fields']['text'] for n in nodes if n['kind']=='InlineCode']
    md=(root/chapter/('corrected-original.md' if chapter=='12' else 'original.md')).read_text(encoding='utf-8')
    mdcodes=re.findall(r'`([^`\n]+)`',re.sub(r'```.*?```','',md,flags=re.S))
    # Original D05 spells ]]> as ordinary text; the pre-repair draft already
    # marks these exact bytes as InlineCode. Preserve that existing choice.
    if chapter=='11':
        assert code[1]==']]>' and 'D05:' in md and ']]>のescape' in md
        compared_code=code[:1]+code[2:]
    else:compared_code=code
    assert compared_code==mdcodes,(chapter,'code mismatch',code,mdcodes)
    rawcode=[n['fields'] for n in nodes if n['kind']=='RawCode']
    blocks=re.findall(r'^```([^\n]*)\n(.*?)^```',md,re.M|re.S)
    assert [x['text'] for x in rawcode]==[x[1] for x in blocks]
    assert [x['languageHint']['fields']['text'] for x in rawcode]==[x[0] for x in blocks]
    links=[n['fields']['target'] for n in nodes if n['kind']=='Link']
    uris=[x['fields'].get('uri',x['fields'].get('path')) for x in links]
    assert uris==re.findall(r'\[[^\]]+\]\(([^)]+)\)',md),(chapter,uris)
    rows=[n for n in nodes if n['kind']=='Row']
    cells=[[plain(c) for c in n['fields']['cells']] for n in rows]
    expected=[[c.strip() for c in line.strip().strip('|').split('|')] for line in md.splitlines() if line.startswith('|') and not re.fullmatch(r'[|:\- ]+',line)]
    assert cells==expected,(chapter,'table changed',cells,expected)
    encoded=(json.dumps(b,ensure_ascii=False,sort_keys=True,indent=2)+'\n').encode()
    (root/chapter/'normalized.json').write_bytes(encoded)
    ps=sorted(set(pairs[start:]))
    (root/chapter/'readings.txt').write_bytes(('\n'.join(x+' / '+y for x,y in ps)+'\n').encode())
    results.append({'chapter':chapter,'normalized_before_after_equal':True,'normalized_sha256':hashlib.sha256(encoded).hexdigest(),'kinds':dict(counts),'ruby_occurrences':len(pairs)-start,'unique_readings':len(ps),'original_inline_code_order_equal':len(mdcodes),'total_inline_code':len(code),'original_rawcode_bytes_and_hints_equal':len(rawcode),'original_link_order_equal':len(uris),'original_table_rows_equal':len(cells),'unannotated_narrative_kanji':0})
(root/'results.json').write_bytes((json.dumps(results,indent=2)+'\n').encode())
print(json.dumps(results,indent=2))
