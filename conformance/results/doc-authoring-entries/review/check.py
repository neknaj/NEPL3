import pathlib,json,hashlib,re,importlib.util
p=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('parser',p/'annotation_parser.py');parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser);parser.forms=json.loads((p/'snapshot/design/forms.json').read_text(encoding='utf-8'))['categories']
def text(x):
 if not isinstance(x,list):return ''
 if x and isinstance(x[0],str):
  if x[0] in ['Text','InlineCode']:return x[1]
  if x[0]=='Ruby':return text(x[1])
  if x[0]=='Link':return text(x[-1])
  if x[0] in ['Anno','Strong','Em']:return text(x[1])
 return ''.join(text(y) for y in x)
results=[]
for scope in ['history','migration']:
 raw=(p/'snapshot/doc/migration/authored'/scope/'README.nepld').read_bytes();md=(p/'snapshot/doc'/scope/'README.md').read_text(encoding='utf-8');v=parser.Parser(raw.decode()).run()
 paragraphs=[];titles=[];codes=[];rawcodes=[];links=[];strong=[];ruby=[];uncovered=[];sentences=[]
 def walk(x,annotated=False):
  if not isinstance(x,list):return
  if x and isinstance(x[0],str):
   kind=x[0]
   if kind=='RawCode':rawcodes.append(x);return
   if kind=='InlineCode':codes.append(x[1]);return
   if kind=='Text' and not annotated and re.search('[\u3400-\u9fff々]',x[1]):uncovered.append(x[1])
   if kind=='Ruby':ruby.append([text(x[1]),text(x[2])]);walk(x[1],True);return
   if kind=='Link':links.append(x[1][1]);walk(x[-1],annotated);return
   if kind=='Strong':strong.append(text(x[1]))
   if kind=='Paragraph':paragraphs.append(text(x))
   if kind=='Sentence':sentences.append(text(x))
   if kind in ['Article','Section']:titles.append(text(x[2]))
  for y in x:walk(y,annotated)
 walk(v)
 fences=re.findall(r'^```([^\n]*)\n(.*?)^```[ \t]*$',md,flags=re.M|re.S)
 no_fences=re.sub(r'^```[^\n]*\n.*?^```[ \t]*\n?', '',md,flags=re.M|re.S)
 assert [x[2] for x in rawcodes]==[b for _,b in fences],rawcodes
 assert [x[1][1] for x in rawcodes]==[a for a,_ in fences],rawcodes
 assert re.findall(r'`([^`]+)`',no_fences)==codes
 assert re.findall(r'\[[^\]]*\]\(([^)]+)\)',no_fences)==links
 assert re.findall(r'\*\*([^*]+)\*\*',no_fences)==strong
 def plain(s):return re.sub(r'\*\*([^*]+)\*\*',r'\1',re.sub(r'`([^`]+)`',r'\1',re.sub(r'\[([^\]]*)\]\([^)]+\)',r'\1',s)))
 mp=[];mt=[]
 for block in no_fences.strip().split('\n\n'):
  lines=block.splitlines();blocks=lines if lines and all(x.startswith('- ') for x in lines) else [''.join(lines)]
  for line in blocks:
   if not line:continue
   if line.startswith('#'):mt.append(plain(re.sub(r'^#+ ', '',line)))
   else:mp.append(plain(re.sub(r'^- ','',line)))
 assert mp==paragraphs,(mp,paragraphs);assert mt==titles;assert not uncovered;assert b'\r' not in raw
 assert all(re.fullmatch('[\u3400-\u9fff々]+',a) and re.fullmatch('[ぁ-ゖー]+',b) for a,b in ruby)
 results.append(dict(scope=scope,paragraphs_exact=len(paragraphs),titles=titles,codes=codes,links=links,strong=strong,rawcode=[dict(language=a,bytes=len(b.encode()),sha256=hashlib.sha256(b.encode()).hexdigest(),text=b) for a,b in fences],ruby=ruby,sentences=len(sentences),uncovered_kanji=uncovered))
(p/'result.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n');print('PASS',[(r['scope'],r['paragraphs_exact'],r['sentences'],len(r['ruby']),len(r['rawcode'])) for r in results])
