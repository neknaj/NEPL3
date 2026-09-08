from pathlib import Path
import difflib,hashlib,json,re,subprocess,sys
root=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers=helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')];exec(helpers)
old=(out/'before.nepld').read_bytes();new=(root/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
marker=b'      cons paragraph\n        cons sentence\n          cons code "PageLinkPlan"'
assert old.count(marker)==1;pos=old.index(marker)
assert old[:pos]==new[:pos] and new.endswith(old[pos:])
added=new[pos:len(new)-len(old[pos:])]
assert b'\r' not in new and not new.startswith(b'\xef\xbb\xbf')
forms=load_forms(root);tree=Parser(new.decode(),forms).complete('Doc/Article');audit_text(tree)
extra=Parser('article ja "addition" body\n'+added.decode()+'nil',forms).complete('Doc/Article')
paragraphs=[n for n in walk(normalize(extra)) if n['kind']=='Paragraph'];assert len(paragraphs)==1
source=(out/'source.md').read_text(encoding='utf-8');start=source.index('`Relative` のpathが空');end=source.index('\n\n`PageLinkPlan`',start)
assert plain(paragraphs[0])==source[start:end].replace('\n','').replace('`','')
codes=[n['fields']['text'] for n in walk(normalize(extra)) if n['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',source[start:end])
pairs.clear();normalize(extra,True)
result={'scope':'author self-check only; no independent or runtime verification','source_commit':'379c85bc95ba57fbc3187476c338ee9f8a44bf72','source_sha256':hashlib.sha256((out/'source.md').read_bytes()).hexdigest(),'before_sha256':hashlib.sha256(old).hexdigest(),'draft_sha256':hashlib.sha256(new).hexdigest(),'existing_bytes_unchanged':True,'paragraphs':1,'sentences':len(paragraphs[0]['fields']['items']),'inline_codes':codes,'ruby_count':len(pairs),'base_text_exact':True,'full_form_and_ruby_check':True,'utf8_lf':True}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');(out/'after.nepld').write_bytes(new);(out/'addition.nepld').write_bytes(added)
(out/'paragraph.txt').write_text(plain(paragraphs[0])+'\n',encoding='utf-8');(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'draft.diff').write_text(''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8')
(out/'source.diff').write_bytes(subprocess.check_output(['git','diff','721afd9','379c85b','--','doc/spec/21-doc-pages.md'],cwd=root))
for path in ['AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py']:
    p=out/'context'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((root/path).read_bytes())
(out/'context/helpers.py').write_text(helpers,encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
