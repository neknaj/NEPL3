from pathlib import Path
import sys,json,re,hashlib,collections
import mistune
from bs4 import BeautifulSoup
p=Path(__file__).resolve().parent;root=p.parents[1]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
exec(compile((p/'helpers.py').read_text(encoding='utf-8'),'helpers.py','exec'))
draft=root/'doc/migration/authored/22-external-extensions.nepld'
tree=Parser(draft.read_text(encoding='utf-8'),load_forms(root)).complete('Doc/Article')
audit_text(tree)
base=normalize(tree,True)
def blocks(n):
    k=n['kind'];f=n['fields']
    if k=='Article':return [('heading',plain(f['title']))]+blocks(f['body'])
    if k=='Body':return [b for x in f['blocks'] for b in blocks(x)]
    if k=='Section':return [('heading',plain(f['title']))]+blocks(f['body'])
    if k=='Paragraph':return [('paragraph',plain(f['items']))]
    if k=='Table':return [('table',plain(n))]
    if k=='List':return [b for item in f['items'] for b in blocks(item['fields']['body'])]
    raise AssertionError(k)
actual=blocks(base)
md=mistune.create_markdown(plugins=['table'])
soup=BeautifulSoup(md((p/'source.md').read_text(encoding='utf-8')),'html.parser')
expected=[]
for node in soup.children:
    if not getattr(node,'name',None):continue
    if node.name in ['h1','h2']:expected.append(('heading',node.get_text()))
    elif node.name=='p':expected.append(('paragraph',node.get_text().replace('\n','')))
    elif node.name=='table':expected.append(('table',''.join(x.get_text() for x in node.find_all(['th','td']))))
    elif node.name=='ol':
        expected += [('paragraph',x.get_text().replace('\n','')) for x in node.find_all('li')]
    else:raise AssertionError(node.name)
assert actual==expected,[(i,a,b) for i,(a,b) in enumerate(zip(actual,expected)) if a!=b]
codes=[n['fields']['text'] for n in walk(base) if n['kind']=='InlineCode']
assert codes==[n.get_text() for n in soup.find_all('code')]
counts=collections.Counter(n['kind'] for n in walk(tree))
result={'formal_structure':True,'all_ordered_base_blocks_equal_original':True,'block_count':len(actual),'inline_codes':codes,'ruby_count':len(pairs),'ruby_kanji_only':True,'prefix_text_ruby_audit':True,'source_sha256':hashlib.sha256((p/'source.md').read_bytes()).hexdigest(),'draft_sha256':hashlib.sha256(draft.read_bytes()).hexdigest(),'original_unchanged_since_capture':(root/'doc/spec/22-external-extensions.md').read_bytes()==(p/'source.md').read_bytes(),'counts_prefix':dict(counts),'production_parser':False,'html':False,'independent_review':False,'canonical_cutover':False}
(p/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
(p/'readings.json').write_text(json.dumps(pairs,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(result,ensure_ascii=True))
