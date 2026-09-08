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
