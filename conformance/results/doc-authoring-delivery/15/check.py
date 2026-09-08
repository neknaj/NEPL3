import collections,hashlib,json,pathlib,re,sys
import source_parser as parser
sys.stdout.reconfigure(encoding='utf-8')
p=pathlib.Path(__file__).resolve().parent
parser.forms=json.loads((p/'snapshot/design/forms.json').read_text(encoding='utf-8'))['categories']
raw=(p/'snapshot/doc/migration/authored/15-site.nepld').read_bytes()
md=(p/'snapshot/doc/spec/15-site.md').read_text(encoding='utf-8')
v=parser.Parser(raw.decode('utf-8')); value=v.run()
codes=[]; links=[]; tables=[]; sections=[]
def walk(x):
    if not isinstance(x,list):return
    if x and isinstance(x[0],str):
        if x[0]=='InlineCode':codes.append(x[1])
        if x[0]=='Link':links.append(x[1])
        if x[0]=='Table':tables.append(x)
        if x[0]=='Section':sections.append(x[1])
    for child in x:walk(child)
walk(value)
original_codes=re.findall(r'`([^`]+)`',md)
assert collections.Counter(original_codes)==collections.Counter(codes), (original_codes,codes)
assert original_codes==codes
original_links=re.findall(r'\[[^\]]*\]\(([^)]+)\)',md)
targets=[x[1] for x in links]
assert original_links==targets, (original_links,targets)
def text(items):
    assert all(x[0]=='Text' for x in items),items
    return ''.join(x[1] for x in items)
readings=[[text(a),text(b)] for a,b in v.readings]
assert all(re.fullmatch('[\u3400-\u9fff々]+',a) for a,b in readings)
assert all(re.fullmatch('[\u3040-\u309f\u30fc]+',b) for a,b in readings)
assert b'\r' not in raw
assert len(sections)==len(set(sections))==10
assert len(tables)==1 and len(tables[0][3])==8
result=dict(authored_sha256=hashlib.sha256(raw).hexdigest(),inline_code_exact_multiset=codes,links_exact_in_order=targets,sections=sections,tables=tables,ruby_count=len(readings),readings=readings,scope='Independent restricted source parser from fixed forms plus direct full-source review, not production runtime')
(p/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['tables','readings']},ensure_ascii=False,indent=2))
print(json.dumps(tables,ensure_ascii=False,indent=2))
