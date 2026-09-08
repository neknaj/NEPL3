import collections, hashlib, importlib.util, json, pathlib, re, sys
sys.stdout.reconfigure(encoding='utf-8')
p=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('annotations',p/'annotation_parser.py')
parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
parser.forms=json.loads((p/'snapshot/design/forms.json').read_text(encoding='utf-8'))['categories']
raw=(p/'snapshot/doc/migration/authored/guide/review.nepld').read_bytes()
md=(p/'snapshot/doc/review.md').read_text(encoding='utf-8')
instance=parser.Parser(raw.decode('utf-8'));value=instance.run()
def text(x):
 if not isinstance(x,list):return ''
 if x and isinstance(x[0],str):
  if x[0] in ['Text','InlineCode']:return x[1]
  if x[0] in ['Ruby','Anno','Strong','Em']:return text(x[1])
  if x[0]=='Link':return text(x[-1])
 return ''.join(map(text,x))
codes=[];links=[];rawcodes=[];tables=[];paragraphs=[];titles=[];rubies=[];uncovered=[];sentences=[];sections=[];annos=[];kinds=collections.Counter()
def walk(x,annotated=False):
 if not isinstance(x,list):return
 if x and isinstance(x[0],str):
  kind=x[0];kinds[kind]+=1
  if kind=='InlineCode':codes.append(x[1]);return
  if kind=='RawCode':rawcodes.append(x);return
  if kind=='Text' and not annotated and re.search('[\u3400-\u9fff々]',x[1]):uncovered.append(x[1])
  if kind=='Ruby':rubies.append([text(x[1]),text(x[2])]);walk(x[1],True);return
  if kind=='Anno':annos.append(x)
  if kind=='Link':links.append(x[1]);walk(x[-1],annotated);return
  if kind=='Paragraph':paragraphs.append(text(x))
  if kind=='Sentence':sentences.append(text(x))
  if kind=='Table':tables.append(x)
  if kind in ['Section','Article']:titles.append(text(x[2]))
  if kind=='Section':sections.append(x[1])
 for y in x:walk(y,annotated)
walk(value)
fences=re.findall(r'^```([^\n]*)\n(.*?)^```[ \t]*$',md,re.M|re.S)
no_fences=re.sub(r'^```[^\n]*\n.*?^```[ \t]*\n?', '',md,flags=re.M|re.S)
def plain(s):
 return re.sub(r'\*\*([^*]+)\*\*',r'\1',re.sub(r'`([^`]+)`',r'\1',re.sub(r'\[([^\]]*)\]\(([^)]+)\)',r'\1',s)))
mp=[];mt=[];mc=[]
for block in no_fences.strip().split('\n\n'):
 lines=block.splitlines()
 if lines and lines[0].startswith('|'):
  for line in lines:
   if not re.fullmatch(r'[| :\-]+',line):mc.append([plain(c.strip()) for c in line.strip('|').split('|')])
  continue
 blocks=lines if lines and all(re.match(r'^(?:- |\d+\. )',x) for x in lines) else [''.join(lines)]
 for line in blocks:
  if not line:continue
  if line.startswith('#'):mt.append(plain(re.sub(r'^#+ ', '',line)))
  else:mp.append(plain(re.sub(r'^(?:- |\d+\. )','',line)))
ac=[]
for t in tables:
 assert all(a==['AlignmentDefault'] for a in t[1]),t[1]
 assert t[2][0]=='SomeRow'
 ac.append([text(c) for c in t[2][1][1]])
 ac.extend([text(c) for c in r[1]] for r in t[3])
mismatches=[]
for n,(a,b) in enumerate(zip(mp,paragraphs)):
 if a!=b:mismatches.append(dict(index=n,original=a,draft=b))
result=dict(draft_sha256=hashlib.sha256(raw).hexdigest(),paragraph_counts=[len(mp),len(paragraphs)],paragraph_mismatches=mismatches,titles_equal=mt==titles,table_rows_equal=mc==ac,table_rows=len(mc),table_columns=[len(t[1]) for t in tables],inline_code_equal=re.findall(r'`([^`]+)`',no_fences)==codes,inline_codes=len(codes),links_equal=re.findall(r'\[[^\]]*\]\(([^)]+)\)',no_fences)==[l[1] for l in links],links=links,raw_code_equal=[(x[1][1],x[2]) for x in rawcodes]==fences,raw_codes=len(rawcodes),ruby_count=len(rubies),ruby_unique=len(set(map(tuple,rubies))),ruby_invalid=[r for r in rubies if not re.fullmatch('[\u3400-\u9fff々]+',r[0]) or not re.fullmatch('[ぁ-ゖー]+',r[1])],uncovered_kanji=uncovered,sentence_count=len(sentences),multiple_sentence_candidates=[s for s in sentences if s.count('。')>1],sections_unique=len(sections)==len(set(sections)),kinds=kinds,anno_count=len(annos),utf8_lf=b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf'))
for name,data in [('checks.json',result),('readings.json',sorted(set(map(tuple,rubies)))),('paragraphs.json',paragraphs),('sentences.json',sentences),('tables.json',ac),('annotations.json',annos)]:
 (p/name).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['links','multiple_sentence_candidates','paragraph_mismatches']},ensure_ascii=False,indent=2))
print('mismatch indexes', [x['index'] for x in mismatches], 'multiple sentence candidates', len(result['multiple_sentence_candidates']))
assert len(mp)==len(paragraphs)
assert all(result[k] for k in ['titles_equal','table_rows_equal','inline_code_equal','links_equal','raw_code_equal','sections_unique','utf8_lf'])
assert not result['ruby_invalid'] and not uncovered
assert not mismatches, 'Exact narrative mismatch; inspect checks.json (do not normalize whitespace)'
