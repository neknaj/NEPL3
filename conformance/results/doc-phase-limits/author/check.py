from pathlib import Path
import difflib, hashlib, json, re, subprocess, sys
root=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers=helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
exec(helpers)
forms=load_forms(root)
old=(out/'before.nepld').read_bytes();new=(root/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
old_sentence='parse/lower、bare-metal、previewの[既定値/きていち]やParseProfileのLimitsは[変更/へんこう]しない。'
new_sentence='output_limits[自体/じたい]はparse/lower、bare-metal、previewの[既定値/きていち]やParseProfileのLimitsを[変更/へんこう]しない。'
assert old.count(old_sentence.encode())==1 and new.count(new_sentence.encode())==1
updated=old.replace(old_sentence.encode(),new_sentence.encode())
marker=b'      cons paragraph\n        cons "Section'
pos=updated.rindex(marker)
assert new[:pos]==updated[:pos] and new.endswith(updated[pos:])
addition=new[pos:len(new)-len(updated[pos:])]
assert b'\r' not in old+new and not new.startswith(b'\xef\xbb\xbf')
before=Parser(old.decode(),forms).complete('Doc/Article')
after=Parser(new.decode(),forms).complete('Doc/Article');audit_text(after)
extra=Parser('article ja "addition" body\n'+addition.decode()+'nil',forms).complete('Doc/Article')
paras=[n for n in walk(normalize(extra)) if n['kind']=='Paragraph'];assert len(paras)==3
source=(out/'source.md').read_text(encoding='utf-8')
start=source.index('pages入力manifestには `parse_limits`')
end=source.index('\n\nSectionの明示ID',start)
expected=[p.replace('\n','').replace('`','') for p in source[start:end].split('\n\n')]
actual=[plain(p) for p in paras];assert actual==expected,list(zip(actual,expected))
codes=[n['fields']['text'] for p in paras for n in walk(p) if n['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',source[start:end])
replacement_tree=Parser('article ja "replacement" body cons paragraph cons "'+new_sentence+'" nil nil',forms).complete('Doc/Article')
replacement_plain=plain([n for n in walk(normalize(replacement_tree)) if n['kind']=='Paragraph'][0])
assert replacement_plain==next(line for line in source.splitlines() if line.startswith('output_limits自体'))
pairs.clear();normalize(extra,True);normalize(replacement_tree,True)
result={'scope':'author self-check only; not independent review, runtime parsing, lower, or HTML','source_sha256':hashlib.sha256((out/'source.md').read_bytes()).hexdigest(),'old_draft_sha256':hashlib.sha256(old).hexdigest(),'draft_sha256':hashlib.sha256(new).hexdigest(),'unrelated_bytes_unchanged':True,'added_paragraphs':3,'added_sentences':[len(p['fields']['items']) for p in paras],'replacement_sentences':1,'inline_codes_exact':codes,'ruby_occurrences_added_and_replacement':len(pairs),'base_paragraph_text_exact':True,'structure_and_ruby_audit':True,'utf8_lf':True}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(out/'addition.nepld').write_bytes(addition);(out/'after.nepld').write_bytes(new)
(out/'paragraphs.txt').write_text(replacement_plain+'\n\n'+'\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'draft.diff').write_text(''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8')
source_before=subprocess.check_output(['git','show','c15928aa67839014454f6a4e5d05fcd83857f201:doc/spec/21-doc-pages.md'],cwd=root)
(out/'source-before.md').write_bytes(source_before)
(out/'source.diff').write_text(''.join(difflib.unified_diff(source_before.decode().splitlines(True),source.splitlines(True),fromfile='source-before.md',tofile='source.md')),encoding='utf-8')
for path in ['AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py']:
    p=out/'context'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((root/path).read_bytes())
(out/'context/ruby-helpers.py').write_text(helpers,encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
