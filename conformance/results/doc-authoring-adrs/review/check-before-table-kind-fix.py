import collections,hashlib,importlib.util,json,pathlib,re,sys
sys.stdout.reconfigure(encoding='utf-8')
p=pathlib.Path(__file__).resolve().parent
# Preserve Ruby nodes for annotation inspection; the inherited restricted parser
# otherwise projects them away. This is a reviewer helper, not production parsing.
s=(p/'source_parser.py').read_text(encoding='utf-8')
s=s.replace('readings.append([a,b]); out+=a',"readings.append([a,b]); out.append(['Ruby',a,b])")
s=s.replace("if kind=='Ruby':self.readings.append(fields);return fields[0]","if kind=='Ruby':self.readings.append(fields);return [[kind]+fields]")
(p/'annotation_parser.py').write_text(s,encoding='utf-8',newline='\n')
spec=importlib.util.spec_from_file_location('annotations',p/'annotation_parser.py');parser=importlib.util.module_from_spec(spec);spec.loader.exec_module(parser)
parser.forms=json.loads((p/'snapshot/design/forms.json').read_text(encoding='utf-8'))['categories']
def text(x):
    if not isinstance(x,list):return ''
    if x and isinstance(x[0],str):
        if x[0] in ['Text','InlineCode']:return x[1]
        if x[0]=='Ruby':return text(x[1])
        if x[0]=='Link':return text(x[-1])
        if x[0]=='Anno':return text(x[1])
        if x[0] in ['Sentence','Paragraph']:return ''.join(map(text,x[1:]))
    return ''.join(map(text,x))
result=[]
for row in json.loads((p/'author-input-manifest.json').read_text(encoding='utf-8'))['files']:
    raw=(p/'snapshot'/row['path']).read_bytes();md=(p/'snapshot'/row['original_path']).read_text(encoding='utf-8')
    instance=parser.Parser(raw.decode('utf-8'));value=instance.run()
    codes=[];links=[];tables=[];sentences=[];paragraphs=[];titles=[];rubies=[];uncovered=[];sections=[]
    def walk(x,in_ruby=False):
        if not isinstance(x,list):return
        if x and isinstance(x[0],str):
            kind=x[0]
            if kind=='Text' and not in_ruby and re.search('[\u3400-\u9fff々]',x[1]):uncovered.append(x[1])
            if kind=='InlineCode':codes.append(x[1]);return
            if kind=='Ruby':
                rubies.append([text(x[1]),text(x[2])]);walk(x[1],True);walk(x[2],True);return
            if kind=='Link':links.append(x[1]);walk(x[-1],in_ruby);return
            if kind=='Table':tables.append(x)
            if kind=='Sentence':sentences.append(text(x))
            if kind=='Paragraph':paragraphs.append(text(x))
            if kind=='Article':titles.append(text(x[2]))
            if kind=='Section':sections.append(x[1]);titles.append(text(x[2]))
        for child in x:walk(child,in_ruby)
    walk(value)
    original_codes=re.findall(r'`([^`]+)`',md);assert original_codes==codes
    original_links=re.findall(r'\[[^\]]*\]\(([^)]+)\)',md)
    targets=[l[1] for l in links];assert original_links==targets
    assert not uncovered,uncovered
    assert all(re.fullmatch('[\u3400-\u9fff々]+',a) and re.fullmatch('[\u3040-\u309fー]+',b) for a,b in rubies)
    assert len(sections)==len(set(sections))
    assert b'\r' not in raw and not raw.startswith(b'\xef\xbb\xbf')
    # These originals contain single-line paragraphs/list items, a two-column
    # pipe table and no fenced code, emphasis, escapes or nested Markdown lists.
    # Compare exact sentence text including spaces/code/label bytes after only
    # removing the original Markdown structural delimiters.
    md_lines=[];md_titles=[];md_cells=[]
    def markdown_text(line):
        return re.sub(r'`([^`]+)`',r'\1',re.sub(r'\[([^\]]*)\]\([^)]+\)',r'\1',line))
    for line in md.splitlines():
        if not line:continue
        if line.startswith('#'):md_titles.append(markdown_text(re.sub(r'^#+ ', '',line)));continue
        if line.startswith('|'):
            if re.fullmatch(r'[| :\-]+',line):continue
            md_cells.append([markdown_text(cell.strip()) for cell in line.strip('|').split('|')]);continue
        md_lines.append(markdown_text(re.sub(r'^(?:- |\d+\. )','',line)))
    assert paragraphs==md_lines,(row['path'],paragraphs,md_lines)
    assert titles==md_titles,(titles,md_titles)
    actual_cells=[]
    for table in tables:
        assert table[1]==[['Default'],['Default']]
        assert table[2][0]=='SomeRow'
        actual_cells.append([text(c) for c in table[2][1][1]])
        actual_cells.extend([text(c) for c in r[1]] for r in table[3])
    assert actual_cells==md_cells,(actual_cells,md_cells)
    result.append(dict(path=row['path'],sha256=hashlib.sha256(raw).hexdigest(),paragraphs_exact=len(paragraphs),titles_exact=titles,code_exact=codes,links_exact=targets,table_rows_exact=actual_cells,ruby_count=len(rubies),rubies=rubies,uncovered_kanji=uncovered,sections=sections))
(p/'checks.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps([{k:v for k,v in r.items() if k!='rubies'} for r in result],ensure_ascii=False,indent=2))
