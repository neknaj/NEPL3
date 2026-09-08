from pathlib import Path
import sys,json,re,hashlib,difflib

out=Path(__file__).resolve().parent
root=out.parents[1]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helper=helper[helper.index('han=re.compile'):helper.index('results=[]')]
(out/'helpers.py').write_text(helper,encoding='utf-8',newline='\n')
exec(compile(helper,'saved-review-helpers','exec'))
before=(out/'before.nepld').read_bytes()
path=root/'doc/migration/authored/21-doc-pages.nepld'
after=path.read_bytes()
marker=b'  cons section annotated '
at=after.index(marker)
assert before.endswith(b'  nil\n')
assert after[:at]==before[:-6] and after.endswith(b'  nil\n')
delta=after[at:-6]
(out/'addition.nepld').write_bytes(delta)
(out/'after.nepld').write_bytes(after)
assert b'\r' not in after and not after.startswith(b'\xef\xbb\xbf')
tree=Parser(after.decode('utf-8'),load_forms(root)).complete('Doc/Article')
section=[n for n in walk(tree) if n['kind']=='Section'][-1]
audit_text(section)
n=normalize(section,True)
paragraphs=[x for x in walk(n) if x['kind']=='Paragraph']
source=(out/'source.md').read_text(encoding='utf-8')
source=source.split('## 注釈付きMarkdown閲覧projection\n',1)[1].strip()
expected=[''.join(p.splitlines()) for p in source.split('\n\n')]
assert len(paragraphs)==len(expected)==8
def compact(s):return re.sub(r'\s+','',s)
actual=[plain(p) for p in paragraphs]
expected_plain=[re.sub(r'`([^`]+)`',r'\1',p) for p in expected]
assert [compact(x) for x in actual]==[compact(x) for x in expected_plain],[(i,a,e) for i,(a,e) in enumerate(zip(actual,expected_plain)) if compact(a)!=compact(e)]
codes=[x['fields']['text'] for x in walk(n) if x['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',source)
counts=[len(p['fields']['items']) for p in paragraphs]
readings=sorted(set(pairs))
(out/'readings.txt').write_text('\n'.join(f'{a} / {b}' for a,b in readings)+'\n',encoding='utf-8',newline='\n')
(out/'draft.diff').write_text(''.join(difflib.unified_diff(before.decode().splitlines(True),after.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8',newline='\n')
result={'paragraphs':8,'sentences':counts,'sentence_total':sum(counts),'inline_codes':codes,'ruby_occurrences':len(pairs),'unrelated_bytes_unchanged':True,'utf8_lf':True,'formal_structure_audit':True,'paragraph_base_text_equal_ignoring_whitespace':True,'production_runtime':False,'independent_review':False,'after_sha256':hashlib.sha256(after).hexdigest()}
(out/'check.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(result,ensure_ascii=True))
