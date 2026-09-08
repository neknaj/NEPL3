from pathlib import Path
import collections, difflib, hashlib, json, re, subprocess, sys

root = Path(__file__).resolve().parents[2]
out = Path(__file__).resolve().parent
helper = Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers = helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')]
sys.path.insert(0, str(root))
from tools.audit.structure import Parser, load_forms
exec(helpers)
forms = load_forms(root)
old = (out/'before.nepld').read_bytes()
new = (root/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
assert b'\r' not in old and b'\r' not in new and not new.startswith(b'\xef\xbb\xbf')
marker = b'      cons paragraph\n        cons "Section'
pos = old.rindex(marker)
assert new[:pos] == old[:pos]
assert new.endswith(old[pos:])
addition = new[pos:len(new)-len(old[pos:])]
assert addition
(out/'addition.nepld').write_bytes(addition)
before = Parser(old.decode(), forms).complete('Doc/Article')
after = Parser(new.decode(), forms).complete('Doc/Article')
audit_text(after)
bn = normalize(before)
an = normalize(after)
bp = [n for n in walk(bn) if n['kind']=='Paragraph']
ap = [n for n in walk(an) if n['kind']=='Paragraph']
assert ap[:-4] == bp[:-1] and ap[-1] == bp[-1]
extra = ap[-4:-1]
assert len(extra) == 3
source = (out/'source.md').read_text(encoding='utf-8')
start = source.index('文書一式のbuildでは')
end = source.index('\n\nSectionの明示ID', start)
paragraphs = source[start:end].split('\n\n')
assert len(paragraphs) == 3
expected = [p.replace('\n','').replace('`','') for p in paragraphs]
actual = [plain(p) for p in extra]
assert actual == expected, list(zip(actual,expected))
codes = [n['fields']['text'] for p in extra for n in walk(p) if n['kind']=='InlineCode']
assert codes == re.findall(r'`([^`]+)`', source[start:end])
added_tree = Parser('article ja "addition" body\n'+addition.decode()+'nil', forms).complete('Doc/Article')
pairs.clear()
normalize(added_tree, True)
report = {'scope':'Author self-check only; no production parse/lower/HTML or independent review',
          'source_sha256':hashlib.sha256((out/'source.md').read_bytes()).hexdigest(),
          'original_draft_sha256':hashlib.sha256(old).hexdigest(), 'draft_sha256':hashlib.sha256(new).hexdigest(),
          'original_prefix_suffix_byte_equal':True, 'paragraphs':3, 'sentences':[len(p['fields']['items']) for p in extra],
          'source_paragraph_base_text_exact':True, 'inline_code_exact':codes, 'ruby_occurrences':len(pairs),
          'no_unannotated_narrative_kanji':True, 'utf8_lf':True}
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'paragraphs.txt').write_text('\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'draft.diff').write_text(''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8')
(out/'after.nepld').write_bytes(new)
(out/'result.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report,ensure_ascii=False,indent=2))
