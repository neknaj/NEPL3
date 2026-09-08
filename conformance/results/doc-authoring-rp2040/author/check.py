from pathlib import Path
import hashlib,json,re,subprocess,sys
root=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
head='3813c0385a430d064c445615a80e45739f74a7e1'
source_path='conformance/targets/rp2040/README.md';draft_path='doc/migration/authored/conformance/targets/rp2040/README.nepld'
src=subprocess.check_output(['git','show',head+':'+source_path],cwd=root)
assert src==(root/source_path).read_bytes()
raw=(root/draft_path).read_bytes();assert b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf')
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers=helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')]
exec(helpers)
tree=Parser(raw.decode(),load_forms(root)).complete('Doc/Article');norm=normalize(tree)
paragraphs=[n for n in walk(norm) if n['kind']=='Paragraph']
actual=[' '.join(plain(s) for s in p['fields']['items']) for p in paragraphs]
text=src.decode();code_blocks=re.findall(r'```([^\n]+)\n(.*?)```',text,re.S)
without_code=re.sub(r'```[^\n]+\n.*?```','',text,flags=re.S)
units=[]
for block in without_code.split('\n\n'):
    block=block.strip()
    if not block or block.startswith('#'): continue
    if block.startswith('- '): units.extend(re.split(r'\n- ',block[2:]))
    else: units.append(block)
def base(value):
    value=' '.join(line.strip() for line in value.splitlines())
    value=re.sub(r'\[([^\]]+)\]\([^\)]+\)',r'\1',value)
    return value.replace('`','').replace('**','')
expected=[base(v) for v in units]
assert actual==expected,list(zip(actual,expected))
actual_codes=[n['fields']['text'] for n in walk(norm) if n['kind']=='InlineCode']
assert actual_codes==re.findall(r'`([^`]+)`',without_code)
rawnodes=[n for n in walk(norm) if n['kind']=='RawCode'];assert len(rawnodes)==1
assert rawnodes[0]['fields']['text']==code_blocks[0][1]
assert rawnodes[0]['fields']['languageHint']['fields']['text']==code_blocks[0][0]
links=[n for n in walk(norm) if n['kind']=='Link']
actual_links=[(plain(n['fields']['label']),n['fields']['target']['fields']['uri']) for n in links]
assert actual_links==re.findall(r'\[([^\]]+)\]\(([^\)]+)\)',without_code)
assert [plain(n['fields']['inline']) for n in walk(norm) if n['kind']=='Strong']==['not']
assert len([n for n in walk(norm) if n['kind']=='ListItem'])==5
assert [plain(n['fields']['title']) for n in walk(norm) if n['kind'] in ('Article','Section')]==re.findall(r'^#+ (.*)$',text,re.M)
result={'source_commit':head,'source_path':source_path,'source_sha256':hashlib.sha256(src).hexdigest(),'draft_path':draft_path,'draft_sha256':hashlib.sha256(raw).hexdigest(),'scope':'author self-check; no independent review or production parse/lower/HTML','paragraphs_including_items':len(paragraphs),'sentences':[len(p['fields']['items']) for p in paragraphs],'paragraph_text_exact':True,'inline_code_count':len(actual_codes),'links':actual_links,'rawcode_bytes':len(code_blocks[0][1].encode()),'rawcode_hint':code_blocks[0][0],'rawcode_exact':True,'list_items':5,'strong_not':True,'section_titles_exact':True,'language':'en','ruby_or_anno_added':False,'utf8_lf':True}
(out/'source.md').write_bytes(src);(out/'draft.nepld').write_bytes(raw)
(out/'paragraphs.txt').write_text('\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
for path in ['AGENTS.md','doc/authoring.md','doc/spec/doc-signatures.md','design/forms.json','tools/audit/structure.py']:
    p=out/'context'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((root/path).read_bytes())
(out/'context/helpers.py').write_text(helpers,encoding='utf-8')
print(json.dumps(result,indent=2))
