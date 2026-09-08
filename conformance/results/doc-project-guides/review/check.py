from pathlib import Path
import subprocess,json,hashlib,re
r=Path(__file__).parent;root=r.parent.parent
commit='dcb1e2f04820a24c5761cf53a6d14ecb1b8e23e1';base='2b48d5'
sha=lambda b:hashlib.sha256(b).hexdigest()
paths=['README.md','implementation-status.json','AGENTS.md','doc/authoring.md','SECURITY.md','CONTRIBUTING.md','doc/migration/authored/project/SECURITY.nepld','doc/migration/authored/project/CONTRIBUTING.nepld']
for path in paths:
    out=r/'snapshot'/path;out.parent.mkdir(parents=True,exist_ok=True)
    out.write_bytes(subprocess.check_output(['git','show',commit+':'+path],cwd=root))
arities={'article':3,'body':1,'cons':2,'nil':0,'paragraph':1,'sentence':1,'text':1,'ruby':2,'concat':1,'link':2,'external':1,'relative':2,'none':0,'code':1}
def parse(text):
    tokens=re.findall(r'"(?:[^"\\]|\\.)*"|[^\s]+',text);index=0
    def take():
        nonlocal index
        t=tokens[index];index+=1
        if t.startswith('"'):return json.loads(t)
        if t=='ja':return t
        assert t in arities,t
        return (t,[take() for _ in range(arities[t])])
    node=take();assert index==len(tokens);return node
def seq(node):
    kind,args=node
    if kind=='nil':return []
    assert kind=='cons';return [args[0]]+seq(args[1])
def review(name):
    md=(r/'snapshot'/f'{name}.md').read_text(encoding='utf-8')
    doc=parse((r/'snapshot'/f'doc/migration/authored/project/{name}.nepld').read_text(encoding='utf-8'))
    links=[];codes=[];readings=[];sentences=[];uncovered=[]
    def literal(s):
        # This pair uses only ordinary Text and non-nested Ruby literals.
        parts=[];last=0
        for m in re.finditer(r'\[([^\[\]/]+)/([^\[\]/]+)\]',s):
            ordinary=s[last:m.start()];uncovered.extend(re.findall(r'[一-龯々]+',ordinary));parts.extend([ordinary,m[1]]);readings.append([m[1],m[2]]);last=m.end()
        tail=s[last:];uncovered.extend(re.findall(r'[一-龯々]+',tail));parts.append(tail)
        assert not re.search(r'[\[\]{}]', ''.join(parts))
        return ''.join(parts)
    def inline(n,rubybase=False):
        k,a=n
        if k=='text':
            if not rubybase:uncovered.extend(re.findall(r'[一-龯々]+',a[0]))
            return a[0]
        if k=='ruby':
            assert a[0][0]=='text' and a[1][0]=='text';readings.append([a[0][1][0],a[1][1][0]]);return inline(a[0],True)
        if k=='concat':return ''.join(inline(x,rubybase) for x in seq(a[0]))
        if k=='code':codes.append(a[0]);return a[0]
        if k=='link':
            target=a[0];assert target[0] in ['relative','external']
            if target[0]=='relative':assert target[1][1]==('none',[])
            label=inline(a[1]);links.append([label,target[1][0]]);return label
        raise AssertionError(k)
    def sentence(n):
        out=literal(n) if isinstance(n,str) else ''.join(inline(x) for x in seq(n[1][0]))
        sentences.append(out);assert out.count('。')<=1;return out
    assert doc[0]=='article' and doc[1][0]=='ja' and doc[1][2][0]=='body'
    title=literal(doc[1][1]);paragraphs=[]
    for node in seq(doc[1][2][1][0]):
        assert node[0]=='paragraph';paragraphs.append(''.join(sentence(x) for x in seq(node[1][0])))
    expected=md.strip().split('\n\n');assert expected.pop(0)=='# '+title
    mdlinks=[];mdcodes=[]
    def flatten(s):
        def l(m):mdlinks.append([m[1],m[2]]);return m[1]
        s=re.sub(r'\[([^\]]+)\]\(([^)]+)\)',l,s)
        def c(m):mdcodes.append(m[1]);return m[1]
        return re.sub(r'`([^`]+)`',c,s)
    expected=list(map(flatten,expected));assert paragraphs==expected,(name,paragraphs,expected)
    assert links==mdlinks and codes==mdcodes
    assert not uncovered,uncovered
    assert all(re.fullmatch(r'[一-龯々]+',b) and re.fullmatch(r'[ぁ-ゖー]+',v) for b,v in readings)
    return dict(page=name,paragraphs=paragraphs,sentences=sentences,links=links,codes=codes,readings=readings,exact_text_equal=True,uncovered_kanji=uncovered)
result=[review(n) for n in ['SECURITY','CONTRIBUTING']]
(r/'checks.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
author=root/'.tmp/authoring';manifest=author/'manifest.json';assert sha(manifest.read_bytes())=='880ea953543644eaa7027f9eefb7aad1cc8166a700823f209c2efe769dab775b'
for item in json.loads(manifest.read_text(encoding='utf-8'))['files']:
    data=(author/item['path']).read_bytes();assert len(data)==item['bytes'] and sha(data)==item['sha256']
(r/'source.json').write_text(json.dumps(dict(commit=commit,base=base,author_evidence_sha256=sha(manifest.read_bytes()),inputs=[dict(path=p,sha256=sha((r/'snapshot'/p).read_bytes())) for p in paths]),indent=2)+'\n',encoding='utf-8',newline='\n')
print([(v['page'],len(v['paragraphs']),len(v['sentences']),len(v['links']),len(v['codes']),len(v['readings'])) for v in result])
