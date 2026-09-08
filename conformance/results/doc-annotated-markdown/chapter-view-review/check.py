from pathlib import Path
import sys,json,re,hashlib,collections
from html.parser import HTMLParser
import mistune

out=Path(__file__).resolve().parent
root=out.parents[1]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
files=[('doc/spec/13-reproducibility.md','canonical.md'),('doc/migration/authored/13-reproducibility.nepld','draft.nepld'),('.tmp/chapter13/generated.md','generated.md'),('.tmp/chapter13/github.html','github.html')]
for source,dest in files:
    target=out/dest
    if not target.exists(): target.write_bytes((root/source).read_bytes())
helper=(out/'helpers.py').read_text(encoding='utf-8')
exec(compile(helper,'review-literal-helper','exec'))
tree=Parser((out/'draft.nepld').read_text(encoding='utf-8'),load_forms(root)).complete('Doc/Article')
def expand(n):
    if isinstance(n,list):return [expand(x) for x in n]
    if not isinstance(n,dict):return n
    if n['kind']=='leaf':return expand(literal(n['token']))
    return {'kind':n['kind'],'fields':{k:expand(v) for k,v in n['fields'].items()}}
tree=expand(tree)
def compact(events):
    result=[]
    for e in events:
        if e[0]=='text' and not e[1]:continue
        if e[0]=='text' and result and result[-1][0]=='text':result[-1]=('text',result[-1][1]+e[1])
        else:result.append(e)
    return result
def inline(n,notes=True):
    if isinstance(n,list):return compact([e for x in n for e in inline(x,notes)])
    k=n['kind']; f=n['fields']
    if k=='Text':return [('text',f['text'])]
    if k=='InlineCode':return [('code',f['text'])]
    if k in ('Sentence','Concat'):return inline(f['inlines'],notes)
    if k=='Ruby':return compact(inline(f['base'],notes)+([('text','[')]+inline(f['reading'],notes)+[('text',']')] if notes else []))
    if k=='Anno':
        ev=inline(f['base'],notes)
        if notes:
            ev+=[('text','{')]
            for i,note in enumerate(f['notes']):
                if i:ev+=[('text','/')]
                ev+=inline(note,notes)
            ev+=[('text','}')]
        return compact(ev)
    if k=='Link':
        target=f['target']; assert target['kind']=='ExternalTarget'
        return [('link-start',target['fields']['uri'])]+inline(f['label'],notes)+[('link-end',)]
    raise AssertionError(('unexpected actual inline kind',k))
def blocks(n,level=1,notes=True):
    k=n['kind']; f=n['fields']
    if k=='Article':return [('h1',inline(f['title'],notes))]+blocks(f['body'],1,notes)
    if k=='Body':return [b for x in f['blocks'] for b in blocks(x,level,notes)]
    if k=='Section':return [('h'+str(level+1),inline(f['title'],notes))]+blocks(f['body'],level+1,notes)
    if k=='Paragraph':return [('p',inline(f['items'],notes))]
    raise AssertionError(('unexpected actual block kind',k))
expected=blocks(tree)
base=blocks(tree,notes=False)
class Content(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True);self.blocks=[];self.tag=None;self.events=[];self.code=None;self.anchors=[]
    def handle_starttag(self,tag,attrs):
        attrs=dict(attrs)
        if tag in ['p','h1','h2','h3','h4','h5','h6']:
            assert self.tag is None;self.tag=tag;self.events=[]
        elif tag=='a':
            if 'href' in attrs:self.events.append(('link-start',attrs['href']))
            elif 'name' in attrs:self.anchors.append(attrs['name'])
        elif tag=='code':self.code=''
        else:raise AssertionError(('unexpected html element',tag,attrs))
    def handle_endtag(self,tag):
        if tag==self.tag:
            e=compact(self.events)
            if e:self.blocks.append((self.tag,e))
            self.tag=None
        elif tag=='code':self.events.append(('code',self.code));self.code=None
        elif tag=='a':
            if self.events and any(e[0]=='link-start' for e in self.events):self.events.append(('link-end',))
        else:raise AssertionError(('unexpected close',tag))
    def handle_data(self,data):
        if self.code is not None:self.code+=data
        elif self.tag:self.events.append(('text',data))
        else:assert not data.strip(),data
def html_blocks(text):
    p=Content();p.feed(text);p.close();return p
github=html_blocks((out/'github.html').read_text(encoding='utf-8'))
aliases=json.loads((out/'root-aliases.json').read_text(encoding='utf-8'))
assert all(github.anchors.count('user-content-'+a['name'])==1 for a in aliases)
mdrenderer=mistune.create_markdown(escape=False)
parsed=html_blocks(mdrenderer((out/'generated.md').read_text(encoding='utf-8')))
assert github.blocks==expected,[(i,a,b) for i,(a,b) in enumerate(zip(github.blocks,expected)) if a!=b]
if parsed.blocks!=expected:
    mismatch=[{'index':i,'parsed':a,'expected':b} for i,(a,b) in enumerate(zip(parsed.blocks,expected)) if a!=b]
    (out/'mistune-mismatch.json').write_text(json.dumps({'lengths':[len(parsed.blocks),len(expected)],'mismatch':mismatch},ensure_ascii=False,indent=2),encoding='utf-8')
    assert len(parsed.blocks)==len(expected) and [x['index'] for x in mismatch]==[5]
# Separately inspect the old Markdown, rather than erase notes from output by regex.
old=html_blocks(mdrenderer((out/'canonical.md').read_text(encoding='utf-8')))
def text_of(b):return ''.join(e[1] for e in b[1] if e[0] in ['text','code'])
assert len(old.blocks)==len(base)
differences=[]
for i,(a,b) in enumerate(zip(old.blocks,base)):
    if a!=b:
        differences.append({'block':i,'old':a,'draft_base':b,'plain_equal':text_of(a)==text_of(b),'plain_equal_ignoring_whitespace':re.sub(r'\s+','',text_of(a))==re.sub(r'\s+','',text_of(b))})
def codes(bs):return [e[1] for _,es in bs for e in es if e[0]=='code']
def uris(bs):return [e[1] for _,es in bs for e in es if e[0]=='link-start']
assert uris(old.blocks)==uris(base)==uris(expected)==uris(github.blocks)
assert all(a[0]==b[0] for a,b in zip(old.blocks,base))
assert all(d['plain_equal'] for d in differences)
counts=collections.Counter(n['kind'] for n in walk(tree))
result={'source_files':[{'source':a,'payload':b,'bytes':len((out/b).read_bytes()),'sha256':hashlib.sha256((out/b).read_bytes()).hexdigest()} for a,b in files], 'mistune_version':mistune.__version__,'blocks':len(expected),'headings':[b for b in expected if b[0]!='p'],'paragraphs':sum(b[0]=='p' for b in expected),'doc_kinds':dict(counts),'draft_inline_codes':codes(expected),'original_inline_codes':codes(old.blocks),'external_uris':uris(expected),'old_to_draft_differences':differences,'checks':{'generated_markdown_mistune_all_events_equal_doc':parsed.blocks==expected,'supplied_github_html_all_notes_events_equal_doc':True,'old_heading_levels_equal':True,'old_base_paragraphs_equal_ignoring_whitespace':True,'github_links_equal':True,'browser':False,'production_rerun':False,'doc_roundtrip':False}}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({'blocks':len(expected),'paragraphs':result['paragraphs'],'doc_kinds':dict(counts),'old_to_draft_difference_count':len(differences),'code_counts':[len(result['original_inline_codes']),len(result['draft_inline_codes'])],'uris':result['external_uris']},ensure_ascii=True))
