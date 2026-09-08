from pathlib import Path
import hashlib, json, re, sys

root = Path(__file__).resolve().parents[2]
out = Path(__file__).resolve().parent
sys.path.insert(0, str(root))
from tools.audit.structure import Parser, load_forms
text = Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
exec(text[text.index('han=re.compile'):text.index('results=[]\nfor chapter')])
source = (out/'source.md').read_bytes()
assert source == (root/'README.md').read_bytes()
raw = (root/'doc/migration/authored/project/README.nepld').read_bytes()
assert b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf')
tree = Parser(raw.decode(), load_forms(root)).complete('Doc/Article')
audit_text(tree)
normalized = normalize(tree, True)
nodes = list(walk(normalized))
md = source.decode()
blocks = re.findall(r'^```([^\n]*)\n(.*?)^```', md, re.M|re.S)
raw_nodes = [n['fields'] for n in nodes if n['kind']=='RawCode']
assert [n['text'] for n in raw_nodes] == [v for _,v in blocks]
assert [n['languageHint']['fields']['text'] for n in raw_nodes] == [h for h,_ in blocks]
body_md = re.sub(r'^```.*?^```\n?', '', md, flags=re.M|re.S)
codes = [n['fields']['text'] for n in nodes if n['kind']=='InlineCode']
assert codes == re.findall(r'`([^`]+)`', body_md)
images = re.findall(r'!\[([^\]]*)\]\(([^)]+)\)', md)
image_nodes = [n for n in nodes if n['kind']=='InlineImage']
assert len(images)==len(image_nodes)==1
assert (plain(image_nodes[0]['fields']['alt']), image_nodes[0]['fields']['asset']['fields']['id']) == images[0]
assert image_nodes[0]['fields']['asset']['fields']['digest']['kind']=='NoText'
no_images = re.sub(r'!\[([^\]]*)\]\(([^)]+)\)', lambda m:m[1], body_md)
links = [(plain(n['fields']['label']), n['fields']['target']['fields'].get('path',n['fields']['target']['fields'].get('uri'))) for n in nodes if n['kind']=='Link']
assert links == re.findall(r'\[([^\]]+)\]\(([^)]+)\)', no_images), links
strong = [plain(n) for n in nodes if n['kind']=='Strong']
assert strong == re.findall(r'\*\*([^*]+)\*\*', body_md)
def mdplain(s):
    s = re.sub(r'!\[([^\]]*)\]\(([^)]+)\)', lambda m:m[1], s)
    s = re.sub(r'\[([^\]]+)\]\(([^)]+)\)', lambda m:m[1], s)
    return s.replace('**','').replace('`','')
paragraphs = [plain(n) for n in nodes if n['kind']=='Paragraph']
expected = [mdplain(p) for p in re.split(r'\n\s*\n', no_images.strip()) if p and not p.startswith('#') and not p.startswith('|')]
assert paragraphs == expected, list(zip(paragraphs,expected))
rows = [[plain(c) for c in n['fields']['cells']] for n in nodes if n['kind']=='Row']
expected_rows = [[mdplain(c.strip()) for c in line.strip('|').split('|')] for line in body_md.splitlines() if line.startswith('|') and not re.fullmatch(r'[| :\-]+',line)]
assert rows == expected_rows
assert tree['fields']['language']=='ja' and plain(normalized['fields']['title'])=='NEPL3'
result = {'source_sha256':hashlib.sha256(source).hexdigest(),'draft_sha256':hashlib.sha256(raw).hexdigest(),
          'paragraphs_exact':len(paragraphs), 'sentence_counts':[len(n['fields']['items']) for n in nodes if n['kind']=='Paragraph'],
          'table_rows_including_header':len(rows),'table_columns':2,'links_exact':len(links),'inline_codes_exact':codes,
          'raw_code_bytes':len(raw_nodes[0]['text'].encode()),'raw_code_hint':'sh','strong_exact':strong,
          'badge':{'image_uri_as_unresolved_asset_id':images[0][1],'alt':images[0][0],'link_target':links[0][1],'digest':None,'resolved':False},
          'ruby_occurrences':len(pairs),'narrative_kanji_without_ruby':0,'utf8_lf':True,
          'scope':'Author self-check only; no production parse/lower/HTML/asset resolution or independent review'}
(out/'draft.nepld').write_bytes(raw)
(out/'paragraphs.txt').write_text('\n\n'.join(paragraphs)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
