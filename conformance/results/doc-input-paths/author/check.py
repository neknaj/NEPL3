from pathlib import Path
import difflib, hashlib, json, re, subprocess, sys
root=Path(__file__).resolve().parents[2]
out=Path(__file__).resolve().parent
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers=helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser, load_forms
exec(helpers)
forms=load_forms(root)
old=(out/'before.nepld').read_bytes()
new=(root/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
assert b'\r' not in old+new and not new.startswith(b'\xef\xbb\xbf')
marker='        cons "[現在/げんざい]の[表示/ひょうじ]はRowsに[固定/こてい]する。"\n        nil\n'.encode()
assert old.count(marker)==1
pos=old.index(marker)+len(marker)
assert new[:pos]==old[:pos] and new.endswith(old[pos:])
addition=new[pos:len(new)-len(old[pos:])]
(out/'addition.nepld').write_bytes(addition)
before=Parser(old.decode(),forms).complete('Doc/Article')
after=Parser(new.decode(),forms).complete('Doc/Article')
audit_text(after)
extra=Parser('article ja "addition" body\n'+addition.decode()+'nil',forms).complete('Doc/Article')
normalized=normalize(extra)
paras=[n for n in walk(normalized) if n['kind']=='Paragraph']
assert len(paras)==2
source=(out/'source.md').read_text(encoding='utf-8')
start=source.index('各pageには省略可能な')
end=source.index('\n\n全pageを同じproduction API',start)
expected=[p.replace('\n','').replace('`','') for p in source[start:end].split('\n\n')]
actual=[plain(p) for p in paras]
assert actual==expected,list(zip(actual,expected))
codes=[n['fields']['text'] for p in paras for n in walk(p) if n['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',source[start:end])
pairs.clear(); normalize(extra,True)
result={'scope':'author self-check, not independent review or production parse/lower/HTML','head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'source_sha256':hashlib.sha256((out/'source.md').read_bytes()).hexdigest(),'old_draft_sha256':hashlib.sha256(old).hexdigest(),'draft_sha256':hashlib.sha256(new).hexdigest(),'existing_prefix_suffix_bytes_unchanged':True,'paragraphs':2,'sentences':[len(p['fields']['items']) for p in paras],'inline_codes':codes,'ruby_count':len(pairs),'source_paragraph_text_exact':True,'full_form_structure':True,'ruby_text_audit':True,'utf8_lf':True}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(out/'paragraphs.txt').write_text('\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'after.nepld').write_bytes(new)
(out/'draft.diff').write_text(''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8')
(out/'source.diff').write_bytes(subprocess.check_output(['git','diff','--','doc/spec/21-doc-pages.md'],cwd=root))
for p in ['AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py']:
    target=out/'context'/p; target.parent.mkdir(parents=True,exist_ok=True); target.write_bytes((root/p).read_bytes())
(out/'context/ruby-check-helpers.py').write_text(helpers,encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
